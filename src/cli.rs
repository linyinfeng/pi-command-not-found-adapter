use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "command-not-found-agent",
    version,
    about = "Answer a command the shell could not find with code it can source",
    after_help = "Settings (the model, the prompts, the timeouts, …) are not command line options:\n$XDG_CONFIG_HOME/pi-command-not-found/config.json and the PI_COMMAND_NOT_FOUND_*\nvariables carry them, so a shell only has to run `run --shell <shell>`."
)]
pub struct Cli {
    /// Merge one more config file over the XDG ones
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ask pi for the missing command and print the shell code that runs it
    Run(Box<Run>),
    /// Print a new session id, for a shell to export
    SessionId,
    /// Print the system prompt the handler sends to pi
    SystemPrompt,
    /// Print the settings the adapter resolved from every layer
    Config,
}

/// What the caller has to say per invocation; everything else is a setting.
#[derive(Debug, Args)]
pub struct Run {
    /// The command line the user typed, after `--`
    #[arg(last = true, required = true, num_args = 1.., allow_hyphen_values = true)]
    pub input: Vec<String>,

    /// Shell that will source the answer, e.g. bash, zsh, fish, nu
    #[arg(long, env = "PI_COMMAND_NOT_FOUND_SHELL", required = true)]
    pub shell: String,

    /// Session id; defaults to a random UUID
    #[arg(long, env = "PI_COMMAND_NOT_FOUND_SESSION_ID")]
    pub session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_takes_the_command_after_a_double_dash() {
        let cli = Cli::parse_from(["agent", "run", "--shell", "bash", "--", "cowsay", "--help"]);
        let Command::Run(run) = cli.command else {
            panic!("expected run");
        };
        assert_eq!(run.input, ["cowsay", "--help"]);
        assert_eq!(run.shell, "bash");
    }

    #[test]
    fn run_rejects_the_command_without_the_separator() {
        assert!(Cli::try_parse_from(["agent", "run", "--shell", "bash", "cowsay"]).is_err());
    }

    #[test]
    fn run_needs_a_shell() {
        assert!(Cli::try_parse_from(["agent", "run", "--", "cowsay"]).is_err());
    }

    #[test]
    fn config_takes_no_arguments() {
        let cli = Cli::parse_from(["agent", "config"]);
        assert!(matches!(cli.command, Command::Config));
    }

    #[test]
    fn session_id_takes_no_arguments() {
        let cli = Cli::parse_from(["agent", "session-id"]);
        assert!(matches!(cli.command, Command::SessionId));
    }

    #[test]
    fn config_can_come_before_or_after_the_subcommand() {
        for argv in [
            [
                "agent",
                "--config",
                "extra.json",
                "run",
                "--shell",
                "bash",
                "--",
                "ls",
            ],
            [
                "agent",
                "run",
                "--config",
                "extra.json",
                "--shell",
                "bash",
                "--",
                "ls",
            ],
        ] {
            let cli = Cli::parse_from(argv);
            assert_eq!(
                cli.config.as_deref(),
                Some(std::path::Path::new("extra.json"))
            );
        }
    }

    #[test]
    fn settings_are_not_options() {
        // They come from the config file or the environment, never from argv.
        let setting = Cli::try_parse_from([
            "agent", "run", "--shell", "bash", "--model", "x", "--", "cowsay",
        ]);
        assert!(setting.is_err(), "--model is not an option any more");
        let prompt =
            Cli::try_parse_from(["agent", "system-prompt", "--system-prompt-file", "a.md"]);
        assert!(prompt.is_err(), "prompt files are a setting too");
    }
}
