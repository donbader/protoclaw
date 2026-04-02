use anyhow::Result;
use clap::Parser;

mod banner;
mod cli;
mod init;
mod status;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = cli::Cli::parse();

    match cli.command {
        None | Some(cli::Commands::Run) => {
            tracing::info!(config_path = %cli.config, "protoclaw starting");
            let config = protoclaw_config::ProtoclawConfig::load(Some(&cli.config))
                .map_err(|e| anyhow::anyhow!("failed to load config: {e}"))?;
            tracing::info!(agent = %config.agent.binary, channels = config.channels.len(), "config loaded");
            print!("{}", banner::format_banner(&config, &cli.config));
            protoclaw::supervisor::Supervisor::new(config).run().await?;
            tracing::info!("protoclaw shut down");
        }
        Some(cli::Commands::Init { force }) => {
            init::run_init(&cli.config, force)?;
        }
        Some(cli::Commands::Validate) => {
            let config = protoclaw_config::ProtoclawConfig::load(Some(&cli.config))
                .map_err(|e| anyhow::anyhow!("failed to load config: {e}"))?;

            let result = protoclaw_config::validate_config(&config);

            for error in &result.errors {
                eprintln!("  \u{2717} {error}");
            }
            for warning in &result.warnings {
                eprintln!("  \u{26a0} {warning}");
            }

            if result.is_ok() {
                println!("\u{2713} Configuration valid: {}", cli.config);
            } else {
                eprintln!("\u{2717} Configuration has {} error(s)", result.errors.len());
                std::process::exit(1);
            }
        }
        Some(cli::Commands::Status { port }) => {
            if let Err(e) = status::run_status(port).await {
                eprintln!("\u{2717} {e}");
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
