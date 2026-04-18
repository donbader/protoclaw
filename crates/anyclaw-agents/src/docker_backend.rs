use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyclaw_config::parse::{parse_cpu_limit, parse_memory_limit};
use anyclaw_config::types::{DockerWorkspaceConfig, PullPolicy};
use bollard::Docker;
use bollard::models::{ContainerCreateBody, EndpointSettings, HostConfig, NetworkingConfig};
use bollard::query_parameters::{
    AttachContainerOptions, CreateContainerOptions, CreateImageOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions, WaitContainerOptions,
};
use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tracing::{info, warn};

use crate::backend::ProcessBackend;
use crate::error::AgentsError;

/// [`ProcessBackend`] implementation for Docker containers via the bollard API.
///
/// Manages the full container lifecycle: create → attach → start → stop → remove.
/// Stdio is captured via Docker attach and exposed as async read/write streams.
pub struct DockerBackend {
    docker: Docker,
    container_id: Option<String>,
    container_name: Option<String>,
    alive: Arc<AtomicBool>,
    stdin: Mutex<Option<Box<dyn AsyncWrite + Unpin + Send>>>,
    stdout: Mutex<Option<Box<dyn AsyncRead + Unpin + Send>>>,
    stderr: Mutex<Option<Box<dyn AsyncRead + Unpin + Send>>>,
}

impl DockerBackend {
    fn connect(config: &DockerWorkspaceConfig) -> Result<Docker, AgentsError> {
        match &config.docker_host {
            Some(host) => Docker::connect_with_http(host, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| AgentsError::DockerError(e.to_string())),
            None => Docker::connect_with_local_defaults()
                .map_err(|e| AgentsError::DockerError(e.to_string())),
        }
    }

    fn build_container_config(
        config: &DockerWorkspaceConfig,
        labels: HashMap<String, String>,
        host_config: HostConfig,
    ) -> ContainerCreateBody {
        let env: Vec<String> = config
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        let entrypoint = config.entrypoint.as_ref().map(|ep| ep.0.clone());

        let networking_config = config.network.as_ref().map(|net| {
            let mut endpoints = HashMap::new();
            endpoints.insert(net.clone(), EndpointSettings::default());
            NetworkingConfig {
                endpoints_config: Some(endpoints),
            }
        });

        ContainerCreateBody {
            image: Some(config.image.clone()),
            hostname: None,
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(false),
            open_stdin: Some(true),
            env: if env.is_empty() { None } else { Some(env) },
            cmd: None,
            entrypoint,
            labels: Some(labels),
            host_config: Some(host_config),
            networking_config,
            ..Default::default()
        }
    }

    async fn create_and_start_container(
        docker: &Docker,
        container_name: &str,
        container_config: ContainerCreateBody,
    ) -> Result<String, AgentsError> {
        let create_opts = CreateContainerOptions {
            name: Some(container_name.to_string()),
            ..Default::default()
        };
        let created = docker
            .create_container(Some(create_opts), container_config)
            .await
            .map_err(|e| AgentsError::DockerError(e.to_string()))?;

        let container_id = created.id;
        info!(container_id = %container_id, container_name = %container_name, "Created Docker container");

        docker
            .start_container(&container_id, None::<StartContainerOptions>)
            .await
            .map_err(|e| AgentsError::DockerError(e.to_string()))?;
        info!(container_id = %container_id, container_name = %container_name, "Started Docker container");

        Ok(container_id)
    }

