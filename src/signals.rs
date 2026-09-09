use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use anyhow::Result;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

static INTERRUPTS: AtomicUsize = AtomicUsize::new(0);

/// Handle SIGINT ourselves so a Ctrl-C reaches the child instead of us; the
/// second one is counted for the caller to act on. Other signals exit.
pub fn install() -> Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP])?;
    thread::spawn(move || {
        for signal in signals.forever() {
            if signal == SIGINT {
                INTERRUPTS.fetch_add(1, Ordering::SeqCst);
            } else {
                std::process::exit(128 + signal);
            }
        }
    });
    Ok(())
}

pub fn interrupts() -> usize {
    INTERRUPTS.load(Ordering::SeqCst)
}
