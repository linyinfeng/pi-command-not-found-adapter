use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{debug, warn};

use crate::ask::Event;
use crate::cli::Run;
use crate::session::Session;

/// A running `pi --mode rpc` process.
pub struct Agent {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    events: Receiver<Event>,
    reader: Option<JoinHandle<()>>,
    timeout: Duration,
}

impl Agent {
    pub fn spawn(args: &Run, session: &Session, system_prompt: &str) -> Result<Self> {
        let mut command = Command::new(&args.pi);
        command
            .arg("--mode")
            .arg("rpc")
            .arg("--no-context-files")
            .arg("--session")
            .arg(session.session_file())
            .arg("--append-system-prompt")
            .arg(system_prompt);
        if let Some(model) = &args.model {
            command.arg("--model").arg(model);
        }
        if let Some(thinking) = &args.thinking {
            command.arg("--thinking").arg(thinking);
        }
        command.args(&args.pi_args);
        debug!(
            "{} --mode rpc --session {} (model {:?})",
            args.pi,
            session.session_file().display(),
            args.model
        );
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to start {}", args.pi))?;
        let stdin = child.stdin.take().context("pi stdin missing")?;
        let stdout = child.stdout.take().context("pi stdout missing")?;
        let stdin = Arc::new(Mutex::new(stdin));
        let (sender, events) = channel();
        let reader = spawn_reader(stdout, Arc::clone(&stdin), sender, args.trace.clone());
        Ok(Self {
            child,
            stdin,
            events,
            reader: Some(reader),
            timeout: Duration::from_secs(args.timeout),
        })
    }

    pub fn events(&self) -> &Receiver<Event> {
        &self.events
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Send one prompt; its events arrive on [`Self::events`] until `Settled`.
    pub fn send_prompt(&self, message: &str) -> Result<(), String> {
        self.send(&json!({"type": "prompt", "message": message}))
    }

    pub fn send(&self, command: &Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().map_err(|_| "pi stdin poisoned")?;
        writeln!(stdin, "{command}").map_err(|error| format!("pi stdin: {error}"))?;
        stdin
            .flush()
            .map_err(|error| format!("pi stdin flush: {error}"))
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    pub fn exit_status(&mut self) -> String {
        for _ in 0..20 {
            match self.child.try_wait() {
                Ok(Some(status)) => return status.to_string(),
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(error) => return error.to_string(),
            }
        }
        "still running".into()
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        self.stop();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn spawn_reader(
    stdout: std::process::ChildStdout,
    stdin: Arc<Mutex<ChildStdin>>,
    events: std::sync::mpsc::Sender<Event>,
    trace: Option<PathBuf>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut trace = trace.and_then(|path| {
            match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                Ok(file) => Some(file),
                Err(error) => {
                    warn!("cannot append to {}: {error}", path.display());
                    None
                }
            }
        });
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(file) = trace.as_mut() {
                let _ = writeln!(file, "{line}");
            }
            if let Ok(event) = serde_json::from_str::<Incoming>(&line)
                && let Some(event) = translate(event, &stdin)
                && events.send(event).is_err()
            {
                break;
            }
        }
        let _ = events.send(Event::Closed);
    })
}

fn translate(event: Incoming, stdin: &Arc<Mutex<ChildStdin>>) -> Option<Event> {
    match event {
        Incoming::Response {
            command,
            success,
            error,
        } => {
            if success {
                return None;
            }
            Some(Event::Failed(format!(
                "pi rejected {command}: {}",
                error.unwrap_or_else(|| "no error given".into())
            )))
        }
        Incoming::MessageEnd { message } => {
            let message = message?;
            if message.role.as_deref() != Some("assistant") {
                return None;
            }
            let text = message.text();
            if !text.trim().is_empty() {
                return Some(Event::Text(text));
            }
            message.error_message.map(Event::Error)
        }
        Incoming::MessageUpdate { event } => match event.and_then(|event| event.kind).as_deref() {
            Some("thinking_start") => Some(Event::Thinking),
            _ => None,
        },
        Incoming::ToolExecutionStart { tool_name, args } => Some(Event::Tool {
            name: tool_name.unwrap_or_else(|| "?".into()),
            args: args.unwrap_or(Value::Null),
        }),
        Incoming::AgentSettled => Some(Event::Settled),
        Incoming::ExtensionUiRequest { id, method } => {
            if matches!(method.as_str(), "select" | "confirm" | "input" | "editor") {
                let answer = json!({
                    "type": "extension_ui_response",
                    "id": id,
                    "cancelled": true,
                });
                if let Ok(mut stdin) = stdin.lock() {
                    let _ = writeln!(stdin, "{answer}");
                    let _ = stdin.flush();
                }
            }
            None
        }
        Incoming::Other => None,
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Incoming {
    Response {
        #[serde(default)]
        command: String,
        #[serde(default)]
        success: bool,
        error: Option<String>,
    },
    MessageEnd {
        message: Option<Message>,
    },
    MessageUpdate {
        #[serde(rename = "assistantMessageEvent")]
        event: Option<Delta>,
    },
    ToolExecutionStart {
        #[serde(rename = "toolName")]
        tool_name: Option<String>,
        args: Option<Value>,
    },
    AgentSettled,
    ExtensionUiRequest {
        id: String,
        method: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct Message {
    role: Option<String>,
    content: Option<Vec<Content>>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
}

impl Message {
    fn text(&self) -> String {
        self.content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|part| part.kind.as_deref() == Some("text"))
            .filter_map(|part| part.text.as_deref())
            .collect()
    }
}

#[derive(Deserialize)]
struct Content {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(rename = "type")]
    kind: Option<String>,
}
