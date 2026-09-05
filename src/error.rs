//! User-facing error types.

use std::path::PathBuf;
use thiserror::Error;

/// Failures produced by the library and CLI.
#[derive(Error, Debug)]
pub enum Error {
    /// The configured logbook does not exist.
    #[error("no logbook file at {path}. Run `logbook init` first (or set LOGBOOK_FILE to point elsewhere).")]
    NotFound { path: PathBuf },

    /// A CLI date argument did not have the required shape.
    #[error("--{flag} must be YYYY-MM-DD (got: \"{value}\")")]
    BadDate { flag: String, value: String },

    /// A filesystem operation failed.
    #[error("failed to {action} {path}: {source}")]
    Io {
        action: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// `git add` failed.
    #[error("git command failed: {0}")]
    Git(String),

    /// Another process held the append lock for too long.
    #[error(
        "timed out waiting for the logbook write lock at {0}; if no logbook process is writing, remove that lock directory and retry"
    )]
    Locked(PathBuf),

    /// User input would produce a malformed canonical entry.
    #[error("invalid entry: {0}")]
    InvalidEntry(String),

    /// Reading a piped decision reason failed.
    #[error("failed to read why text from stdin: {0}")]
    Stdin(#[source] std::io::Error),

    /// Writing command output failed.
    #[error("failed to write command output: {0}")]
    Output(#[source] std::io::Error),

    /// A source logbook failed structural validation.
    #[error("logbook check failed with {0} problem(s)")]
    CheckFailed(usize),

    /// Neither `$EDITOR` nor `$VISUAL` is configured.
    #[error("no editor configured — set $EDITOR or $VISUAL, or pass --why directly")]
    NoEditor,

    /// The configured editor failed.
    #[error("editor command failed: {0}")]
    Editor(String),

    /// The editor produced no decision reason.
    #[error("aborting: empty entry (no why text was provided)")]
    EmptyEntry,

    /// No entry exists on the requested date.
    #[error("no entry dated {0} to supersede — check `logbook list` for valid dates")]
    SupersedeTargetMissing(String),

    /// More than one entry matches the supplied selector.
    #[error("more than one entry matches {0} — pass --old-title with the exact title")]
    SupersedeTargetAmbiguous(String),

    /// No entry on a date has the supplied title.
    #[error("no entry dated {date} has title \"{title}\"")]
    SupersedeTitleMissing { date: String, title: String },

    /// The selected decision already has a successor.
    #[error("cannot supersede {0}: it is already superseded")]
    SupersedeTargetInactive(String),

    /// A dated title would duplicate an existing decision reference.
    #[error("a decision already exists at {0}")]
    DuplicateDecision(String),
}

/// Result type used throughout logbook.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn io(
        action: impl Into<String>,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Error::Io {
            action: action.into(),
            path: path.into(),
            source,
        }
    }
}
