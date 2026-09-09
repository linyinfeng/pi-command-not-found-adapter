use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::session::open_log;
use crate::signals;

const CHUNK: usize = 64 * 1024;
const GRACE: Duration = Duration::from_millis(500);
const POLL: Duration = Duration::from_millis(20);

/// Run the command, tee its output to the terminal and the log, and return
/// the exit status it should produce for the shell.
pub fn run(history: Option<&Path>, command: &str) -> Result<i32> {
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(command)
        .stdin(stdin())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run bash -c {command:?}"))?;
    let stdout = child.stdout.take().context("stdout missing")?;
    let stderr = child.stderr.take().context("stderr missing")?;
    let (sender, done) = channel();
    let out = tee(
        stdout,
        io::stdout(),
        history.and_then(|dir| open_log(&dir.join("stdout"))),
        sender.clone(),
    );
    let err = tee(
        stderr,
        io::stderr(),
        history.and_then(|dir| open_log(&dir.join("stderr"))),
        sender,
    );
    let status = wait(&mut child);
    // A grandchild may hold the pipes open forever, so wait only a bounded
    // grace period; the detached threads die with the process.
    let mut finished = 0;
    while finished < 2 && done.recv_timeout(GRACE).is_ok() {
        finished += 1;
    }
    drop((out, err));
    Ok(exit_code(status))
}

/// Interactive commands read the terminal, not pi's pipe.
fn stdin() -> Stdio {
    File::open("/dev/tty")
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::inherit())
}

fn wait(child: &mut Child) -> ExitStatus {
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return status;
        }
        if signals::interrupts() > 1 {
            let _ = child.kill();
        }
        thread::sleep(POLL);
    }
}

/// Copy a pipe to the terminal and the log until EOF or the grace period.
fn tee<R, W>(
    mut reader: R,
    mut writer: W,
    mut log: Option<File>,
    done: Sender<()>,
) -> JoinHandle<()>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; CHUNK];
        while let Ok(read) = reader.read(&mut buffer) {
            if read == 0 {
                break;
            }
            let _ = writer.write_all(&buffer[..read]);
            let _ = writer.flush();
            if let Some(file) = log.as_mut()
                && file.write_all(&buffer[..read]).is_err()
            {
                log = None;
            }
        }
        let _ = done.send(());
    })
}

fn exit_code(status: ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}
