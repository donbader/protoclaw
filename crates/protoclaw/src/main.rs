use anyhow::Result;
use clap::Parser;

mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = cli::Cli::parse();
    tracing::info!(config_path = %cli.config, "protoclaw starting");

    let config = protoclaw_config::ProtoclawConfig::load(Some(&cli.config))
        .map_err(|e| anyhow::anyhow!("failed to load config: {e}"))?;

    tracing::info!(agent = %config.agent.binary, channels = config.channels.len(), "config loaded");

    protoclaw::supervisor::Supervisor::new(config).run().await?;

    tracing::info!("protoclaw shut down");
    Ok(())
}
