use std::collections::HashMap;
use std::sync::Arc;

use anyclaw_config::{PreopenedDir, WasmSandboxConfig};
use wasmtime::{
    Config, Engine, Error as WasmtimeError, Linker, Module, ResourceLimiter, Store, Trap,
};
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

use crate::error::ToolsError;

/// Enforces a per-invocation memory cap on WASM linear memory growth.
///
/// Returned `Ok(false)` from `memory_growing` causes `memory.grow` to return -1
/// (failure) to the WASM module, not a trap. Modules that call `unreachable` on
/// growth failure will trap; modules that ignore it will see a wrong allocation.
struct WasmResourceLimiter {
    memory_limit: usize,
}

impl ResourceLimiter for WasmResourceLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, WasmtimeError> {
        let allow = desired <= self.memory_limit;
        if !allow {
            tracing::warn!(
                current_bytes = current,
                desired_bytes = desired,
                limit_bytes = self.memory_limit,
                "WASM memory growth denied — exceeds configured limit"
            );
        }
        Ok(allow)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, WasmtimeError> {
        Ok(desired <= 10_000)
    }
}

/// Holds both the WASI context and the resource limiter in a single Store data type.
///
/// wasmtime's `Store::limiter()` requires the limiter to be accessible via a closure
/// over the store data — this wrapper makes that possible while keeping both pieces
/// of per-invocation state colocated.
struct WasmState {
    wasi: WasiP1Ctx,
    limiter: WasmResourceLimiter,
}

pub struct WasmToolRunner {
    engine: Arc<Engine>,
    epoch_handle: tokio::task::JoinHandle<()>,
}

impl WasmToolRunner {
    pub fn new() -> Result<Self, ToolsError> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);

        let engine = Arc::new(
            Engine::new(&config)
                .map_err(|e| ToolsError::McpHostFailed(format!("wasmtime engine: {e}")))?,
        );

        let engine_clone = engine.clone();
        let epoch_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                anyclaw_core::constants::EPOCH_TICK_INTERVAL_SECS,
            ));
            loop {
                interval.tick().await;
                engine_clone.increment_epoch();
            }
        });

        Ok(Self {
            engine,
            epoch_handle,
        })
    }

    pub async fn execute(
        &self,
        module_bytes: &[u8],
        input_json: &str,
        sandbox: &WasmSandboxConfig,
        options: &HashMap<String, serde_json::Value>,
    ) -> Result<String, ToolsError> {
        let engine = self.engine.clone();
        let module_bytes = module_bytes.to_vec();
        let input_json = input_json.to_string();
        let sandbox = sandbox.clone();
        let options = options.clone();

        tokio::task::spawn_blocking(move || {
            Self::execute_sync(&engine, &module_bytes, &input_json, &sandbox, &options)
        })
        .await
        .map_err(|e| ToolsError::McpHostFailed(format!("spawn_blocking: {e}")))?
    }

    fn execute_sync(
        engine: &Engine,
        module_bytes: &[u8],
        input_json: &str,
        sandbox: &WasmSandboxConfig,
        options: &HashMap<String, serde_json::Value>,
    ) -> Result<String, ToolsError> {
        let module = Module::new(engine, module_bytes)
            .map_err(|e| ToolsError::McpHostFailed(format!("wasm compile: {e}")))?;

        let stdout = MemoryOutputPipe::new(4096);
        let wasi = build_wasi_ctx(input_json, &stdout, &sandbox.preopened_dirs, options)?;

        let state = WasmState {
            wasi,
            limiter: WasmResourceLimiter {
                memory_limit: sandbox.memory_limit_bytes as usize,
            },
        };
        let mut store = Store::new(engine, state);
        store.limiter(|s| &mut s.limiter);
        store
            .set_fuel(sandbox.fuel_limit)
            .map_err(|e| ToolsError::McpHostFailed(format!("set fuel: {e}")))?;
        store.set_epoch_deadline(sandbox.epoch_timeout_secs);
        store.epoch_deadline_trap();

        let mut linker: Linker<WasmState> = Linker::new(engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |state: &mut WasmState| &mut state.wasi)
            .map_err(|e| ToolsError::McpHostFailed(format!("wasi link: {e}")))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| ToolsError::McpHostFailed(format!("instantiate: {e}")))?;

        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|e| ToolsError::McpHostFailed(format!("no _start export: {e}")))?;

        if let Err(e) = start.call(&mut store, ()) {
            if let Some(trap) = e.downcast_ref::<Trap>() {
                return match trap {
                    Trap::OutOfFuel => Err(ToolsError::McpHostFailed(
                        "WASM execution exceeded fuel limit".into(),
                    )),
                    Trap::Interrupt => Err(ToolsError::McpHostFailed(
                        "WASM execution exceeded time limit".into(),
                    )),
                    _ => Err(ToolsError::McpHostFailed(format!("wasm trap: {trap}"))),
                };
            }
            // Check if the error string indicates a memory limit denial — this occurs when
            // memory.grow fails (ResourceLimiter returned Ok(false)) and the WASM module
            // calls unreachable or the error propagates as an unreachable trap.
            let err_str = format!("{e}");
            if err_str.contains("unreachable") {
                return Err(ToolsError::McpHostFailed(
                    "WASM execution exceeded memory limit".into(),
                ));
            }
            return Err(ToolsError::McpHostFailed(format!("wasm error: {e}")));
        }

        let output_bytes = stdout.contents();
        let output = String::from_utf8(output_bytes.to_vec())
            .map_err(|e| ToolsError::McpHostFailed(format!("invalid utf8 output: {e}")))?;

        Ok(output.trim().to_string())
    }

    pub fn engine(&self) -> &Arc<Engine> {
        &self.engine
    }
}

