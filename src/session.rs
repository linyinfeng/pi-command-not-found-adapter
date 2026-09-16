use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use tracing::{debug, warn};

use crate::cli::Run;
use crate::config::Config;

const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

pub struct Session {
    pub id: String,
    /// `$XDG_STATE_HOME/pi-command-not-found-adapter`: pi's sessions and the
    /// agent's notes live here.
    pub state: PathBuf,
    pub dir: PathBuf,
}

impl Session {
    pub fn resolve(args: &Run, config: &Config) -> Result<Self> {
        let id = sanitize(args.session_id.as_deref().unwrap_or_default());
        let id = if id.is_empty() {
            let id = random_id();
            warn!("no session id given; using {id}");
            id
        } else {
            id
        };
        let state = dirs::state_dir()
            .context("cannot determine the XDG state directory")?
            .join("pi-command-not-found-adapter");
        let root = config
            .session_root
            .clone()
            .unwrap_or_else(|| state.join("sessions"));
        let dir = root.join(&id);
        DirBuilder::new()
            .recursive(true)
            .mode(DIR_MODE)
            .create(&dir)?;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(DIR_MODE));
        Ok(Self { id, state, dir })
    }

    pub fn session_file(&self) -> PathBuf {
        self.dir.join("session.jsonl")
    }

    /// Create the per-invocation log directory and its static files.
    pub fn start_history(&self, input: &str, markdown: &str, source: &str) {
        let dir = self.dir.join("history").join(stamp());
        if DirBuilder::new()
            .recursive(true)
            .mode(DIR_MODE)
            .create(&dir)
            .is_err()
        {
            return;
        }
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(DIR_MODE));
        for (name, content) in [("input", input), ("markdown", markdown), ("source", source)] {
            write_file(&dir.join(name), content);
        }
        debug!("history {}", dir.display());
    }
}

fn write_file(path: &Path, content: &str) {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(FILE_MODE)
        .open(path);
    if let Ok(mut file) = file {
        let _ = file.write_all(content.as_bytes());
    }
}

/// Keep session ids to characters that are safe in a path component.
fn sanitize(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn random_id() -> String {
    if let Ok(uuid) = fs::read_to_string("/proc/sys/kernel/random/uuid") {
        let uuid = uuid.trim();
        if !uuid.is_empty() {
            return uuid.to_string();
        }
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}")
}

/// `2026-09-09T16-56-00-023Z`, matching the shell history naming.
fn stamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default();
    let (days, rest) = (millis.div_euclid(86_400_000), millis.rem_euclid(86_400_000));
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (rest / 3_600_000, rest / 60_000 % 60, rest / 1000 % 60);
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}-{minute:02}-{second:02}-{:03}Z",
        rest % 1000
    )
}

/// Days since the Unix epoch to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_path_hostile_ids() {
        assert_eq!(sanitize("a/b.c d"), "a_b_c_d");
        assert_eq!(sanitize("abc-123"), "abc-123");
    }

    #[test]
    fn formats_known_instants() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn random_id_is_not_empty() {
        assert!(!random_id().is_empty());
    }
}
