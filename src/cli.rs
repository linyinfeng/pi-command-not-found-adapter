use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "command-not-found-agent",
    version,
    about = "Ask pi for a command when the shell cannot find one, then run it"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ask pi for a command and run it
    Run(Box<Run>),
    /// Print a new session id, for a shell to export
    SessionId,
}

#[derive(Debug, Args)]
pub struct Run {
    /// The command line the user typed, after `--`
    #[arg(last = true, required = true, num_args = 1.., allow_hyphen_values = true)]
    pub input: Vec<String>,

    /// pi executable to drive
    #[arg(long, env = "COMMAND_NOT_FOUND_PI", default_value = "pi")]
    pub pi: String,

    /// Model passed to pi as --model (provider/id, optional :thinking)
    #[arg(long, env = "COMMAND_NOT_FOUND_MODEL")]
    pub model: Option<String>,

    /// Thinking level passed to pi as --thinking
    #[arg(long, env = "COMMAND_NOT_FOUND_THINKING")]
    pub thinking: Option<String>,

    /// Extra argument for pi; repeat for several
    #[arg(
        long = "pi-arg",
        env = "COMMAND_NOT_FOUND_PI_ARGS",
        value_delimiter = '\n'
    )]
    pub pi_args: Vec<String>,

    /// Session id; defaults to a random UUID
    #[arg(long, env = "COMMAND_NOT_FOUND_SESSION_ID")]
    pub session_id: Option<String>,

    /// Shell that will source the answer
    #[arg(long, env = "COMMAND_NOT_FOUND_SHELL", required = true)]
    pub shell: String,

    /// Replace the built-in base system prompt with this file
    #[arg(long, env = "COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE")]
    pub system_prompt_file: Option<PathBuf>,

    /// Directory holding the per-session directories
    #[arg(long, env = "COMMAND_NOT_FOUND_SESSION_ROOT")]
    pub session_root: Option<PathBuf>,

    /// mdcat executable used to render the note
    #[arg(long, env = "COMMAND_NOT_FOUND_MDCAT", default_value = "mdcat")]
    pub mdcat: String,

    /// Wrap width; defaults to the terminal width
    #[arg(long, env = "COMMAND_NOT_FOUND_WIDTH")]
    pub width: Option<usize>,

    /// Extra pi turns when the answer does not parse
    #[arg(long, env = "COMMAND_NOT_FOUND_RETRIES", default_value_t = 2)]
    pub retries: u32,

    /// Tool call lines kept in the progress block
    #[arg(long, env = "COMMAND_NOT_FOUND_TOOL_LINES", default_value_t = 5)]
    pub tool_lines: usize,

    /// Seconds allowed per pi turn
    #[arg(long, env = "COMMAND_NOT_FOUND_TIMEOUT", default_value_t = 600)]
    pub timeout: u64,

    /// Append every raw pi protocol line to this file
    #[arg(long, env = "COMMAND_NOT_FOUND_TRACE")]
    pub trace: Option<PathBuf>,
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
    fn session_id_takes_no_arguments() {
        let cli = Cli::parse_from(["agent", "session-id"]);
        assert!(matches!(cli.command, Command::SessionId));
    }
}
