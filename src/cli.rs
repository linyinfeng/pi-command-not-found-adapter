use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "command-not-found-agent",
    version,
    about = "Ask pi for a command when the shell cannot find one, then run it"
)]
pub struct Args {
    /// The command line the user typed
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 1..)]
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

    /// Print the command instead of running it
    #[arg(long)]
    pub dry_run: bool,

    /// Append every raw pi protocol line to this file
    #[arg(long, env = "COMMAND_NOT_FOUND_TRACE")]
    pub trace: Option<PathBuf>,
}
