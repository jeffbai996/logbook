//! Core library for `logbook`. The binary in `src/main.rs` is a thin CLI
//! wrapper around the pure functions and types defined here, which lets the
//! `tests/` suite exercise the parser, error paths, and atomic-write logic
//! directly without shelling out.

pub mod error;
pub mod parse;
pub mod store;

pub use error::{Error, Result};
pub use parse::{Entry, parse_entries};
pub use store::{atomic_append, init_file, read_text, render_entry_block, RenderInput};

use chrono::Local;
use std::path::PathBuf;

/// Default filename when `LOGBOOK_FILE` is not set.
pub const DEFAULT_LOGBOOK_FILE: &str = "logbook.md";

/// Environment variable that overrides the default path.
pub const ENV_VAR: &str = "LOGBOOK_FILE";

/// Header written to a freshly-initialized logbook file.
pub const HEADER: &str = "# logbook\n\nAppend-only record of architectural decisions for this project.\nNewest entries at the bottom. Generated and maintained by `logbook` — https://github.com/jeffbai996/logbook\n\n";

/// Resolve the logbook path: `$LOGBOOK_FILE` if set, otherwise `./logbook.md`.
pub fn logbook_path() -> PathBuf {
    std::env::var_os(ENV_VAR)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LOGBOOK_FILE))
}

/// Today's date, formatted as `YYYY-MM-DD` in local time. Centralised so
/// that any future change (UTC, fixed timezone) only needs one edit.
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Cheap shape check used by `show <date>`, `--since`, and `--until`. We
/// don't validate "is this a real day on the Gregorian calendar" — only
/// that it looks like ten characters of `dddd-dd-dd`. Wrong real-world
/// dates just return no matches, which is the right UX.
pub fn is_date_shaped(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let bytes = s.as_bytes();
    bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(|b| b.is_ascii_digit())
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[8..].iter().all(|b| b.is_ascii_digit())
}
