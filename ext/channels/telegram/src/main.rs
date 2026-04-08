mod channel;
mod deliver;
mod dispatcher;
mod peer;
mod permissions;
mod state;
mod turn;

use std::sync::Arc;

use protoclaw_sdk_channel::ChannelHarness;

use crate::channel::TelegramChannel;
use crate::state::SharedState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let state = Arc::new(SharedState::new());
    let channel = TelegramChannel::new(state);

    if let Err(e) = ChannelHarness::new(channel).run_stdio().await {
        tracing::error!(%e, "telegram channel harness exited with error");
    }
}
