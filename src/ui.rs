use std::collections::VecDeque;
use std::io::Write;

use console::{Term, measure_text_width};

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const CLOUD: &str = "💭";

/// Progress block on stderr: the last few tool calls plus a live status line.
pub struct Ui {
    term: Term,
    tty: bool,
    /// Wrap width for rendered note output (may exceed the terminal; mcat
    /// wraps itself).
    width: usize,
    /// Physical room for the progress block: must not exceed the terminal,
    /// or a wrapped line would corrupt the redraw.
    block_width: usize,
    max_lines: usize,
    lines: VecDeque<String>,
    clouds: usize,
    frame: usize,
    drawn: usize,
}

impl Ui {
    pub fn new(max_lines: usize, width: Option<usize>) -> Self {
        let term = Term::stderr();
        let tty = term.is_term();
        let physical = term.size_checked().map(|(_, columns)| columns as usize);
        let width = width.or(physical).unwrap_or(80).max(20);
        let block_width = width.min(physical.unwrap_or(width));
        Self {
            term,
            tty,
            width,
            block_width,
            max_lines: max_lines.max(1),
            lines: VecDeque::new(),
            clouds: 0,
            frame: 0,
            drawn: 0,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn is_tty(&self) -> bool {
        self.tty
    }

    pub fn tool(&mut self, name: &str, summary: &str) {
        self.clouds = 0;
        let mut line = format!("🔧{name}");
        if !summary.is_empty() {
            line.push_str(": ");
            line.push_str(summary);
        }
        let line = self.clip(&line);
        if self.lines.len() == self.max_lines {
            self.lines.pop_front();
        }
        if self.tty {
            self.lines.push_back(line);
            self.draw();
        } else {
            self.plain(&line);
        }
    }

    pub fn think(&mut self) {
        self.clouds += 1;
        self.draw();
    }

    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % FRAMES.len();
        self.draw();
    }

    /// Erase the block and park the cursor on its first line.
    pub fn clear(&mut self) {
        if self.tty && self.drawn > 0 {
            let _ = self.term.clear_last_lines(self.drawn);
            let _ = self.term.flush();
            self.drawn = 0;
        }
    }

    /// Write a line outside the block.
    pub fn line(&mut self, text: &str) {
        self.clear();
        self.plain(text);
    }

    pub fn rule(&mut self) {
        let rule = "─".repeat(self.block_width);
        self.line(&rule);
    }

    /// Announce the command that is about to run.
    pub fn announce(&mut self, command: &str) {
        self.clear();
        let shown = printable(command).replace('\n', "⏎").replace('\t', " ");
        if self.tty {
            let _ = writeln!(self.term, "⚡ \x1b[1m{shown}\x1b[0m");
        } else {
            let _ = writeln!(self.term, "⚡ {shown}");
        }
        let _ = self.term.flush();
    }

    fn plain(&mut self, text: &str) {
        let _ = writeln!(self.term, "{text}");
        let _ = self.term.flush();
    }

    fn status(&self) -> String {
        let room = self.block_width.saturating_sub(2) / 2;
        let mut status = CLOUD.repeat(self.clouds.min(room));
        status.push_str(FRAMES[self.frame % FRAMES.len()]);
        status
    }

    fn draw(&mut self) {
        if !self.tty {
            return;
        }
        let mut block: Vec<String> = self.lines.iter().cloned().collect();
        block.push(self.status());
        let total = self.drawn.max(block.len());
        if self.drawn > 0 {
            let _ = self.term.move_cursor_up(self.drawn);
        }
        for index in 0..total {
            let _ = self.term.clear_line();
            if let Some(line) = block.get(index) {
                let _ = self.term.write_str(line);
            }
            let _ = self.term.write_str("\n");
        }
        let _ = self.term.flush();
        self.drawn = total;
    }

    fn clip(&self, text: &str) -> String {
        let text = printable(text);
        let limit = self.block_width.saturating_sub(1).max(1);
        let mut clipped = String::new();
        let mut used = 0;
        for char in text.chars() {
            let size = measure_text_width(&char.to_string());
            if used + size > limit {
                clipped.push('…');
                return clipped;
            }
            clipped.push(char);
            used += size;
        }
        clipped
    }
}

/// Drop control characters that would corrupt the terminal.
pub fn printable(text: &str) -> String {
    text.chars()
        .filter(|c| matches!(c, '\n' | '\t') || !c.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_control_characters_out() {
        assert_eq!(printable("a\u{7}b\tc\nd"), "ab\tc\nd");
    }

    #[test]
    fn clips_wide_characters() {
        let ui = Ui::new(5, Some(20));
        assert_eq!(
            ui.clip("abcdefghijklmnopqrstuvwxyz"),
            "abcdefghijklmnopqrs…"
        );
        let wide = "你好世界你好世界你好世界你好世界";
        assert!(measure_text_width(&ui.clip(wide)) <= 20);
    }

    #[test]
    fn keeps_short_lines() {
        let ui = Ui::new(5, Some(20));
        assert_eq!(ui.clip("short"), "short");
    }
}
