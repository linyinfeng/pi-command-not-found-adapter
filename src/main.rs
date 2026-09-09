mod ask;
mod cli;
mod exec;
mod markdown;
mod pi;
mod prompt;
mod protocol;
mod session;
mod signals;
mod ui;

use std::env;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::cli::Args;
use crate::protocol::Input;
use crate::ui::Ui;

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(status) => ExitCode::from(status.clamp(0, 255) as u8),
        Err(error) => {
            eprintln!("command-not-found: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn run(args: &Args) -> Result<i32> {
    signals::install()?;
    let session = session::Session::resolve(args)?;
    let input = Input {
        session_id: session.id.clone(),
        cwd: env::current_dir()
            .context("cannot read the current directory")?
            .display()
            .to_string(),
        input: args.input.join(" "),
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
    markdown::render(&answer.markdown, args, &mut ui);
    let history = session.start_history(&input.input, &answer.markdown, &answer.command);
    if answer.command.trim().is_empty() {
        session::write_status(history.as_deref(), 0);
        return Ok(0);
    }
    if !answer.markdown.trim().is_empty() {
        ui.line("");
    }
    ui.announce(&answer.command);
    ui.rule();
    if args.dry_run {
        session::write_status(history.as_deref(), 0);
        return Ok(0);
    }
    let status = exec::run(history.as_deref(), &answer.command)?;
    session::write_status(history.as_deref(), status);
    Ok(status)
}
