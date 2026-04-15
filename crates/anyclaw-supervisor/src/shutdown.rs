use std::time::Duration;

use crate::ManagerSlot;
use crate::Supervisor;

pub(crate) async fn shutdown_signal() {
    use tokio::signal::unix::SignalKind;

    let mut sigterm =
        tokio::signal::unix::signal(SignalKind::terminate()).expect("failed to register SIGTERM");
    let mut sigint =
        tokio::signal::unix::signal(SignalKind::interrupt()).expect("failed to register SIGINT");

    tokio::select! {
        _ = sigterm.recv() => tracing::info!("received SIGTERM"),
        _ = sigint.recv() => tracing::info!("received SIGINT"),
    }
}

impl Supervisor {
    pub(crate) async fn shutdown_ordered(
        &self,
        slots: &mut [ManagerSlot],
        per_manager_timeout: Duration,
    ) {
        for slot in slots.iter_mut().rev() {
            tracing::info!(manager = %slot.name, "shutting down");
            slot.lifecycle.cancel_token.cancel();

            if let Some(handle) = slot.join_handle.take() {
                match tokio::time::timeout(per_manager_timeout, handle).await {
                    Ok(Ok(Ok(()))) => {
                        tracing::info!(manager = %slot.name, "shut down cleanly");
                    }
                    Ok(Ok(Err(e))) => {
                        tracing::error!(manager = %slot.name, error = %e, "error during shutdown");
                    }
                    Ok(Err(e)) => {
                        tracing::error!(manager = %slot.name, error = %e, "panicked during shutdown");
                    }
                    Err(_) => {
                        tracing::warn!(manager = %slot.name, "shutdown timed out, aborting");
                        slot.join_handle.as_ref().inspect(|h| h.abort());
                    }
                }
            }
        }
    }
}
