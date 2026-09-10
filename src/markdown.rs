use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;

use crate::cli::Run;
use crate::ui::{Ui, printable};

/// Render the note with mdcat, falling back to the raw text.
pub fn render(text: &str, args: &Run, ui: &mut Ui) {
    if text.trim().is_empty() {
        return;
    }
    ui.clear();
    if mdcat(text, args, ui) {
        return;
    }
    ui.line(printable(text).trim_end());
}

fn mdcat(text: &str, args: &Run, ui: &Ui) -> bool {
    let mut command = Command::new(&args.mdcat);
    command
        .arg("--columns")
        .arg(ui.width().max(20).to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if !ui.is_tty() {
        command.arg("--no-colour");
    }
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let text = text.to_string();
        thread::spawn(move || {
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    let Ok(output) = child.wait_with_output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let mut stderr = std::io::stderr();
    stderr.write_all(&output.stdout).is_ok() && stderr.flush().is_ok()
}
