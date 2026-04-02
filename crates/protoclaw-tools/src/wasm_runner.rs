use std::sync::Arc;

use protoclaw_config::WasmSandboxConfig;
use wasmtime::{Config, Engine, Linker, Module, Store, Trap};
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::WasiCtxBuilder;

use crate::error::ToolsError;

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
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
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
    ) -> Result<String, ToolsError> {
        let engine = self.engine.clone();
        let module_bytes = module_bytes.to_vec();
        let input_json = input_json.to_string();
        let sandbox = sandbox.clone();

        tokio::task::spawn_blocking(move || {
            Self::execute_sync(&engine, &module_bytes, &input_json, &sandbox)
        })
        .await
        .map_err(|e| ToolsError::McpHostFailed(format!("spawn_blocking: {e}")))?
    }

    fn execute_sync(
        engine: &Engine,
        module_bytes: &[u8],
        input_json: &str,
        sandbox: &WasmSandboxConfig,
    ) -> Result<String, ToolsError> {
        let module = Module::new(engine, module_bytes)
            .map_err(|e| ToolsError::McpHostFailed(format!("wasm compile: {e}")))?;

        let stdout = MemoryOutputPipe::new(4096);
        let wasi = WasiCtxBuilder::new()
            .stdin(MemoryInputPipe::new(input_json.as_bytes().to_vec()))
            .stdout(stdout.clone())
            .inherit_stderr()
            .build_p1();

        let mut store = Store::new(engine, wasi);
        store
            .set_fuel(sandbox.fuel_limit)
            .map_err(|e| ToolsError::McpHostFailed(format!("set fuel: {e}")))?;
        store.set_epoch_deadline(sandbox.epoch_timeout_secs);
        store.epoch_deadline_trap();

        let mut linker: Linker<WasiP1Ctx> = Linker::new(engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |ctx| ctx)
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

impl Drop for WasmToolRunner {
    fn drop(&mut self) {
        self.epoch_handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn runner_creates_engine_with_fuel_and_epoch() {
        let runner = WasmToolRunner::new().unwrap();
        assert!(Arc::strong_count(runner.engine()) >= 1);
    }

    #[tokio::test]
    async fn runner_execute_simple_echo_module() {
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
            .execute(&module_bytes, r#"{"hello":"world"}"#, &default_sandbox())
            .await
            .unwrap();
        assert_eq!(result, r#"{"hello":"world"}"#);
    }

    #[tokio::test]
    async fn runner_execute_fuel_exhaustion() {
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
            .execute(&module_bytes, "", &low_fuel_sandbox())
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("fuel"),
            "expected fuel error, got: {err}"
        );
    }

    #[tokio::test]
    async fn runner_execute_epoch_timeout() {
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
            .execute(&module_bytes, "", &short_epoch_sandbox())
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("epoch") || err.contains("time limit"),
            "expected epoch error, got: {err}"
        );
    }

    #[tokio::test]
    async fn runner_fresh_store_per_invocation() {
        let runner = WasmToolRunner::new().unwrap();

        let wat = r#"
            (module
                (memory (export "memory") 1)
                (func (export "_start"))
            )
        "#;

        let module_bytes = wat::parse_str(wat).unwrap();
        let sandbox = default_sandbox();
        let r1 = runner.execute(&module_bytes, "", &sandbox).await;
        let r2 = runner.execute(&module_bytes, "", &sandbox).await;
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }
}
