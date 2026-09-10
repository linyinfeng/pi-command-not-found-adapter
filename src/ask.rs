use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::Value;

use crate::pi::Agent;
use crate::prompt;
use crate::protocol::{Answer, Input};
use crate::signals;
use crate::ui::Ui;

const TICK: Duration = Duration::from_millis(100);

/// Why the agent did not answer, kept for the caller to show.
pub struct Failure {
    pub reason: String,
    pub last_text: Option<String>,
    pub errors: Vec<String>,
}

/// How a turn ended without an answer.
enum TurnError {
    Interrupted,
    Failed(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)?;
        if let Some(error) = self.errors.last() {
            write!(f, ": {error}")?;
        }
        Ok(())
    }
}

impl Agent {
    /// Send the input, then re-ask in the same session until the answer parses.
    pub fn ask(&mut self, input: &Input, retries: u32, ui: &mut Ui) -> Result<Answer, Failure> {
        let mut message = serde_json::to_string(input).expect("Input always serializes");
        let mut failure = Failure {
            reason: "no answer".into(),
            last_text: None,
            errors: Vec::new(),
        };
        for attempt in 0..=retries {
            let turn = match self.turn(&message, ui) {
                Ok(turn) => turn,
                Err(TurnError::Interrupted) => {
                    // The flag is set before this error; the caller turns it
                    // into exit 130 instead of reporting a failure.
                    failure.reason = "interrupted".into();
                    return Err(failure);
                }
                Err(TurnError::Failed(reason)) => {
                    failure.reason = reason;
                    return Err(failure);
                }
            };
            failure.last_text = turn.texts.last().cloned();
            failure.errors.extend(turn.errors);
            match crate::protocol::parse_answer(&turn.texts) {
                Ok(answer) => return Ok(answer),
                Err(reason) => failure.reason = reason,
            }
            if attempt < retries {
                message = prompt::retry_prompt(&failure.reason);
            }
        }
        failure.reason = format!(
            "pi did not return the expected answer JSON after {} turns ({})",
            retries + 1,
            failure.reason
        );
        Err(failure)
    }

    fn turn(&mut self, message: &str, ui: &mut Ui) -> Result<Turn, TurnError> {
        self.send_prompt(message).map_err(TurnError::Failed)?;
        let deadline = Instant::now() + self.timeout();
        let mut turn = Turn::default();
        // Nothing before the prompt's ack belongs to this turn: a resumed
        // session may replay earlier messages, and they must not become the
        // answer to the new command.
        let mut started = false;
        loop {
            if signals::interrupted() {
                // A busy event stream must not starve Ctrl-C: stop and let the
                // caller exit 130.
                self.stop();
                return Err(TurnError::Interrupted);
            }
            let event = self.events().recv_timeout(TICK);
            match event {
                Ok(Event::Acked) => started = true,
                Ok(Event::Closed) => {
                    return Err(TurnError::Failed(format!(
                        "pi exited early ({})",
                        self.exit_status()
                    )));
                }
                Ok(Event::Failed(error)) => return Err(TurnError::Failed(error)),
                Ok(_) if !started => continue,
                Ok(Event::Settled) => return Ok(turn),
                Ok(Event::Text(text)) => turn.texts.push(text),
                Ok(Event::Error(error)) => turn.errors.push(error),
                Ok(Event::Tool { name, args }) => ui.tool(&name, &summary(&args)),
                Ok(Event::Thinking) => ui.think(),
                Err(RecvTimeoutError::Timeout) => {
                    if Instant::now() >= deadline {
                        let seconds = self.timeout().as_secs();
                        return Err(TurnError::Failed(format!(
                            "pi did not settle within {seconds}s"
                        )));
                    }
                    ui.tick();
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(TurnError::Failed("pi stream closed".into()));
                }
            }
        }
    }
}

#[derive(Default)]
pub struct Turn {
    pub texts: Vec<String>,
    pub errors: Vec<String>,
}

/// Argument keys the progress block shows, in the order it looks for them.
/// `prompts/base.md` promises this list to the model; keep both in step.
const ARGUMENT_KEYS: [&str; 5] = ["command", "path", "pattern", "query", "url"];

/// One clipped line per tool call: the tool name plus a short argument.
pub fn summary(args: &Value) -> String {
    let Some(object) = args.as_object() else {
        return String::new();
    };
    for key in ARGUMENT_KEYS {
        if let Some(value) = object.get(key).and_then(Value::as_str) {
            let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
            if !value.is_empty() {
                return value;
            }
        }
    }
    String::new()
}

/// Events the reader thread forwards to the turn loop.
pub enum Event {
    /// `pi` acknowledged the prompt; events before this are not this turn's content.
    Acked,
    Text(String),
    Error(String),
    Tool {
        name: String,
        args: Value,
    },
    Thinking,
    Settled,
    Failed(String),
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn summarizes_known_argument_keys() {
        assert_eq!(summary(&json!({"command": "ls  -l"})), "ls -l");
        assert_eq!(summary(&json!({"path": "/tmp/a"})), "/tmp/a");
        assert_eq!(summary(&json!({"pattern": "foo"})), "foo");
        assert_eq!(summary(&json!({"other": "x"})), "");
        assert_eq!(summary(&json!("not an object")), "");
    }

    #[test]
    fn prefers_command_over_later_keys() {
        let args = json!({"query": "q", "command": "c"});
        assert_eq!(summary(&args), "c");
    }

    /// The prompt names these keys, so a new key here is a prompt change too.
    #[test]
    fn the_prompt_lists_every_argument_key() {
        let prompt = include_str!("../prompts/base.md");
        for key in ARGUMENT_KEYS {
            assert!(prompt.contains(key), "prompts/base.md does not name {key}");
        }
    }
}