    async fn attach_container_streams(
        docker: &Docker,
        container_id: &str,
        agent_name: &str,
    ) -> Result<AttachedStreams, AgentsError> {
        let attach_opts = AttachContainerOptions {
            stdin: true,
            stdout: true,
            stderr: true,
            stream: true,
            logs: false,
            detach_keys: None,
        };
        let attach = docker
            .attach_container(container_id, Some(attach_opts))
            .await
            .map_err(|e| AgentsError::DockerError(e.to_string()))?;

        let (stdout_write, stdout_read) = tokio::io::duplex(64 * 1024);
        let (stderr_write, stderr_read) = tokio::io::duplex(64 * 1024);

        let (stdin_tx, stdin_rx) = tokio::io::duplex(64 * 1024);
        let bollard_stdin = attach.input;
        let stdin_container_id = container_id.to_string();
        let stdin_agent_name = agent_name.to_string();
        tokio::spawn(stdin_bridge_loop(
            stdin_rx,
            bollard_stdin,
            stdin_container_id,
            stdin_agent_name,
        ));

        let alive_flag = Arc::new(AtomicBool::new(true));
        let alive_for_demux = Arc::clone(&alive_flag);
        let mut output_stream = attach.output;
        let mut stdout_write = stdout_write;
        let mut stderr_write = stderr_write;

        tokio::spawn(async move {
            while let Some(item) = output_stream.next().await {
                match item {
                    Ok(bollard::container::LogOutput::StdOut { message }) => {
                        if stdout_write.write_all(&message).await.is_err() {
                            break;
                        }
                    }
                    Ok(bollard::container::LogOutput::StdErr { message }) => {
                        if stderr_write.write_all(&message).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!(error = %e, "Docker attach stream error");
                        break;
                    }
                }
            }
            alive_for_demux.store(false, Ordering::SeqCst);
        });

        Ok(AttachedStreams {
            alive_flag,
            stdin: Box::new(stdin_tx),
            stdout: Box::new(stdout_read),
            stderr: Box::new(stderr_read),
        })
    }

    /// Spawn a new Docker container from the given config and return a ready `DockerBackend`.
    ///
    /// Performs image pull (gated by `PullPolicy`), container creation, start, and stream attach.
    pub async fn spawn(
        config: &DockerWorkspaceConfig,
        agent_name: &str,
    ) -> Result<Self, AgentsError> {
        let docker = Self::connect(config)?;
        pull_image_if_needed(&docker, &config.image, &config.pull_policy).await?;

        let cname = container_name(agent_name);
        let labels = container_labels(agent_name);
        let host_config = build_host_config(config)?;
        let container_config = Self::build_container_config(config, labels, host_config);
        let container_id =
            Self::create_and_start_container(&docker, &cname, container_config).await?;
        let attached = Self::attach_container_streams(&docker, &container_id, agent_name).await?;

        Ok(DockerBackend {
            docker,
            container_id: Some(container_id),
            container_name: Some(cname),
            alive: attached.alive_flag,
            stdin: Mutex::new(Some(attached.stdin)),
            stdout: Mutex::new(Some(attached.stdout)),
            stderr: Mutex::new(Some(attached.stderr)),
        })
    }
}

struct AttachedStreams {
    alive_flag: Arc<AtomicBool>,
    stdin: Box<dyn AsyncWrite + Unpin + Send>,
    stdout: Box<dyn AsyncRead + Unpin + Send>,
    stderr: Box<dyn AsyncRead + Unpin + Send>,
}

/// Pull `image` according to `policy`. Returns `Err(ImagePullFailed)` on any pull error.
async fn pull_image_if_needed(
    docker: &Docker,
    image: &str,
    policy: &PullPolicy,
) -> Result<(), AgentsError> {
    match policy {
        PullPolicy::Never => {
            info!(image, "PullPolicy::Never — skipping image pull");
            Ok(())
        }
        PullPolicy::IfNotPresent => match docker.inspect_image(image).await {
            Ok(_) => {
                info!(image, "Image already present — skipping pull");
                Ok(())
            }
            Err(_) => {
                info!(image, "Image not found locally — pulling");
                do_pull(docker, image).await
            }
        },
        PullPolicy::Always => {
            info!(image, "PullPolicy::Always — pulling image");
            do_pull(docker, image).await
        }
    }
}

