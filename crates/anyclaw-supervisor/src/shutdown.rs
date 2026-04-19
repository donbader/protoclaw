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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyclaw_core::{CrashTracker, ExponentialBackoff, SlotLifecycle};
    use rstest::rstest;
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn test_config() -> anyclaw_config::AnyclawConfig {
        anyclaw_config::AnyclawConfig {
            agents_manager: anyclaw_config::AgentsManagerConfig::default(),
            channels_manager: anyclaw_config::ChannelsManagerConfig::default(),
            tools_manager: anyclaw_config::ToolsManagerConfig::default(),
            supervisor: anyclaw_config::SupervisorConfig {
                shutdown_timeout_secs: 3,
                health_check_interval_secs: 60,
                ..Default::default()
            },
            log_level: "info".into(),
            log_format: anyclaw_config::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            session_store: Default::default(),
        }
    }

    fn given_slot(name: &str, cancel: &CancellationToken) -> ManagerSlot {
        ManagerSlot {
            name: name.to_string(),
            join_handle: None,
            lifecycle: SlotLifecycle::new(
                cancel,
                ExponentialBackoff::default(),
                CrashTracker::default(),
            ),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_slot_shuts_down_cleanly_then_completes() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());

        let mut slot = given_slot("alpha", &cancel);
        let handle = tokio::spawn(async { Ok::<(), anyclaw_core::ManagerError>(()) });
        slot.join_handle = Some(handle);

        let mut slots = vec![slot];
        sup.shutdown_ordered(&mut slots, Duration::from_secs(1))
            .await;

        assert!(slots[0].join_handle.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn when_slot_returns_error_then_shutdown_continues() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());

        let mut slot = given_slot("alpha", &cancel);
        let handle = tokio::spawn(async {
            Err::<(), anyclaw_core::ManagerError>(anyclaw_core::ManagerError::Internal(
                "oops".into(),
            ))
        });
        slot.join_handle = Some(handle);

        let mut slots = vec![slot];
        sup.shutdown_ordered(&mut slots, Duration::from_secs(1))
            .await;

        assert!(slots[0].join_handle.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn when_slot_panics_then_shutdown_continues() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());

        let mut slot = given_slot("alpha", &cancel);
        let handle = tokio::spawn(async {
            panic!("boom");
            #[allow(unreachable_code)]
            Ok::<(), anyclaw_core::ManagerError>(())
        });
        slot.join_handle = Some(handle);

        tokio::time::sleep(Duration::from_millis(20)).await;

        let mut slots = vec![slot];
        sup.shutdown_ordered(&mut slots, Duration::from_secs(1))
            .await;

        assert!(slots[0].join_handle.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn when_slot_times_out_then_aborted_within_timeout() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());

        let mut slot = given_slot("stuck", &cancel);
        let handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok::<(), anyclaw_core::ManagerError>(())
        });
        slot.join_handle = Some(handle);

        let mut slots = vec![slot];
        let start = tokio::time::Instant::now();
        sup.shutdown_ordered(&mut slots, Duration::from_millis(1))
            .await;

        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[rstest]
    #[tokio::test]
    async fn when_multiple_slots_present_then_processed_in_reverse_order() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());
        let order: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let names = ["first", "second", "third"];
        let mut slots: Vec<ManagerSlot> = names
            .iter()
            .map(|&name| {
                let lifecycle = SlotLifecycle::new(
                    &cancel,
                    ExponentialBackoff::default(),
                    CrashTracker::default(),
                );
                let token = lifecycle.cancel_token.clone();
                let captured_order = order.clone();
                let captured_name = name.to_string();
                let handle = tokio::spawn(async move {
                    token.cancelled().await;
                    captured_order.lock().unwrap().push(captured_name);
                    Ok::<(), anyclaw_core::ManagerError>(())
                });
                ManagerSlot {
                    name: name.to_string(),
                    join_handle: Some(handle),
                    lifecycle,
                }
            })
            .collect();

        sup.shutdown_ordered(&mut slots, Duration::from_secs(1))
            .await;

        let observed = order.lock().unwrap().clone();
        assert_eq!(observed, vec!["third", "second", "first"]);
    }

    #[rstest]
    #[tokio::test]
    async fn when_slot_has_no_join_handle_then_skipped() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());

        let slot = given_slot("empty", &cancel);
        assert!(slot.join_handle.is_none());

        let mut slots = vec![slot];
        sup.shutdown_ordered(&mut slots, Duration::from_secs(1))
            .await;

        assert!(slots[0].join_handle.is_none());
    }
}
