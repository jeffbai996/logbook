//! # logbook
//!
//! Core library for the [`logbook`](https://github.com/jeffbai996/logbook)
//! CLI. The binary in `src/main.rs` is a thin wrapper around the pure
//! functions and types defined here, which lets the integration suite
//! exercise the parser, error paths, and atomic-write logic directly
//! without shelling out.
//!
//! Most consumers should use the CLI; this library is exposed so that
//! tooling (test suites, downstream integrations, alternative front-ends
//! like a web viewer) can manipulate logbook files without re-implementing
//! the format.
//!
//! ## Example: append an entry to a file
//!
//! ```no_run
//! use logbook::{atomic_append, init_file, render_entry_block, RenderInput, today};
//! use std::path::Path;
//!
//! let path = Path::new("logbook.md");
//! init_file(path)?;
//!
//! let date = today();
//! let block = render_entry_block(&RenderInput {
//!     date: &date,
//!     title: "switched to websockets",
//!     why: "polling was hammering the API",
//!     rejected: Some("redis pub/sub (overkill)"),
//!     risk: None,
//!     tags: &["refactor".to_string(), "perf".to_string()],
//! });
//! atomic_append(path, &block)?;
//! # Ok::<(), logbook::Error>(())
//! ```
//!
//! ## Example: parse an existing file
//!
//! ```
//! use logbook::parse_entries;
//!
//! let text = "# logbook\n\n## 2026-05-16 — t\n**why:** w\n**tags:** refactor, perf\n";
//! let entries = parse_entries(text);
//! assert_eq!(entries.len(), 1);
//! assert_eq!(entries[0].date.as_deref(), Some("2026-05-16"));
//! assert_eq!(entries[0].tags, vec!["refactor", "perf"]);
//! ```

pub mod error;
pub mod parse;
pub mod store;

pub use error::{Error, Result};
pub use parse::{parse_entries, Entry};
pub use store::{atomic_append, init_file, read_text, render_entry_block, RenderInput};

use chrono::Local;
use std::path::PathBuf;

/// Default filename used when [`ENV_VAR`] is not set: `logbook.md`.
pub const DEFAULT_LOGBOOK_FILE: &str = "logbook.md";

/// Environment variable that overrides the default logbook path.
///
/// Set `LOGBOOK_FILE` to any path (relative or absolute) and the CLI will
/// read and write that file instead of `./logbook.md`. Useful for
/// monorepos (`LOGBOOK_FILE=docs/decisions.md`) or for keeping a personal
/// log in a fixed location across projects.
pub const ENV_VAR: &str = "LOGBOOK_FILE";

/// Markdown header written to a freshly-initialized logbook file.
///
/// Ends with a blank line so subsequent entries don't merge into the header.
pub const HEADER: &str = "# logbook\n\nAppend-only record of architectural decisions for this project.\nNewest entries at the bottom. Generated and maintained by `logbook` — https://github.com/jeffbai996/logbook\n\n";

/// Resolve the logbook file path.
///
/// Returns `$LOGBOOK_FILE` if set, otherwise `./logbook.md`. The path is
/// not canonicalized — callers can do that themselves if they want an
/// absolute path (the CLI's `where` subcommand does).
///
/// # Example
///
/// ```
/// use logbook::{logbook_path, DEFAULT_LOGBOOK_FILE};
/// use std::path::PathBuf;
///
/// std::env::remove_var("LOGBOOK_FILE");
/// assert_eq!(logbook_path(), PathBuf::from(DEFAULT_LOGBOOK_FILE));
/// ```
pub fn logbook_path() -> PathBuf {
    std::env::var_os(ENV_VAR)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LOGBOOK_FILE))
}

/// Today's date in `YYYY-MM-DD` format, local time.
///
/// Centralized so that any future change to the date convention (UTC,
/// fixed timezone, ISO 8601 with time component) only needs one edit.
///
/// # Example
///
/// ```
/// let today = logbook::today();
/// assert_eq!(today.len(), 10);
/// assert_eq!(today.chars().filter(|c| *c == '-').count(), 2);
/// ```
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Cheap shape check for date arguments.
///
/// Returns `true` iff `s` looks like `YYYY-MM-DD` — ten characters,
/// dashes at positions 4 and 7, digits everywhere else. **This is not
/// calendar validation**: `2026-13-99` shapes correctly even though it
/// isn't a real day. That's intentional — wrong real-world dates just
/// return no matches when used as filters, which is the right UX. Full
/// calendar validation would force us to take on a date-parsing dep
/// like `chrono::NaiveDate::parse_from_str` for negligible value.
///
/// # Example
///
/// ```
/// use logbook::is_date_shaped;
///
/// assert!(is_date_shaped("2026-05-16"));
/// assert!(!is_date_shaped("2026-5-16"));
/// assert!(!is_date_shaped("banana1234"));
/// assert!(!is_date_shaped("2O26-05-16")); // letter O, not zero
/// ```
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

#[cfg(test)]
mod date_tests {
    use super::is_date_shaped;

    #[test]
    fn accepts_valid_shape() {
        assert!(is_date_shaped("2026-05-16"));
        assert!(is_date_shaped("0001-01-01"));
        assert!(is_date_shaped("9999-12-31"));
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(!is_date_shaped(""));
        assert!(!is_date_shaped("2026-5-16"));
        assert!(!is_date_shaped("2026-05-16T00:00:00"));
    }

    #[test]
    fn rejects_non_digit_or_dash_chars() {
        assert!(!is_date_shaped("banana1234"));
        assert!(!is_date_shaped("2026/05/16"));
        assert!(!is_date_shaped("2O26-05-16")); // capital O, not zero
    }

    #[test]
    fn rejects_misplaced_dashes() {
        assert!(!is_date_shaped("20-2605-16"));
        assert!(!is_date_shaped("2026-0516-"));
    }
}