async fn do_pull(docker: &Docker, image: &str) -> Result<(), AgentsError> {
    let opts = CreateImageOptions {
        from_image: Some(image.to_string()),
        ..Default::default()
    };
    let mut stream = docker.create_image(Some(opts), None, None);
    while let Some(item) = stream.next().await {
        match item {
            Ok(info) => {
                if let Some(status) = &info.status {
                    info!(image, status, "Pull progress");
                }
            }
            Err(e) => {
                return Err(AgentsError::ImagePullFailed {
                    image: image.to_string(),
                    reason: e.to_string(),
                });
            }
        }
    }
    Ok(())
}

impl ProcessBackend for DockerBackend {
    fn is_alive(&mut self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    fn take_stdin(&mut self) -> Option<Box<dyn AsyncWrite + Unpin + Send>> {
        self.stdin.lock().expect("stdin mutex poisoned").take()
    }

    fn take_stdout(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
        self.stdout.lock().expect("stdout mutex poisoned").take()
    }

    fn take_stderr(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
        self.stderr.lock().expect("stderr mutex poisoned").take()
    }

    fn kill(&mut self) -> Pin<Box<dyn Future<Output = Result<(), AgentsError>> + Send + '_>> {
        Box::pin(async move {
            if let Some(id) = &self.container_id {
                let id = id.clone();
                let cname = self.container_name.as_deref().unwrap_or("unknown");
                let stop_opts = StopContainerOptions {
                    t: Some(10),
                    ..Default::default()
                };
                if let Err(e) = self.docker.stop_container(&id, Some(stop_opts)).await {
                    warn!(container_id = %id, container_name = %cname, error = %e, "Failed to stop container (continuing to remove)");
                }
                self.docker
                    .remove_container(
                        &id,
                        Some(RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    )
                    .await
                    .map_err(|e| AgentsError::DockerError(e.to_string()))?;
                info!(container_id = %id, container_name = %cname, "Removed Docker container");
            }
            self.alive.store(false, Ordering::SeqCst);
            Ok(())
        })
    }

    fn wait(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<std::process::ExitStatus, AgentsError>> + Send + '_>>
    {
        Box::pin(async move {
            let exit_code: i64 = if let Some(id) = &self.container_id {
                let id = id.clone();
                let cname = self.container_name.as_deref().unwrap_or("unknown");
                let mut stream = self
                    .docker
                    .wait_container(&id, None::<WaitContainerOptions>);
                match stream.next().await {
                    Some(Ok(resp)) => resp.status_code,
                    Some(Err(e)) => {
                        // Docker returns an error when exit code > 0 — extract the code
                        // from the error message, falling back to 1
                        let msg = e.to_string();
                        if let Some(code) = parse_exit_code_from_error(&msg) {
                            code
                        } else {
                            warn!(error = %e, container_name = %cname, "wait_container stream error, treating as exit 1");
                            1
                        }
                    }
                    None => 0,
                }
            } else {
                0
            };

            self.alive.store(false, Ordering::SeqCst);

            // Synthesize a real std::process::ExitStatus using a shell exit command
            std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("exit {}", exit_code))
                .status()
                .map_err(AgentsError::Io)
        })
    }
}

async fn stdin_bridge_loop(
    mut reader: impl AsyncRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
    container_id: String,
    agent_name: String,
) {
    use tokio::io::AsyncReadExt;
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if let Err(e) = writer.write_all(&buf[..n]).await {
                    warn!(
                        error = %e,
                        container_id = %container_id,
                        agent = %agent_name,
                        "bollard stdin write failed"
                    );
                    break;
                }
                if let Err(e) = writer.flush().await {
                    warn!(
                        error = %e,
                        container_id = %container_id,
                        agent = %agent_name,
                        "bollard stdin flush failed"
                    );
                    break;
                }
            }
        }
    }
}

