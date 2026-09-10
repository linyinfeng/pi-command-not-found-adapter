use std::env;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};

use crate::cli::Run;
use crate::ui::{Ui, printable};

/// mcat's only light theme; the rest are dark.
const LIGHT_THEME: &str = "makurai-light";
/// Long enough for a slow link to answer, short enough that a terminal which
/// never answers does not hold up the note.
const TIMEOUT: Duration = Duration::from_millis(300);

/// Render the note with mcat, falling back to the raw text.
pub fn render(text: &str, args: &Run, ui: &mut Ui) {
    if text.trim().is_empty() {
        return;
    }
    ui.clear();
    if mcat(text, args, ui) {
        return;
    }
    ui.line(printable(text).trim_end());
}

fn mcat(text: &str, args: &Run, ui: &Ui) -> bool {
    let mut command = Command::new(&args.mcat);
    command
        // Never page, never show loading bars, and wrap to our width.
        .arg("-P")
        .arg("--silent")
        .arg("--sc")
        .arg(format!("{}xauto", ui.width().max(20)))
        // mcat drops ANSI when its stdout is not a terminal; ours is a pipe
        // even on the way to the user's terminal.
        .arg(if ui.is_tty() { "-c" } else { "-C" });
    if let Some(theme) = theme() {
        command.arg("--theme").arg(theme);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
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

/// The theme to force: none when mcat already has one from the environment,
/// none for a dark terminal (mcat's default is dark) or when the terminal
/// cannot be asked at all.
fn theme() -> Option<&'static str> {
    if env::var_os("MCAT_THEME").is_some() {
        return None;
    }
    let mut options = QueryOptions::default();
    options.timeout = TIMEOUT;
    pick(theme_mode(options).ok())
}

fn pick(mode: Option<ThemeMode>) -> Option<&'static str> {
    match mode {
        Some(ThemeMode::Light) => Some(LIGHT_THEME),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_light_terminal_gets_a_theme() {
        assert_eq!(pick(Some(ThemeMode::Light)), Some(LIGHT_THEME));
        assert_eq!(pick(Some(ThemeMode::Dark)), None);
        assert_eq!(pick(None), None);
    }
}
