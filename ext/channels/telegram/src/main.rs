mod channel;
mod deliver;
mod dispatcher;
mod peer;
mod permissions;
mod state;

use std::sync::Arc;

use protoclaw_sdk_channel::ChannelHarness;
use teloxide::Bot;

use crate::channel::TelegramChannel;
use crate::state::SharedState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .or_else(|_| {
                    std::env::var("LOG_LEVEL").map(|level| {
                        // Scope noisy third-party crates to warn when using LOG_LEVEL
                        let filter = format!("hyper=warn,h2=warn,reqwest=warn,tower=warn,{level}");
                        tracing_subscriber::EnvFilter::new(filter)
                    })
                })
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let token = std::env::var("TELEGRAM_BOT_TOKEN")
        .expect("TELEGRAM_BOT_TOKEN environment variable must be set");
    let bot = Bot::new(token);
    let state = Arc::new(SharedState::new());
    let channel = TelegramChannel::new(state, bot);

    if let Err(e) = ChannelHarness::new(channel).run_stdio().await {
        tracing::error!(%e, "telegram channel harness exited with error");
    }
}
