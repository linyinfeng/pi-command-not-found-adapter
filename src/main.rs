mod ask;
mod cli;
mod markdown;
mod pi;
mod prompt;
mod protocol;
mod session;
mod signals;
mod ui;

use std::env;
use std::io::Write;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::cli::{Cli, Run};
use crate::protocol::Input;
use crate::ui::Ui;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .without_time()
        .with_target(false)
        .init();
    let result = match Cli::parse().command {
        cli::Command::SessionId => {
            println!("{}", session::random_id());
            Ok(0)
        }
        cli::Command::Run(args) => run(&args),
    };
    match result {
        Ok(status) => ExitCode::from(status.clamp(0, 255) as u8),
        Err(error) => {
            eprintln!("command-not-found: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn command_line(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Shell-quote one word so the logged input can be pasted back.
fn quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_alphanumeric() || "-_./:@%+=".contains(c));
    if plain {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

fn run(args: &Run) -> Result<i32> {
    signals::install()?;
    let session = session::Session::resolve(args)?;
    let command_line = command_line(&args.input);
    let input = Input {
        session_id: session.id.clone(),
        shell: args.shell.clone(),
        cwd: Some(
            env::current_dir()
                .context("cannot read the current directory")?
                .display()
                .to_string(),
        ),
        input: Some(command_line.clone()),
    };
    let mut ui = Ui::new(args.tool_lines, args.width);
    let system_prompt = prompt::system_prompt(args)?;
    let mut agent = pi::Agent::spawn(args, &session, &system_prompt)?;
    let answer = match agent.ask(&input, args.retries, &mut ui) {
        Ok(answer) => answer,
        Err(failure) => {
            if let Some(text) = &failure.last_text {
                markdown::render(text, args, &mut ui);
            }
            ui.clear();
            bail!("{failure}");
        }
    };
    let note = answer.markdown.unwrap_or_default();
    let source = answer.source.unwrap_or_default();
    markdown::render(&note, args, &mut ui);
    session.start_history(&command_line, &note, &source);
    if source.trim().is_empty() {
        return Ok(0);
    }
    if !note.trim().is_empty() {
        ui.line("");
    }
    ui.announce(&source);
    ui.rule();
    let mut stdout = std::io::stdout();
    writeln!(stdout, "{source}")?;
    stdout.flush()?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_what_needs_it() {
        assert_eq!(quote("ls"), "ls");
        assert_eq!(quote("a-b/c.d"), "a-b/c.d");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn joins_words() {
        let words = ["cowsay".to_string(), "hello world".to_string()];
        assert_eq!(command_line(&words), "cowsay 'hello world'");
    }
}