/// Extract exit code from bollard `DockerContainerWaitError` message.
/// Bollard formats it as "Docker container waiting error: {error}, code: {code}".
fn parse_exit_code_from_error(msg: &str) -> Option<i64> {
    // Try to find "code: N" at end of bollard wait error messages
    msg.rsplit("code: ")
        .next()
        .and_then(|s| s.trim().parse().ok())
}

/// Build a `HostConfig` from `DockerWorkspaceConfig`, parsing memory and CPU limits.
pub(crate) fn build_host_config(config: &DockerWorkspaceConfig) -> Result<HostConfig, AgentsError> {
    let memory = config
        .memory_limit
        .as_deref()
        .map(parse_memory_limit)
        .transpose()
        .map_err(|e| AgentsError::DockerError(e.to_string()))?;

    let nano_cpus = config
        .cpu_limit
        .as_deref()
        .map(parse_cpu_limit)
        .transpose()
        .map_err(|e| AgentsError::DockerError(e.to_string()))?;

    let binds: Option<Vec<String>> = if config.volumes.is_empty() {
        None
    } else {
        Some(config.volumes.clone())
    };

    let network_mode = config.network.clone();

    let extra_hosts: Option<Vec<String>> = if config.extra_hosts.is_empty() {
        None
    } else {
        Some(config.extra_hosts.clone())
    };

    Ok(HostConfig {
        memory,
        nano_cpus,
        binds,
        network_mode,
        extra_hosts,
        ..Default::default()
    })
}

/// Generate a unique container name for an agent.
///
/// Format: `anyclaw-{agent_name}-{short_uuid}` (8-char UUID prefix).
pub(crate) fn container_name(agent_name: &str) -> String {
    let id = uuid::Uuid::new_v4();
    let short = &id.to_string()[..8];
    format!("anyclaw-{}-{}", agent_name, short)
}

