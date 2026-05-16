//! Custom error type so callers (and the test suite) can match on failure
//! modes instead of grepping the message text from `anyhow`.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("no logbook file at {path}. Run `logbook init` first (or set LOGBOOK_FILE to point elsewhere).")]
    NotFound { path: PathBuf },

    #[error("--{flag} must be YYYY-MM-DD (got: \"{value}\")")]
    BadDate { flag: String, value: String },

    #[error("failed to {action} {path}: {source}")]
    Io {
        action: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("git command failed: {0}")]
    Git(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Helper for wrapping a `std::io::Error` with the action and path that
    /// triggered it. Keeps the call sites readable.
    pub fn io(action: impl Into<String>, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            action: action.into(),
            path: path.into(),
            source,
        }
    }
}
