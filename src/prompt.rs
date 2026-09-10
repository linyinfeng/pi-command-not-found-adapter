use std::fs;

use anyhow::{Context, Result};
use schemars::{JsonSchema, schema_for};

use crate::cli::PromptArgs;
use crate::protocol::{Answer, Input};

/// Used when no `--system-prompt-file` is given.
const BASE: &str = include_str!("../prompts/base.md");
/// Maintained here because it describes the adapter's own state.
const HISTORY: &str = include_str!("../prompts/history.md");

fn schema<T: JsonSchema>() -> Result<String> {
    Ok(serde_json::to_string_pretty(&schema_for!(T))?)
}

/// The base prompts in the order given (or the built-in one), then the
/// adapter-owned context, then the generated schemas. Everything that
/// varies per invocation travels in the input instead, so this string only
/// depends on the files above and stays identical across runs.
pub fn system_prompt(args: &PromptArgs) -> Result<String> {
    let base = base_prompt(args)?;
    let input = schema::<Input>()?;
    let answer = schema::<Answer>()?;
    Ok(format!(
        "{base}\n{HISTORY}\n## Input schema\n\n```json\n{input}\n```\n\n\
         ## Answer schema\n\n```json\n{answer}\n```\n"
    ))
}

fn base_prompt(args: &PromptArgs) -> Result<String> {
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

    fn args() -> PromptArgs {
        let cli = Cli::parse_from(["agent", "run", "--shell", "bash", "--", "ls"]);
        let Command::Run(run) = cli.command else {
            panic!("expected run");
        };
        run.prompt
    }

    #[test]
    fn prompt_has_fixed_parts_and_both_schemas() {
        let prompt = system_prompt(&args()).unwrap();
        assert!(prompt.contains("## Sessions"));
        assert!(prompt.contains("## Input schema"));
        assert!(prompt.contains("## Answer schema"));
        assert!(prompt.contains("\"markdown\""));
        assert!(prompt.contains("\"session_id\""));
    }

    #[test]
    fn prompt_is_the_same_for_every_session() {
        let first = system_prompt(&args()).unwrap();
        let second = system_prompt(&args()).unwrap();
        assert_eq!(first, second);
        assert!(!first.contains("{state_dir}"));
        assert!(!first.contains("{session_file}"));
    }

    #[test]
    fn external_prompt_replaces_the_base_only() {
        let path = temp("one", "# custom agent\n");
        let mut args = args();
        args.system_prompt_files = vec![path.clone()];
        let prompt = system_prompt(&args).unwrap();
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
        let prompt = system_prompt(&args).unwrap();
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
        assert!(prompt.starts_with("# first\n\n# second"), "{prompt}");
        assert!(!prompt.contains("The shell's command-not-found handler"));
    }

    #[test]
    fn a_missing_prompt_file_is_an_error() {
        let mut args = args();
        args.system_prompt_files = vec!["/nonexistent-prompt".into()];
        assert!(system_prompt(&args).is_err());
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