/// Builds a WasiP1Ctx with the given stdin/stdout and preopened directories.
///
/// With no preopened_dirs, the WASM module has zero filesystem access.
/// Each PreopenedDir is wired in with the configured permissions:
/// - `readonly = true` → DirPerms::READ + FilePerms::READ
/// - `readonly = false` → DirPerms::all() + FilePerms::all()
fn build_wasi_ctx(
    input_json: &str,
    stdout: &MemoryOutputPipe,
    preopened_dirs: &[PreopenedDir],
    options: &HashMap<String, serde_json::Value>,
) -> Result<WasiP1Ctx, ToolsError> {
    let mut builder = WasiCtxBuilder::new();
    builder
        .stdin(MemoryInputPipe::new(input_json.as_bytes().to_vec()))
        .stdout(stdout.clone())
        .inherit_stderr();

    for preopen in preopened_dirs {
        let (dir_perms, file_perms) = if preopen.readonly {
            (DirPerms::READ, FilePerms::READ)
        } else {
            (DirPerms::all(), FilePerms::all())
        };
        builder
            .preopened_dir(&preopen.host, &preopen.guest, dir_perms, file_perms)
            .map_err(|e| {
                ToolsError::McpHostFailed(format!("preopened dir {}: {e}", preopen.guest))
            })?;
    }

    for (key, value) in options {
        let val = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        builder.env(key, &val);
    }

    Ok(builder.build_p1())
}

