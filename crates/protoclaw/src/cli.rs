use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "protoclaw",
    about = "Infrastructure sidecar for AI agents",
    version
)]
pub struct Cli {
    #[arg(
        short,
        long,
        default_value = "protoclaw.yaml",
        env = "PROTOCLAW_CONFIG",
        global = true
    )]
    pub config: String,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Run,
    Init {
        #[arg(long)]
        force: bool,
    },
    Validate,
    Status {
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_protoclaw_command_is_none() {
        let cli = Cli::parse_from(["protoclaw"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn run_subcommand_parses() {
        let cli = Cli::parse_from(["protoclaw", "run"]);
        assert!(matches!(cli.command, Some(Commands::Run)));
    }

    #[test]
    fn run_subcommand_with_config_flag() {
        let cli = Cli::parse_from(["protoclaw", "run", "--config", "custom.yaml"]);
        assert!(matches!(cli.command, Some(Commands::Run)));
        assert_eq!(cli.config, "custom.yaml");
    }

    #[test]
    fn global_config_flag_with_no_subcommand() {
        let cli = Cli::parse_from(["protoclaw", "--config", "x.yaml"]);
        assert!(cli.command.is_none());
        assert_eq!(cli.config, "x.yaml");
    }

    #[test]
    fn init_subcommand_defaults_force_false() {
        let cli = Cli::parse_from(["protoclaw", "init"]);
        assert!(matches!(cli.command, Some(Commands::Init { force: false })));
    }

    #[test]
    fn init_subcommand_with_force_flag() {
        let cli = Cli::parse_from(["protoclaw", "init", "--force"]);
        assert!(matches!(cli.command, Some(Commands::Init { force: true })));
    }

    #[test]
    fn validate_subcommand_parses() {
        let cli = Cli::parse_from(["protoclaw", "validate"]);
        assert!(matches!(cli.command, Some(Commands::Validate)));
    }

    #[test]
    fn status_subcommand_defaults_port_3000() {
        let cli = Cli::parse_from(["protoclaw", "status"]);
        assert!(matches!(cli.command, Some(Commands::Status { port: 3000 })));
    }

    #[test]
    fn status_subcommand_with_custom_port() {
        let cli = Cli::parse_from(["protoclaw", "status", "--port", "8080"]);
        assert!(matches!(cli.command, Some(Commands::Status { port: 8080 })));
    }
}
