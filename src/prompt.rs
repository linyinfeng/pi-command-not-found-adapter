use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use schemars::{JsonSchema, schema_for};

use crate::protocol::{Answer, Input};

/// Used when no `system-prompt-file` setting is given.
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
pub fn system_prompt(files: &[PathBuf]) -> Result<String> {
    let base = base_prompt(files)?;
    let input = schema::<Input>()?;
    let answer = schema::<Answer>()?;
    Ok(format!(
        "{base}\n{HISTORY}\n## Input schema\n\n```json\n{input}\n```\n\n\
         ## Answer schema\n\n```json\n{answer}\n```\n"
    ))
}

fn base_prompt(files: &[PathBuf]) -> Result<String> {
    if files.is_empty() {
        return Ok(BASE.trim_end().to_string());
    }
    let mut parts = Vec::new();
    for path in files {
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

    fn prompts() -> Vec<PathBuf> {
        Vec::new()
    }

    #[test]
    fn prompt_has_fixed_parts_and_both_schemas() {
        let prompt = system_prompt(&prompts()).unwrap();
        assert!(prompt.contains("## Sessions"));
        assert!(prompt.contains("## Input schema"));
        assert!(prompt.contains("## Answer schema"));
        assert!(prompt.contains("\"markdown\""));
        assert!(prompt.contains("\"session_id\""));
    }

    #[test]
    fn prompt_is_the_same_for_every_session() {
        let first = system_prompt(&prompts()).unwrap();
        let second = system_prompt(&prompts()).unwrap();
        assert_eq!(first, second);
        assert!(!first.contains("{state_dir}"));
        assert!(!first.contains("{session_file}"));
    }

    #[test]
    fn external_prompt_replaces_the_base_only() {
        let path = temp("one", "# custom agent\n");
        let prompt = system_prompt(std::slice::from_ref(&path)).unwrap();
        let _ = fs::remove_file(&path);
        assert!(prompt.starts_with("# custom agent"));
        assert!(prompt.contains("## Sessions"));
        assert!(!prompt.contains("The shell's command-not-found handler"));
    }

    #[test]
    fn several_prompts_are_concatenated_in_order() {
        let first = temp("first", "# first\n");
        let second = temp("second", "# second\n");
        let prompt = system_prompt(&[first.clone(), second.clone()]).unwrap();
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
        assert!(prompt.starts_with("# first\n\n# second"), "{prompt}");
        assert!(!prompt.contains("The shell's command-not-found handler"));
    }

    #[test]
    fn a_missing_prompt_file_is_an_error() {
        assert!(system_prompt(&["/nonexistent-prompt".into()]).is_err());
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
