use std::fs;

use anyhow::{Context, Result};
use schemars::{JsonSchema, schema_for};

use crate::cli::Run;
use crate::protocol::{Answer, Input};
use crate::session::Session;

/// Used when no `--system-prompt-file` is given.
const BASE: &str = include_str!("../prompts/base.md");
/// Maintained here because it describes the adapter's own state.
const HISTORY: &str = include_str!("../prompts/history.md");

fn schema<T: JsonSchema>() -> Result<String> {
    Ok(serde_json::to_string_pretty(&schema_for!(T))?)
}

/// The base prompts in the order given (or the built-in one), then the
/// adapter-owned context, then the generated schemas.
pub fn system_prompt(args: &Run, session: &Session) -> Result<String> {
    let base = base_prompt(args)?;
    let history = HISTORY
        .replace(
            "{session_file}",
            &session.session_file().display().to_string(),
        )
        .replace("{state_dir}", &session.state.display().to_string());
    let input = schema::<Input>()?;
    let answer = schema::<Answer>()?;
    Ok(format!(
        "{base}\n{history}\n## Input schema\n\n```json\n{input}\n```\n\n\
         ## Answer schema\n\n```json\n{answer}\n```\n"
    ))
}

fn base_prompt(args: &Run) -> Result<String> {
    if args.system_prompt_files.is_empty() {
        return Ok(BASE.trim_end().to_string());
    }
    let mut parts = Vec::new();
    for path in &args.system_prompt_files {
        let text =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        parts.push(text.trim_end().to_string());
    }
    Ok(parts.join("\n\n"))
}

/// Sent as the next user message when the previous answer did not parse.
pub fn retry_prompt(reason: &str) -> String {
    format!(
        "Your previous message did not contain a valid answer JSON object \
         ({reason}). Reply with exactly one JSON object matching the answer \
         schema, and nothing else."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command};
    use clap::Parser;

    fn args() -> Run {
        let cli = Cli::parse_from(["agent", "run", "--shell", "bash", "--", "ls"]);
        let Command::Run(run) = cli.command else {
            panic!("expected run");
        };
        *run
    }

    fn session(root: &str) -> Session {
        Session {
            id: "abc".into(),
            state: std::path::PathBuf::from(root),
            dir: std::path::PathBuf::from(root).join("sessions/abc"),
        }
    }

    #[test]
    fn prompt_has_fixed_parts_and_both_schemas() {
        let prompt = system_prompt(&args(), &session("/state")).unwrap();
        assert!(prompt.contains("## Sessions"));
        assert!(prompt.contains("## Input schema"));
        assert!(prompt.contains("## Answer schema"));
        assert!(prompt.contains("\"markdown\""));
        assert!(prompt.contains("\"session_id\""));
    }

    #[test]
    fn prompt_names_the_real_paths() {
        let prompt = system_prompt(&args(), &session("/state")).unwrap();
        assert!(
            prompt.contains("/state/sessions/abc/session.jsonl"),
            "{prompt}"
        );
        assert!(prompt.contains("`/state` is your memory"), "{prompt}");
        assert!(!prompt.contains("{state_dir}"));
        assert!(!prompt.contains("{session_file}"));
    }

    #[test]
    fn external_prompt_replaces_the_base_only() {
        let path = temp("one", "# custom agent\n");
        let mut args = args();
        args.system_prompt_files = vec![path.clone()];
        let prompt = system_prompt(&args, &session("/state")).unwrap();
        let _ = fs::remove_file(&path);
        assert!(prompt.starts_with("# custom agent"));
        assert!(prompt.contains("## Sessions"));
        assert!(!prompt.contains("The shell's command-not-found handler"));
    }

    #[test]
    fn several_prompts_are_concatenated_in_order() {
        let first = temp("first", "# first\n");
        let second = temp("second", "# second\n");
        let mut args = args();
        args.system_prompt_files = vec![first.clone(), second.clone()];
        let prompt = system_prompt(&args, &session("/state")).unwrap();
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
        assert!(prompt.starts_with("# first\n\n# second"), "{prompt}");
        assert!(!prompt.contains("The shell's command-not-found handler"));
    }

    #[test]
    fn a_missing_prompt_file_is_an_error() {
        let mut args = args();
        args.system_prompt_files = vec!["/nonexistent-prompt".into()];
        assert!(system_prompt(&args, &session("/state")).is_err());
    }

    fn temp(name: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("prompt-test-{}-{name}", std::process::id()));
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn retry_prompt_mentions_reason() {
        assert!(retry_prompt("missing field").contains("missing field"));
    }
}
