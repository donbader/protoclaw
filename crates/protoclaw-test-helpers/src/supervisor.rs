use tokio_util::sync::CancellationToken;

pub async fn boot_supervisor_with_port(
    config: protoclaw_config::ProtoclawConfig,
) -> (CancellationToken, tokio::task::JoinHandle<anyhow::Result<()>>, u16) {
    let cancel = CancellationToken::new();
    let sup = protoclaw_supervisor::Supervisor::new(config);
    let port_rx = sup.debug_http_port_rx();
    let c = cancel.clone();
    let handle = tokio::spawn(async move { sup.run_with_cancel(c).await });
    let port = crate::wait_for_port(port_rx, 10000)
        .await
        .expect("debug-http port not discovered");
    (cancel, handle, port)
}