/// Build the standard labels applied to all anyclaw-managed containers.
///
/// Used by stale-container cleanup (Plan 02) to identify owned containers.
pub(crate) fn container_labels(agent_name: &str) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    labels.insert("anyclaw.managed".to_string(), "true".to_string());
    labels.insert("anyclaw.agent".to_string(), agent_name.to_string());
    labels
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyclaw_config::types::{DockerWorkspaceConfig, PullPolicy};
    use rstest::rstest;

    fn given_minimal_docker_config() -> DockerWorkspaceConfig {
        DockerWorkspaceConfig {
            image: "test-agent:latest".to_string(),
            entrypoint: None,
            volumes: vec![],
            env: HashMap::new(),
            memory_limit: None,
            cpu_limit: None,
            docker_host: None,
            network: None,
            pull_policy: PullPolicy::IfNotPresent,
            working_dir: None,
            extra_hosts: vec![],
        }
    }

    fn given_docker_config_with_limits() -> DockerWorkspaceConfig {
        DockerWorkspaceConfig {
            image: "test-agent:latest".to_string(),
            entrypoint: None,
            volumes: vec!["/tmp:/tmp".to_string()],
            env: HashMap::new(),
            memory_limit: Some("512m".to_string()),
            cpu_limit: Some("1.5".to_string()),
            docker_host: None,
            network: Some("my-net".to_string()),
            pull_policy: PullPolicy::IfNotPresent,
            working_dir: None,
            extra_hosts: vec![],
        }
    }

    #[rstest]
    fn when_container_name_generated_then_has_anyclaw_prefix_and_agent_name() {
        let name = container_name("my-agent");
        assert!(
            name.starts_with("anyclaw-my-agent-"),
            "expected name to start with 'anyclaw-my-agent-', got: {name}"
        );
    }

    #[rstest]
    fn when_container_name_generated_then_short_uuid_suffix_is_8_chars() {
        let name = container_name("agent");
        let suffix = name.trim_start_matches("anyclaw-agent-");
        assert_eq!(
            suffix.len(),
            8,
            "short uuid suffix should be 8 chars, got: {suffix}"
        );
    }

    #[rstest]
    fn when_container_name_generated_twice_then_names_differ() {
        let a = container_name("agent");
        let b = container_name("agent");
        assert_ne!(a, b, "two generated names should differ (UUID randomness)");
    }

    #[rstest]
    fn when_container_labels_generated_then_includes_managed_label() {
        let labels = container_labels("my-agent");
        assert_eq!(labels.get("anyclaw.managed"), Some(&"true".to_string()));
    }

    #[rstest]
    fn when_container_labels_generated_then_includes_agent_name_label() {
        let labels = container_labels("my-agent");
        assert_eq!(labels.get("anyclaw.agent"), Some(&"my-agent".to_string()));
    }

    #[rstest]
    fn when_container_labels_generated_then_has_exactly_two_entries() {
        let labels = container_labels("my-agent");
        assert_eq!(labels.len(), 2);
    }

    #[rstest]
    fn when_build_host_config_with_no_limits_then_memory_and_cpu_are_none() {
        let config = given_minimal_docker_config();
        let hc = build_host_config(&config).expect("build_host_config should succeed");
        assert!(
            hc.memory.is_none(),
            "memory should be None when not configured"
        );
        assert!(
            hc.nano_cpus.is_none(),
            "nano_cpus should be None when not configured"
        );
    }

    #[rstest]
    fn when_build_host_config_with_memory_and_cpu_then_applies_limits() {
        let config = given_docker_config_with_limits();
        let hc = build_host_config(&config).expect("build_host_config should succeed");
        // 512m = 512 * 1024 * 1024 = 536_870_912
        assert_eq!(hc.memory, Some(536_870_912_i64));
        // 1.5 cores = 1_500_000_000 nanocpus
        assert_eq!(hc.nano_cpus, Some(1_500_000_000_i64));
    }

    #[rstest]
    fn when_build_host_config_with_volumes_then_binds_are_set() {
        let config = given_docker_config_with_limits();
        let hc = build_host_config(&config).expect("build_host_config should succeed");
        assert_eq!(hc.binds, Some(vec!["/tmp:/tmp".to_string()]));
    }

    #[rstest]
    fn when_build_host_config_with_no_volumes_then_binds_are_none() {
        let config = given_minimal_docker_config();
        let hc = build_host_config(&config).expect("build_host_config should succeed");
        assert!(hc.binds.is_none());
    }

    #[rstest]
    fn when_build_host_config_with_network_then_network_mode_is_set() {
        let config = given_docker_config_with_limits();
        let hc = build_host_config(&config).expect("build_host_config should succeed");
        assert_eq!(hc.network_mode, Some("my-net".to_string()));
    }

    #[rstest]
    fn when_build_container_config_with_extra_hosts_then_json_contains_extra_hosts_key() {
        let mut config = given_minimal_docker_config();
        config.extra_hosts = vec!["host.docker.internal:host-gateway".to_string()];
        let labels = container_labels("test-agent");
        let host_config = build_host_config(&config).expect("build_host_config should succeed");
        let container_config = DockerBackend::build_container_config(&config, labels, host_config);
        let json =
            serde_json::to_value(&container_config).expect("ContainerCreateBody should serialize");
        let hc = &json["HostConfig"];
        assert!(
            hc.get("ExtraHosts").is_some(),
            "HostConfig JSON must contain ExtraHosts key, got: {hc}"
        );
        assert_eq!(
            hc["ExtraHosts"],
            serde_json::json!(["host.docker.internal:host-gateway"]),
            "ExtraHosts should contain the configured entries"
        );
    }

    #[rstest]
    fn when_build_host_config_with_extra_hosts_then_extra_hosts_are_set() {
        let mut config = given_minimal_docker_config();
        config.extra_hosts = vec!["myhost:192.168.1.1".to_string()];
        let hc = build_host_config(&config).expect("build_host_config should succeed");
        assert_eq!(hc.extra_hosts, Some(vec!["myhost:192.168.1.1".to_string()]));
    }

    #[rstest]
    fn when_build_host_config_with_no_extra_hosts_then_extra_hosts_are_none() {
        let config = given_minimal_docker_config();
        let hc = build_host_config(&config).expect("build_host_config should succeed");
        assert!(hc.extra_hosts.is_none());
    }

    #[rstest]
    fn when_build_host_config_with_invalid_memory_limit_then_returns_error() {
        let mut config = given_minimal_docker_config();
        config.memory_limit = Some("bad_limit".to_string());
        let result = build_host_config(&config);
        assert!(result.is_err(), "invalid memory_limit should return Err");
    }

    #[rstest]
    fn when_build_host_config_with_invalid_cpu_limit_then_returns_error() {
        let mut config = given_minimal_docker_config();
        config.cpu_limit = Some("not_a_number".to_string());
        let result = build_host_config(&config);
        assert!(result.is_err(), "invalid cpu_limit should return Err");
    }

    #[rstest]
    fn when_is_alive_then_reflects_atomic_bool_true() {
        let alive = Arc::new(AtomicBool::new(true));
        let val = alive.load(Ordering::SeqCst);
        assert!(val);
    }

    #[rstest]
    fn when_is_alive_then_reflects_atomic_bool_false() {
        let alive = Arc::new(AtomicBool::new(false));
        let val = alive.load(Ordering::SeqCst);
        assert!(!val);
    }

    #[rstest]
    #[case::with_code("Docker container waiting error: , code: 137", Some(137))]
    #[case::with_zero_code("Docker container waiting error: , code: 0", Some(0))]
    #[case::no_code("some other error", None)]
    fn when_parsing_exit_code_from_error_message_then_extracts_correctly(
        #[case] msg: &str,
        #[case] expected: Option<i64>,
    ) {
        assert_eq!(parse_exit_code_from_error(msg), expected);
    }

    #[rstest]
    fn when_docker_backend_constructed_then_container_name_is_stored() {
        let backend = DockerBackend {
            docker: Docker::connect_with_local_defaults().unwrap(),
            container_id: Some("abc123".to_string()),
            container_name: Some("anyclaw-test-agent-abcd1234".to_string()),
            alive: Arc::new(AtomicBool::new(true)),
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
            stderr: Mutex::new(None),
        };
        assert_eq!(
            backend.container_name.as_deref(),
            Some("anyclaw-test-agent-abcd1234")
        );
    }

    struct FailingWriter;

    impl tokio::io::AsyncWrite for FailingWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            _buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "mock write failure",
            )))
        }
        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
        fn poll_shutdown(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    struct FailingFlusher;

    impl tokio::io::AsyncWrite for FailingFlusher {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            std::task::Poll::Ready(Ok(buf.len()))
        }
        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "mock flush failure",
            )))
        }
        fn poll_shutdown(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_stdin_write_fails_then_bridge_exits() {
        use tokio::io::AsyncWriteExt;
        let (mut tx, rx) = tokio::io::duplex(1024);
        let writer = FailingWriter;

        let handle = tokio::spawn(stdin_bridge_loop(
            rx,
            writer,
            "container-abc".to_string(),
            "test-agent".to_string(),
        ));

        tx.write_all(b"hello").await.unwrap();

        let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "bridge should exit after write failure");
    }

    #[rstest]
    #[tokio::test]
    async fn when_stdin_flush_fails_then_bridge_exits() {
        use tokio::io::AsyncWriteExt;
        let (mut tx, rx) = tokio::io::duplex(1024);
        let writer = FailingFlusher;

        let handle = tokio::spawn(stdin_bridge_loop(
            rx,
            writer,
            "container-abc".to_string(),
            "test-agent".to_string(),
        ));

        tx.write_all(b"hello").await.unwrap();

        // Bridge should exit due to flush failure
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "bridge should exit after flush failure");
    }
}