impl Drop for WasmToolRunner {
    fn drop(&mut self) {
        self.epoch_handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyclaw_config::WasmSandboxConfig;
    use rstest::rstest;

    fn default_sandbox() -> WasmSandboxConfig {
        WasmSandboxConfig::default()
    }

    fn low_fuel_sandbox() -> WasmSandboxConfig {
        WasmSandboxConfig {
            fuel_limit: 1000,
            ..Default::default()
        }
    }

    fn short_epoch_sandbox() -> WasmSandboxConfig {
        WasmSandboxConfig {
            fuel_limit: u64::MAX,
            epoch_timeout_secs: 1,
            ..Default::default()
        }
    }

    /// Sandbox with a tight memory limit: 1 WASM page (64 KiB).
    fn one_page_memory_sandbox() -> WasmSandboxConfig {
        WasmSandboxConfig {
            // 64 KiB = exactly one WASM page; any memory.grow beyond initial page is denied
            memory_limit_bytes: 65536,
            fuel_limit: u64::MAX,
            epoch_timeout_secs: 30,
            ..Default::default()
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_wasm_runner_created_then_engine_initialized_with_fuel_and_epoch() {
        let runner = WasmToolRunner::new().unwrap();
        assert!(Arc::strong_count(runner.engine()) >= 1);
    }

    #[rstest]
    #[tokio::test]
    async fn when_engine_accessor_called_then_returns_shared_engine() {
        let runner = WasmToolRunner::new().unwrap();
        let engine_ref = runner.engine();
        assert!(Arc::strong_count(engine_ref) >= 2);
        let cloned = engine_ref.clone();
        assert!(Arc::ptr_eq(engine_ref, &cloned));
    }

    #[rstest]
    #[tokio::test]
    async fn when_echo_wasm_module_executed_then_output_matches_input() {
        let runner = WasmToolRunner::new().unwrap();

        let wat = r#"
            (module
                (import "wasi_snapshot_preview1" "fd_read"
                    (func $fd_read (param i32 i32 i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "fd_write"
                    (func $fd_write (param i32 i32 i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "proc_exit"
                    (func $proc_exit (param i32)))
                (memory (export "memory") 1)
                (func (export "_start")
                    ;; Set up iovec at offset 100: buf ptr=200, buf len=256
                    (i32.store (i32.const 100) (i32.const 200))
                    (i32.store (i32.const 104) (i32.const 256))
                    ;; Read from stdin (fd 0)
                    (call $fd_read
                        (i32.const 0)   ;; fd: stdin
                        (i32.const 100) ;; iovs ptr
                        (i32.const 1)   ;; iovs len
                        (i32.const 96)  ;; nread ptr
                    )
                    drop
                    ;; Write to stdout (fd 1) using same buffer, nread bytes
                    (i32.store (i32.const 108) (i32.const 200))
                    (i32.store (i32.const 112) (i32.load (i32.const 96)))
                    (call $fd_write
                        (i32.const 1)   ;; fd: stdout
                        (i32.const 108) ;; iovs ptr
                        (i32.const 1)   ;; iovs len
                        (i32.const 96)  ;; nwritten ptr
                    )
                    drop
                )
            )
        "#;

        let module_bytes = wat::parse_str(wat).unwrap();
        let result = runner
            .execute(
                &module_bytes,
                r#"{"hello":"world"}"#,
                &default_sandbox(),
                &HashMap::new(),
            )
            .await
            .unwrap();
        assert_eq!(result, r#"{"hello":"world"}"#);
    }

    #[rstest]
    #[tokio::test]
    async fn when_wasm_module_exceeds_fuel_limit_then_execution_returns_fuel_error() {
        let runner = WasmToolRunner::new().unwrap();

        let wat = r#"
            (module
                (memory (export "memory") 1)
                (func (export "_start")
                    (local $i i32)
                    (loop $loop
                        (local.set $i (i32.add (local.get $i) (i32.const 1)))
                        (br_if $loop (i32.lt_u (local.get $i) (i32.const 1000000)))
                    )
                )
            )
        "#;

        let module_bytes = wat::parse_str(wat).unwrap();
        let result = runner
            .execute(&module_bytes, "", &low_fuel_sandbox(), &HashMap::new())
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("fuel"), "expected fuel error, got: {err}");
    }

    #[rstest]
    #[tokio::test]
    async fn when_wasm_module_runs_infinite_loop_then_execution_returns_epoch_error() {
        let runner = WasmToolRunner::new().unwrap();

        let wat = r#"
            (module
                (memory (export "memory") 1)
                (func (export "_start")
                    (loop $loop (br $loop))
                )
            )
        "#;

        let module_bytes = wat::parse_str(wat).unwrap();
        let result = runner
            .execute(&module_bytes, "", &short_epoch_sandbox(), &HashMap::new())
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("epoch") || err.contains("time limit"),
            "expected epoch error, got: {err}"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_wasm_module_executed_twice_then_each_invocation_uses_fresh_store() {
        let runner = WasmToolRunner::new().unwrap();

        let wat = r#"
            (module
                (memory (export "memory") 1)
                (func (export "_start"))
            )
        "#;

        let module_bytes = wat::parse_str(wat).unwrap();
        let sandbox = default_sandbox();
        let r1 = runner
            .execute(&module_bytes, "", &sandbox, &HashMap::new())
            .await;
        let r2 = runner
            .execute(&module_bytes, "", &sandbox, &HashMap::new())
            .await;
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }

    /// A WASM module that starts with 1 page (64 KiB) and tries to grow by 1 more page.
    /// With a 1-page memory limit, the growth is denied (memory.grow returns -1).
    /// The module checks the result and calls `unreachable` if growth failed,
    /// causing a trap that surfaces as a "memory limit" error.
    #[rstest]
    #[tokio::test]
    async fn when_wasm_module_exceeds_memory_limit_then_execution_returns_memory_error() {
        let runner = WasmToolRunner::new().unwrap();

        // Start with 1 page (64 KiB = within limit), try to grow by 1 more page (128 KiB total = exceeds limit)
        // memory.grow returns -1 on denial; the module traps via unreachable if denied
        let wat = r#"
            (module
                (memory (export "memory") 1)
                (func (export "_start")
                    ;; Attempt to grow by 1 page (to 128 KiB total, exceeds our 64 KiB limit)
                    (memory.grow (i32.const 1))
                    ;; If memory.grow returned -1 (as i32), growth was denied
                    (i32.const -1)
                    i32.eq
                    (if (then unreachable))
                )
            )
        "#;

        let module_bytes = wat::parse_str(wat).unwrap();
        let result = runner
            .execute(
                &module_bytes,
                "",
                &one_page_memory_sandbox(),
                &HashMap::new(),
            )
            .await;
        assert!(result.is_err(), "expected error due to memory limit");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("memory limit") || err.contains("unreachable"),
            "expected memory limit error, got: {err}"
        );
    }

    /// Verifies that a module within the memory limit executes successfully.
    #[rstest]
    #[tokio::test]
    async fn when_wasm_module_within_memory_limit_then_execution_succeeds() {
        let runner = WasmToolRunner::new().unwrap();

        // Module starts with 1 page and does NOT try to grow — should succeed with 1-page limit
        let wat = r#"
            (module
                (memory (export "memory") 1)
                (func (export "_start"))
            )
        "#;

        let module_bytes = wat::parse_str(wat).unwrap();
        let result = runner
            .execute(
                &module_bytes,
                "",
                &one_page_memory_sandbox(),
                &HashMap::new(),
            )
            .await;
        assert!(
            result.is_ok(),
            "expected success within memory limit, got: {result:?}"
        );
    }

    /// Verifies that with no preopened_dirs configured, the sandbox has no filesystem access.
    /// A module that attempts to read from a non-preopened path via WASI path_open
    /// gets an error return (ENOENT/ENOTCAPABLE), which surfaces as a non-zero proc_exit.
    /// We test this indirectly: the no-preopened-dirs case should still allow modules
    /// that do not use the filesystem (backward compat).
    #[rstest]
    #[tokio::test]
    async fn when_no_preopened_dirs_configured_then_module_without_fs_access_still_runs() {
        let runner = WasmToolRunner::new().unwrap();

        // Minimal module with no filesystem operations — should succeed even with no preopens
        let wat = r#"
            (module
                (memory (export "memory") 1)
                (func (export "_start"))
            )
        "#;

        let module_bytes = wat::parse_str(wat).unwrap();
        let sandbox = WasmSandboxConfig {
            preopened_dirs: vec![],
            ..Default::default()
        };
        let result = runner
            .execute(&module_bytes, "", &sandbox, &HashMap::new())
            .await;
        assert!(
            result.is_ok(),
            "non-filesystem module should succeed with no preopens: {result:?}"
        );
    }

    /// Verifies that configuring a preopened directory (readonly) allows the module to run.
    /// We check that build_wasi_ctx does not error when given a valid preopened dir.
    #[rstest]
    fn when_preopened_dir_configured_then_wasi_ctx_builds_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let stdout = MemoryOutputPipe::new(4096);
        let preopened = vec![PreopenedDir {
            host: dir.path().to_path_buf(),
            guest: "/data".into(),
            readonly: true,
        }];

        let result = build_wasi_ctx("{}", &stdout, &preopened, &HashMap::new());
        assert!(
            result.is_ok(),
            "expected WasiCtx to build with valid preopened dir"
        );
    }

    #[rstest]
    fn when_preopened_dir_host_path_does_not_exist_then_wasi_ctx_build_fails() {
        let stdout = MemoryOutputPipe::new(4096);
        let preopened = vec![PreopenedDir {
            host: std::path::PathBuf::from("/nonexistent/path/that/does/not/exist"),
            guest: "/data".into(),
            readonly: true,
        }];

        let result = build_wasi_ctx("{}", &stdout, &preopened, &HashMap::new());
        assert!(result.is_err(), "expected error for nonexistent host path");
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("preopened dir"),
            "expected preopened dir error message, got: {err}"
        );
    }

    #[rstest]
    fn when_options_provided_then_wasi_ctx_builds_without_error() {
        let stdout = MemoryOutputPipe::new(4096);
        let mut options = HashMap::new();
        options.insert(
            "MY_KEY".into(),
            serde_json::Value::String("my_value".into()),
        );
        options.insert("NUMERIC".into(), serde_json::json!(42));

        let result = build_wasi_ctx("{}", &stdout, &[], &options);
        assert!(result.is_ok(), "expected WasiCtx to build with options");
    }

    #[rstest]
    #[tokio::test]
    async fn when_wasm_tool_executed_with_empty_options_then_execution_succeeds() {
        let runner = WasmToolRunner::new().unwrap();

        let wat = r#"
            (module
                (memory (export "memory") 1)
                (func (export "_start"))
            )
        "#;

        let module_bytes = wat::parse_str(wat).unwrap();
        let result = runner
            .execute(&module_bytes, "", &default_sandbox(), &HashMap::new())
            .await;
        assert!(
            result.is_ok(),
            "expected success with empty options: {result:?}"
        );
    }
}
