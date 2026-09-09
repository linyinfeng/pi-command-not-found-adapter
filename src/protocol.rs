use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What the shell passes to the agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Input {
    pub session_id: String,
    pub cwd: String,
    pub input: String,
}

/// What the agent must answer with.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Answer {
    pub markdown: String,
    pub command: String,
}

/// The last JSON object in the assistant text that matches [`Answer`].
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
            Some(Ok(value)) => match serde_json::from_value::<Answer>(value) {
                Ok(answer) => found = Some(answer),
                Err(error) => reason = error.to_string(),
            },
            Some(Err(error)) => reason = error.to_string(),
            None => {}
        }
    }
    found.ok_or(reason)
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
        assert_eq!(answer.markdown, "hi");
        assert_eq!(answer.command, "ls");
    }

    #[test]
    fn parses_object_inside_prose_and_fences() {
        let body = "Sure, here it is:\n```json\n{\"markdown\":\"m\",\"command\":\"\"}\n```\n";
        let answer = parse_answer(&texts(body)).unwrap();
        assert_eq!(answer.markdown, "m");
        assert_eq!(answer.command, "");
    }

    #[test]
    fn last_valid_object_wins() {
        let body = r#"{"markdown":"first","command":"a"} then {"markdown":"second","command":"b"}"#;
        let answer = parse_answer(&texts(body)).unwrap();
        assert_eq!(answer.markdown, "second");
    }

    #[test]
    fn rejects_missing_field_and_reports_reason() {
        let error = parse_answer(&texts(r#"{"markdown":"only"}"#)).unwrap_err();
        assert!(error.contains("command"), "{error}");
    }

    #[test]
    fn ignores_unrelated_json_objects() {
        let error = parse_answer(&texts(r#"{"other":1} plain text"#)).unwrap_err();
        assert!(!error.is_empty());
    }

    #[test]
    fn falls_back_to_earlier_text() {
        let texts = vec![
            "garbage".to_string(),
            r#"{"markdown":"","command":"x"}"#.into(),
        ];
        assert_eq!(parse_answer(&texts).unwrap().command, "x");
    }
}
