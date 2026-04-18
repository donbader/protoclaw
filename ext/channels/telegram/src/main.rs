// D-03: channel.rs uses Value for handle_unknown params/return (unknown methods have no schema)
// and for ChannelInitializeParams.options (channel-specific config with channel-defined schemas)
mod channel;
// D-03: deliver.rs processes DeliverMessage.content (Value) — agent-defined content structure
mod access_control;
mod deliver;
mod dispatcher;
mod formatting;
mod peer;
mod permissions;
mod state;
mod turn;

use std::sync::Arc;

use anyclaw_sdk_channel::ChannelHarness;

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
