//! Error types for the `logbook` library.
//!
//! Custom enum rather than `anyhow` so that callers (and the test suite)
//! can `match` on failure modes instead of grepping message strings. The
//! `Display` impl produces messages suitable for printing directly to
//! stderr; the `Debug` impl surfaces every field for richer reporting
//! tools.

use std::path::PathBuf;
use thiserror::Error;

/// All failure modes the library can produce.
///
/// Each variant carries the context a caller needs to either recover or
/// report meaningfully:
///
/// - [`Error::NotFound`] — the logbook file doesn't exist. Recoverable
///   by running `init` (or, programmatically, [`crate::init_file`]).
/// - [`Error::BadDate`] — an argument that should have been `YYYY-MM-DD`
///   wasn't. Caller-facing input error; not recoverable.
/// - [`Error::Io`] — wraps a `std::io::Error` with the action attempted
///   and the path involved, so the message reads as
///   `failed to {action} {path}: {underlying}`.
/// - [`Error::Git`] — the `git add` shell-out (used by `--stage`)
///   failed or was unable to start.
#[derive(Error, Debug)]
pub enum Error {
    /// The logbook file does not exist at the resolved path.
    ///
    /// Returned by reading operations ([`crate::read_text`], anything
    /// that depends on `read_entries`). Writing operations auto-create
    /// the file and never return this.
    #[error("no logbook file at {path}. Run `logbook init` first (or set LOGBOOK_FILE to point elsewhere).")]
    NotFound {
        /// The path that was searched.
        path: PathBuf,
    },

    /// A CLI argument expected to be `YYYY-MM-DD` had the wrong shape.
    ///
    /// `flag` names the offending flag (e.g. `"since"`, `"date"`) so
    /// the message reads cleanly to the user. `value` is the literal
    /// string they passed.
    #[error("--{flag} must be YYYY-MM-DD (got: \"{value}\")")]
    BadDate {
        /// The flag name without its `--` prefix.
        flag: String,
        /// The malformed value as the user supplied it.
        value: String,
    },

    /// Wraps a `std::io::Error` with the action that triggered it and
    /// the path involved.
    ///
    /// Construct via [`Error::io`] rather than building the variant
    /// directly — the helper keeps call sites readable.
    #[error("failed to {action} {path}: {source}")]
    Io {
        /// Human-readable verb phrase, e.g. `"read"`, `"rename temp file to"`.
        action: String,
        /// The path the operation was acting on.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The `git add` subprocess (used by `--stage`) failed to spawn or
    /// exited non-zero.
    #[error("git command failed: {0}")]
    Git(String),
}

/// Shorthand for `std::result::Result<T, logbook::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Wrap a `std::io::Error` with context about what was being attempted.
    ///
    /// Reads more cleanly than constructing [`Error::Io`] directly.
    ///
    /// # Example
    ///
    /// ```
    /// use logbook::Error;
    /// use std::io;
    /// use std::path::PathBuf;
    ///
    /// let raw = io::Error::new(io::ErrorKind::NotFound, "boom");
    /// let err = Error::io("read", PathBuf::from("/tmp/x"), raw);
    /// assert!(err.to_string().contains("/tmp/x"));
    /// ```
    pub fn io(action: impl Into<String>, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            action: action.into(),
            path: path.into(),
            source,
        }
    }
}
