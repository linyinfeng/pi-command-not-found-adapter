use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use anyhow::Result;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// Handle SIGINT ourselves so the terminal's Ctrl-C does not kill us before
/// the child is reaped; the child is in the same process group and sees the
/// signal on its own. A second Ctrl-C skips the cleanup instead of waiting for
/// a turn that may not be listening. Other signals exit.
pub fn install() -> Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP])?;
    thread::spawn(move || {
        for signal in signals.forever() {
            match signal {
                SIGINT if !INTERRUPTED.swap(true, Ordering::SeqCst) => {}
                SIGINT => std::process::exit(130),
                _ => std::process::exit(128 + signal),
            }
        }
    });
    Ok(())
}

/// Whether a Ctrl-C arrived; the turn loop stops and the caller returns 130.
pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}
