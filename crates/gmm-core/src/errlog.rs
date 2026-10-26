// SPDX-License-Identifier: MIT OR Apache-2.0

//! GMM's local error log, for bug reports (Phase 6, Report a Bug; docs/04-security.md, R1).
//!
//! Errors only, one per line, in `<data>/logs/errors.log`; past [`MAX_LOG_BYTES`] the file
//! becomes `errors.1.log` (replacing the older one), so the log never holds more than about
//! twice that. Nothing leaves the computer: the user reads the log in the app and attaches
//! the file to an issue themselves. Before a line is written, anything that would identify
//! the user goes: the home folder becomes `~` (in every spelling Windows and JSON use), and
//! the account name inside a path becomes `<user>`. Lines are capped and kept to one line.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use gmm_fsguard::{Access, Guard, ScopedPath};
use serde::Serialize;

use crate::error::CoreError;

/// The current log is rotated once it passes this.
pub const MAX_LOG_BYTES: u64 = 256 * 1024;

/// A message longer than this is cut.
const MAX_MESSAGE_CHARS: usize = 2_000;

const LOG_FILE: &str = "errors.log";
const OLD_LOG_FILE: &str = "errors.1.log";

/// One recorded error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LogEntry {
    /// UTC, like `2026-10-25T21:03:11Z`.
    pub time: String,
    /// Where it happened: `app`, `install`, `ui`, `panic`, `cli`.
    pub source: String,
    pub message: String,
}

/// See the module docs. Cheap to share; writes are serialized.
#[derive(Debug)]
pub struct ErrorLog {
    guard: Guard,
    dir: ScopedPath,
    current: ScopedPath,
    old: ScopedPath,
    redact: Redactor,
    lock: Mutex<()>,
}

impl ErrorLog {
    /// Opens `<data_dir>/logs/`, creating it on first use. `home` is the user's home folder,
    /// the one thing every line is cleaned of.
    pub fn open(data_dir: &Path, home: Option<&Path>) -> Result<Self, CoreError> {
        let mut guard = Guard::new();
        let data = guard.ensure_root("data", data_dir, Access::ReadWrite)?;
        let dir = guard.path(data, "logs")?;
        guard.create_dir_all(&dir)?;
        Ok(Self {
            current: dir.join(LOG_FILE)?,
            old: dir.join(OLD_LOG_FILE)?,
            dir,
            guard,
            redact: Redactor::new(home),
            lock: Mutex::new(()),
        })
    }

    /// Appends one error. Best effort: a log that can't be written never breaks anything.
    pub fn record(&self, source: &str, message: &str) {
        let _held = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let line = format!(
            "{}\t{}\t{}\n",
            utc(crate::install::now()),
            one_line(source, 32),
            one_line(&self.redact.apply(message), MAX_MESSAGE_CHARS),
        );
        if self
            .guard
            .metadata(&self.current)
            .is_ok_and(|m| m.len() + line.len() as u64 > MAX_LOG_BYTES)
        {
            let _ = self.guard.remove_file(&self.old);
            let _ = self.guard.rename(&self.current, &self.old);
        }
        let file = self
            .guard
            .open_append(&self.current)
            .or_else(|_| self.guard.create_new(&self.current));
        if let Ok(mut file) = file {
            let _ = file.write_all(line.as_bytes());
        }
    }

    /// The newest `limit` entries, oldest first (both files).
    pub fn entries(&self, limit: usize) -> Vec<LogEntry> {
        let mut out: Vec<LogEntry> = [&self.old, &self.current]
            .into_iter()
            .filter_map(|p| self.guard.read_to_string(p).ok())
            .flat_map(|text| text.lines().filter_map(parse).collect::<Vec<_>>())
            .collect();
        let skip = out.len().saturating_sub(limit);
        out.drain(..skip);
        out
    }

    /// Both files' text, oldest first, as the user would attach it.
    pub fn text(&self) -> String {
        [&self.old, &self.current]
            .into_iter()
            .filter_map(|p| self.guard.read_to_string(p).ok())
            .collect()
    }

    /// Deletes the log for good. Returns how many entries went.
    pub fn clear(&self) -> usize {
        let _held = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let count = self.entries(usize::MAX).len();
        for p in [&self.old, &self.current] {
            let _ = self.guard.remove_file(p);
        }
        count
    }

    /// The `logs` folder, for "Open log folder".
    pub fn folder(&self) -> Result<PathBuf, CoreError> {
        Ok(self.guard.os_path_of_dir(&self.dir)?)
    }

    /// Cleans text the same way lines are cleaned (for bug report drafts).
    pub fn redact(&self, text: &str) -> String {
        self.redact.apply(text)
    }
}

/// Removes the home folder and the account name from text.
#[derive(Debug, Clone, Default)]
struct Redactor {
    /// Spellings of the home folder, longest first, lowercase (matched case-insensitively).
    homes: Vec<String>,
    /// The account name, lowercase (the home folder's last part).
    user: Option<String>,
}

