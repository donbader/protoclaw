use clap::{Parser, Subcommand};

/// Top-level CLI arguments parsed by clap.
#[derive(Parser, Debug)]
#[command(
    name = "anyclaw",
    about = "Infrastructure sidecar for AI agents",
    version
)]
pub struct Cli {
    /// Path to the anyclaw YAML config file.
    #[arg(
        short,
        long,
        default_value = "anyclaw.yaml",
        env = "ANYCLAW_CONFIG",
        global = true
    )]
    pub config: String,

    /// Subcommand to execute (defaults to `run` if omitted).
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available CLI subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the supervisor and all configured managers.
    Run,
    /// Scaffold a default `anyclaw.yaml` config file.
    Init {
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },
    /// Validate the config file without starting the supervisor.
    Validate,
    /// Print the JSON Schema for anyclaw.yaml to stdout.
    Schema,
    /// Query the running supervisor's health endpoint.
    Status {
        /// Admin server port to connect to.
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_no_subcommand_given_then_command_is_none() {
        let cli = Cli::parse_from(["anyclaw"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn when_run_subcommand_given_then_command_is_run() {
        let cli = Cli::parse_from(["anyclaw", "run"]);
        assert!(matches!(cli.command, Some(Commands::Run)));
    }

    #[test]
    fn given_config_flag_when_run_subcommand_given_then_command_is_run_and_config_is_set() {
        let cli = Cli::parse_from(["anyclaw", "run", "--config", "custom.yaml"]);
        assert!(matches!(cli.command, Some(Commands::Run)));
        assert_eq!(cli.config, "custom.yaml");
    }

    #[test]
    fn given_config_flag_when_no_subcommand_then_config_is_set_and_command_is_none() {
        let cli = Cli::parse_from(["anyclaw", "--config", "x.yaml"]);
        assert!(cli.command.is_none());
        assert_eq!(cli.config, "x.yaml");
    }

    #[test]
    fn when_init_subcommand_given_without_force_then_force_is_false() {
        let cli = Cli::parse_from(["anyclaw", "init"]);
        assert!(matches!(cli.command, Some(Commands::Init { force: false })));
    }

    #[test]
    fn when_init_subcommand_given_with_force_flag_then_force_is_true() {
        let cli = Cli::parse_from(["anyclaw", "init", "--force"]);
        assert!(matches!(cli.command, Some(Commands::Init { force: true })));
    }

    #[test]
    fn when_validate_subcommand_given_then_command_is_validate() {
        let cli = Cli::parse_from(["anyclaw", "validate"]);
        assert!(matches!(cli.command, Some(Commands::Validate)));
    }

    #[test]
    fn when_schema_subcommand_given_then_command_is_schema() {
        let cli = Cli::parse_from(["anyclaw", "schema"]);
        assert!(matches!(cli.command, Some(Commands::Schema)));
    }

    #[test]
    fn when_schema_subcommand_given_without_config_flag_then_uses_default_config() {
        let cli = Cli::parse_from(["anyclaw", "schema"]);
        assert!(matches!(cli.command, Some(Commands::Schema)));
        assert_eq!(cli.config, "anyclaw.yaml");
    }

    #[test]
    fn when_status_subcommand_given_without_port_then_port_defaults_to_3000() {
        let cli = Cli::parse_from(["anyclaw", "status"]);
        assert!(matches!(cli.command, Some(Commands::Status { port: 3000 })));
    }

    #[test]
    fn when_status_subcommand_given_with_port_flag_then_port_is_set() {
        let cli = Cli::parse_from(["anyclaw", "status", "--port", "8080"]);
        assert!(matches!(cli.command, Some(Commands::Status { port: 8080 })));
    }
}
