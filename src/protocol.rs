use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What the shell passes to the agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Input {
    /// Session id of this shell conversation; also exported as
    /// COMMAND_NOT_FOUND_SESSION_ID.
    pub session_id: Option<String>,
    /// Directory the command was typed in.
    pub cwd: Option<String>,
    /// Command line the user typed.
    pub input: Option<String>,
}

/// What the agent must answer with; both fields are optional.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Answer {
    /// Short note shown to the user, rendered as Markdown by mdcat on
    /// stderr: plain Markdown only, no HTML, images or mermaid. Omit when
    /// there is nothing worth explaining.
    pub markdown: Option<String>,
    /// The exact command line the user wanted, run with non-interactive
    /// `bash -c` in the current directory and environment — no aliases or
    /// shell functions, and stdout/stderr are pipes, so no colors or
    /// pager. Use `nix shell nixpkgs#<pkg> -c <cmd>` for a package that is
    /// not installed. Omit when nothing should run: a typo, an ambiguous
    /// request, or a task you already did yourself.
    pub command: Option<String>,
}

/// The last JSON object in the assistant text that looks like an [`Answer`].
pub fn parse_answer(texts: &[String]) -> Result<Answer, String> {
    let mut answer = None;
    let mut reason = String::from("no JSON object found");
    for text in texts.iter().rev() {
        match scan(text) {
            Ok(found) => {
                answer = Some(found);
                break;
            }
            Err(why) => reason = why,
        }
    }
    answer.ok_or(reason)
}

fn scan(text: &str) -> Result<Answer, String> {
    let mut found = None;
    let mut reason = String::from("no JSON object found");
    for (offset, _) in text.match_indices('{') {
        let stream = serde_json::Deserializer::from_str(&text[offset..]).into_iter::<Value>();
        match stream.into_iter().next() {
            Some(Ok(value)) => match answer_from(&value) {
                Ok(answer) => found = Some(answer),
                Err(error) => reason = error,
            },
            Some(Err(error)) => reason = error.to_string(),
            None => {}
        }
    }
    found.ok_or(reason)
}

/// An answer must carry at least one of the two fields, otherwise it is just
/// some other JSON object in the model's prose.
fn answer_from(value: &Value) -> Result<Answer, String> {
    let object = value.as_object().ok_or("not a JSON object")?;
    if !object.contains_key("markdown") && !object.contains_key("command") {
        return Err("object has neither `markdown` nor `command`".into());
    }
    serde_json::from_value(value.clone()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(body: &str) -> Vec<String> {
        vec![body.to_string()]
    }

    #[test]
    fn parses_plain_object() {
        let answer = parse_answer(&texts(r#"{"markdown":"hi","command":"ls"}"#)).unwrap();
        assert_eq!(answer.markdown.as_deref(), Some("hi"));
        assert_eq!(answer.command.as_deref(), Some("ls"));
    }

    #[test]
    fn parses_object_inside_prose_and_fences() {
        let body = "Sure, here it is:\n```json\n{\"markdown\":\"m\",\"command\":\"\"}\n```\n";
        let answer = parse_answer(&texts(body)).unwrap();
        assert_eq!(answer.markdown.as_deref(), Some("m"));
        assert_eq!(answer.command.as_deref(), Some(""));
    }

    #[test]
    fn last_valid_object_wins() {
        let body = r#"{"markdown":"first","command":"a"} then {"markdown":"second","command":"b"}"#;
        let answer = parse_answer(&texts(body)).unwrap();
        assert_eq!(answer.markdown.as_deref(), Some("second"));
    }

    #[test]
    fn accepts_one_field_only() {
        let answer = parse_answer(&texts(r#"{"markdown":"only"}"#)).unwrap();
        assert_eq!(answer.markdown.as_deref(), Some("only"));
        assert!(answer.command.is_none());

        let answer = parse_answer(&texts(r#"{"command":"ls"}"#)).unwrap();
        assert!(answer.markdown.is_none());
        assert_eq!(answer.command.as_deref(), Some("ls"));
    }

    #[test]
    fn accepts_explicit_nulls() {
        let answer = parse_answer(&texts(r#"{"markdown":null,"command":"ls"}"#)).unwrap();
        assert!(answer.markdown.is_none());
    }

    #[test]
    fn rejects_unrelated_json_objects() {
        let error = parse_answer(&texts(r#"{"other":1} plain text"#)).unwrap_err();
        assert!(error.contains("markdown"), "{error}");
    }

    #[test]
    fn rejects_wrong_field_types() {
        let error = parse_answer(&texts(r#"{"command":42}"#)).unwrap_err();
        assert!(error.contains("invalid type"), "{error}");
    }

    #[test]
    fn falls_back_to_earlier_text() {
        let texts = vec!["garbage".to_string(), r#"{"command":"x"}"#.to_string()];
        assert_eq!(parse_answer(&texts).unwrap().command.as_deref(), Some("x"));
    }
}
