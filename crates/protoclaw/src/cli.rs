use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "protoclaw", about = "Infrastructure sidecar for AI agents")]
pub struct Cli {
    /// Path to configuration file
    #[arg(
        short,
        long,
        default_value = "protoclaw.toml",
        env = "PROTOCLAW_CONFIG"
    )]
    pub config: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_config_flag() {
        let cli = Cli::parse_from(["protoclaw", "--config", "custom.toml"]);
        assert_eq!(cli.config, "custom.toml");
    }

    #[test]
    fn cli_default_config_is_protoclaw_toml() {
        let cli = Cli::parse_from(["protoclaw"]);
        assert_eq!(cli.config, "protoclaw.toml");
    }

    #[test]
    fn cli_parses_short_config_flag() {
        let cli = Cli::parse_from(["protoclaw", "-c", "short.toml"]);
        assert_eq!(cli.config, "short.toml");
    }
}
