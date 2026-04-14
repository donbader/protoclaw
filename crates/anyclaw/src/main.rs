use anyhow::Result;
use clap::Parser;

mod banner;
mod cli;
mod init;
// Grandfathered: typed replacement in Phase 2-4
#[allow(clippy::disallowed_types)]
mod status;

fn init_tracing(log_level: &str, log_format: &anyclaw_config::LogFormat) {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{EnvFilter, Registry, fmt};

    let filter = EnvFilter::new(log_level);
    match log_format {
        anyclaw_config::LogFormat::Json => {
            Registry::default()
                .with(filter)
                .with(fmt::layer().json())
                .init();
        }
        anyclaw_config::LogFormat::Pretty => {
            Registry::default().with(filter).with(fmt::layer()).init();
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        None | Some(cli::Commands::Run) => {
            let config = anyclaw_config::AnyclawConfig::load(Some(&cli.config))
                .map_err(|e| anyhow::anyhow!("failed to load config: {e}"))?;
            init_tracing(&config.log_level, &config.log_format);
            tracing::info!(config_path = %cli.config, "anyclaw starting");
            tracing::info!(
                agents = config.agents_manager.agents.len(),
                channels = config.channels_manager.channels.len(),
                "config loaded"
            );
            print!("{}", banner::format_banner(&config, &cli.config));
            anyclaw::supervisor::Supervisor::new(config).run().await?;
            tracing::info!("anyclaw shut down");
        }
        _ => {
            tracing_subscriber::fmt().init();
            match cli.command {
                Some(cli::Commands::Init { force }) => {
                    init::run_init(&cli.config, force)?;
                }
                Some(cli::Commands::Validate) => {
                    let config = anyclaw_config::AnyclawConfig::load(Some(&cli.config))
                        .map_err(|e| anyhow::anyhow!("failed to load config: {e}"))?;
                    let result = anyclaw_config::validate_config(&config);
                    for error in &result.errors {
                        eprintln!("  \u{2717} {error}");
                    }
                    for warning in &result.warnings {
                        eprintln!("  \u{26a0} {warning}");
                    }
                    if result.is_ok() {
                        println!("\u{2713} Configuration valid: {}", cli.config);
                    } else {
                        eprintln!(
                            "\u{2717} Configuration has {} error(s)",
                            result.errors.len()
                        );
                        std::process::exit(1);
                    }
                }
                Some(cli::Commands::Status { port }) => {
                    if let Err(e) = status::run_status(port).await {
                        eprintln!("\u{2717} {e}");
                        std::process::exit(1);
                    }
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(())
}