impl Redactor {
    fn new(home: Option<&Path>) -> Self {
        let Some(home) = home.and_then(|h| h.to_str()) else {
            return Self::default();
        };
        let home = home.trim_end_matches(['\\', '/']);
        if home.len() < 3 {
            return Self::default();
        }
        let back = home.replace('/', "\\");
        let forward = home.replace('\\', "/");
        let mut homes = vec![
            format!("\\\\?\\{back}"),
            back.replace('\\', "\\\\"),
            back.clone(),
            forward,
        ];
        homes.iter_mut().for_each(|h| *h = h.to_lowercase());
        homes.sort_by_key(|h| std::cmp::Reverse(h.len()));
        homes.dedup();
        let user = back
            .rsplit('\\')
            .next()
            .filter(|u| u.chars().count() >= 2)
            .map(str::to_lowercase);
        Self { homes, user }
    }

    fn apply(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for home in &self.homes {
            out = replace_ci(&out, home, "~");
        }
        if let Some(user) = &self.user {
            // The account name as a whole path part, wherever else it shows up
            // (`C:\Users\<name>` on another drive, `/home/<name>`).
            for (sep_l, sep_r) in [("\\", "\\"), ("/", "/"), ("\\\\", "\\\\")] {
                out = replace_ci(
                    &out,
                    &format!("{sep_l}{user}{sep_r}"),
                    &format!("{sep_l}<user>{sep_r}"),
                );
            }
        }
        out
    }
}

/// `text` with every case-insensitive match of the lowercase `needle` replaced.
fn replace_ci(text: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() {
        return text.to_owned();
    }
    let lower = text.to_lowercase();
    // Lowercasing can change byte lengths outside ASCII; fall back to exact matching then.
    if lower.len() != text.len() {
        return text.replace(needle, with);
    }
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(found) = lower[at..].find(needle) {
        out.push_str(&text[at..at + found]);
        out.push_str(with);
        at += found + needle.len();
    }
    out.push_str(&text[at..]);
    out
}

/// Tabs and line breaks become spaces; long text is cut at `max` characters.
fn one_line(text: &str, max: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if flat.chars().count() > max {
        let cut: String = flat.chars().take(max).collect();
        format!("{cut}…")
    } else {
        flat
    }
}

fn parse(line: &str) -> Option<LogEntry> {
    let mut parts = line.splitn(3, '\t');
    Some(LogEntry {
        time: parts.next()?.to_owned(),
        source: parts.next()?.to_owned(),
        message: parts.next()?.to_owned(),
    })
}

/// Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
pub fn utc(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs / 3_600,
        secs % 3_600 / 60,
        secs % 60
    )
}

#[cfg(test)]
#[allow(clippy::disallowed_methods, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn log(home: &str) -> (tempfile::TempDir, ErrorLog) {
        let dir = tempfile::tempdir().unwrap();
        let log = ErrorLog::open(dir.path(), Some(Path::new(home))).unwrap();
        (dir, log)
    }

    #[test]
    fn utc_dates() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_792_929_600), "2026-10-25T12:00:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn removes_the_home_folder_and_account_name() {
        let (_dir, log) = log(r"C:\Users\Alex");
        log.record(
            "app",
            r#"can't read C:\Users\alex\AppData\Roaming\XXMI Launcher\x.json; also c:/users/ALEX/a, \\?\C:\Users\alex\b, "C:\\Users\\alex\\c" and D:\Users\alex\d and /home/alex/e"#,
        );
        let entry = &log.entries(10)[0];
        assert_eq!(entry.source, "app");
        assert!(
            !entry.message.to_lowercase().contains("alex"),
            "{}",
            entry.message
        );
        assert!(
            entry
                .message
                .contains(r"~\AppData\Roaming\XXMI Launcher\x.json"),
            "{}",
            entry.message
        );
        assert!(
            entry.message.contains(r"D:\Users\<user>\d"),
            "{}",
            entry.message
        );
        // A mod named like the user stays: only path parts are touched.
        log.record("app", "Alex's outfit failed");
        assert_eq!(log.entries(10)[1].message, "Alex's outfit failed");
    }

    #[test]
    fn keeps_each_entry_on_one_line_and_short() {
        let (_dir, log) = log(r"C:\Users\x1");
        log.record(
            "ui\tbad\nsource",
            &format!("first\nsecond\t{}", "a".repeat(5_000)),
        );
        let entries = log.entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "ui bad source");
        assert!(entries[0].message.starts_with("first second "));
        assert!(entries[0].message.chars().count() <= MAX_MESSAGE_CHARS + 1);
    }

    #[test]
    fn rotates_and_clears() {
        let (dir, log) = log(r"C:\Users\x1");
        let big = "e".repeat(1_900);
        for _ in 0..300 {
            log.record("app", &big);
        }
        let logs = dir.path().join("logs");
        assert!(logs.join(OLD_LOG_FILE).exists());
        let size = |f: &str| std::fs::metadata(logs.join(f)).map_or(0, |m| m.len());
        assert!(size(LOG_FILE) <= MAX_LOG_BYTES && size(OLD_LOG_FILE) <= MAX_LOG_BYTES);
        assert_eq!(log.entries(5).len(), 5);
        assert!(log.text().lines().count() > 100);
        assert!(log.clear() > 100);
        assert!(log.entries(10).is_empty());
        assert_eq!(log.folder().unwrap(), std::fs::canonicalize(&logs).unwrap());
    }
}
