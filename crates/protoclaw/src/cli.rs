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
    use rstest::rstest;

    #[test]
    fn when_no_subcommand_given_then_command_is_none() {
        let cli = Cli::parse_from(["protoclaw"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn when_run_subcommand_given_then_command_is_run() {
        let cli = Cli::parse_from(["protoclaw", "run"]);
        assert!(matches!(cli.command, Some(Commands::Run)));
    }

    #[test]
    fn given_config_flag_when_run_subcommand_given_then_command_is_run_and_config_is_set() {
        let cli = Cli::parse_from(["protoclaw", "run", "--config", "custom.yaml"]);
        assert!(matches!(cli.command, Some(Commands::Run)));
        assert_eq!(cli.config, "custom.yaml");
    }

    #[test]
    fn given_config_flag_when_no_subcommand_then_config_is_set_and_command_is_none() {
        let cli = Cli::parse_from(["protoclaw", "--config", "x.yaml"]);
        assert!(cli.command.is_none());
        assert_eq!(cli.config, "x.yaml");
    }

    #[test]
    fn when_init_subcommand_given_without_force_then_force_is_false() {
        let cli = Cli::parse_from(["protoclaw", "init"]);
        assert!(matches!(cli.command, Some(Commands::Init { force: false })));
    }

    #[test]
    fn when_init_subcommand_given_with_force_flag_then_force_is_true() {
        let cli = Cli::parse_from(["protoclaw", "init", "--force"]);
        assert!(matches!(cli.command, Some(Commands::Init { force: true })));
    }

    #[test]
    fn when_validate_subcommand_given_then_command_is_validate() {
        let cli = Cli::parse_from(["protoclaw", "validate"]);
        assert!(matches!(cli.command, Some(Commands::Validate)));
    }

    #[test]
    fn when_status_subcommand_given_without_port_then_port_defaults_to_3000() {
        let cli = Cli::parse_from(["protoclaw", "status"]);
        assert!(matches!(cli.command, Some(Commands::Status { port: 3000 })));
    }

    #[test]
    fn when_status_subcommand_given_with_port_flag_then_port_is_set() {
        let cli = Cli::parse_from(["protoclaw", "status", "--port", "8080"]);
        assert!(matches!(cli.command, Some(Commands::Status { port: 8080 })));
    }
}
