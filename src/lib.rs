//! Parser, renderer, and filesystem support for the `logbook` CLI.

mod color;
mod editor;
mod error;
mod export;
mod parse;
mod path;
mod store;
mod validate;

pub use color::{colorize_block, should_colorize, ColorChoice};
pub use editor::capture_via_editor;
pub use error::{Error, Result};
pub use export::{check_report_to_json, entries_to_json, entries_to_json_lines};
pub use parse::{parse_entries, Entry};
pub use path::resolve_logbook_path;
pub use store::{
    atomic_append, atomic_append_checked, init_file, read_text, render_entry_block, RenderInput,
};
pub use validate::{validate_entries, ValidationIssue};

use chrono::Local;
use std::path::PathBuf;

/// Default decision-log filename.
pub const DEFAULT_LOGBOOK_FILE: &str = "logbook.md";

/// Environment variable that overrides [`DEFAULT_LOGBOOK_FILE`].
pub const ENV_VAR: &str = "LOGBOOK_FILE";

/// Header written to a new logbook.
pub const HEADER: &str = "# logbook\n\nAppend-only record of decisions for this project.\nNewest entries are at the bottom.\n\n";

/// Return the resolved logbook path, falling back to `logbook.md` if the
/// current directory cannot be read.
pub fn logbook_path() -> PathBuf {
    resolve_logbook_path(None).unwrap_or_else(|_| PathBuf::from(DEFAULT_LOGBOOK_FILE))
}

/// Return today's local date as `YYYY-MM-DD`.
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Check the byte shape `YYYY-MM-DD` without calendar validation.
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

/// Check that a value is a real calendar date in `YYYY-MM-DD` form.
pub fn is_valid_date(value: &str) -> bool {
    is_date_shaped(value) && chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}

#[cfg(test)]
mod tests {
    use super::{is_date_shaped, is_valid_date};

    #[test]
    fn recognizes_date_shape() {
        for valid in ["2026-05-16", "0001-01-01", "9999-12-31"] {
            assert!(is_date_shaped(valid), "{valid}");
        }
        for invalid in [
            "",
            "2026-5-16",
            "2026-05-16T00:00:00",
            "banana1234",
            "2026/05/16",
            "2O26-05-16",
            "20-2605-16",
            "2026-0516-",
        ] {
            assert!(!is_date_shaped(invalid), "{invalid}");
        }
    }

    #[test]
    fn validates_calendar_dates() {
        assert!(is_valid_date("2024-02-29"));
        assert!(!is_valid_date("2023-02-29"));
        assert!(!is_valid_date("2026-13-01"));
        assert!(!is_valid_date("banana"));
    }
}
