use std::fs;

use anyhow::{Context, Result};
use schemars::{JsonSchema, schema_for};

use crate::cli::Run;
use crate::protocol::{Answer, Input};
use crate::session::Session;

/// Replaced by `--system-prompt-file` when given.
const BASE: &str = include_str!("../prompts/base.md");
/// Maintained here because it describes the adapter's own state.
const HISTORY: &str = include_str!("../prompts/history.md");

fn schema<T: JsonSchema>() -> Result<String> {
    Ok(serde_json::to_string_pretty(&schema_for!(T))?)
}

/// Base prompt, then the adapter-owned context, then the generated schemas.
pub fn system_prompt(args: &Run, session: &Session) -> Result<String> {
    let base = match &args.system_prompt_file {
        Some(path) => {
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?
        }
        None => BASE.to_string(),
    };
    let history = HISTORY
        .replace(
            "{session_file}",
            &session.session_file().display().to_string(),
        )
        .replace("{state_dir}", &session.state.display().to_string());
    let input = schema::<Input>()?;
    let answer = schema::<Answer>()?;
    Ok(format!(
        "{}\n{history}\n## Input schema\n\n```json\n{input}\n```\n\n\
         ## Answer schema\n\n```json\n{answer}\n```\n",
        base.trim_end()
    ))
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
        let path = std::env::temp_dir().join(format!("prompt-test-{}", std::process::id()));
        fs::write(&path, "# custom agent\n").unwrap();
        let mut args = args();
        args.system_prompt_file = Some(path.clone());
        let prompt = system_prompt(&args, &session("/state")).unwrap();
        let _ = fs::remove_file(&path);
        assert!(prompt.starts_with("# custom agent"));
        assert!(prompt.contains("## Sessions"));
        assert!(!prompt.contains("The shell's command-not-found handler"));
    }

    #[test]
    fn retry_prompt_mentions_reason() {
        assert!(retry_prompt("missing field").contains("missing field"));
    }
}
