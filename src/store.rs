//! File I/O for the logbook.
//!
//! Renders new entry blocks via [`render_entry_block`] and writes them
//! atomically via [`atomic_append`]: stage to a sibling tempfile, then
//! `rename()` on top of the target. The rename is atomic on POSIX and on
//! NTFS (via `ReplaceFile`), so a crashed run cannot leave a half-written
//! entry behind.

use crate::error::{Error, Result};
use crate::HEADER;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Inputs needed to render an entry block.
///
/// Kept as a borrow-everything struct so the call site reads cleanly and
/// new optional fields can be added in future versions without breaking
/// callers (they'd compile-error on the missing field and add `None`).
///
/// # Example
///
/// ```
/// use logbook::{render_entry_block, RenderInput};
///
/// let tags = vec!["refactor".to_string()];
/// let block = render_entry_block(&RenderInput {
///     date: "2026-05-16",
///     title: "switched ORM",
///     why: "perf",
///     rejected: None,
///     risk: None,
///     tags: &tags,
///     supersedes: None,
/// });
/// assert!(block.starts_with("## 2026-05-16 — switched ORM\n"));
/// ```
#[derive(Debug, Clone)]
pub struct RenderInput<'a> {
    /// Date string, conventionally `YYYY-MM-DD`. Not validated by the
    /// renderer — pass [`crate::today`] if you want today's date.
    pub date: &'a str,
    /// Short single-line title.
    pub title: &'a str,
    /// The mandatory `why` field — the reason for the decision.
    pub why: &'a str,
    /// Optional `rejected` field — alternatives considered and why not.
    /// Empty or whitespace-only strings are omitted from the output.
    pub rejected: Option<&'a str>,
    /// Optional `risk` field — what could go wrong.
    /// Empty or whitespace-only strings are omitted from the output.
    pub risk: Option<&'a str>,
    /// Tags to attach. Per-tag whitespace is trimmed and empty entries
    /// are dropped before rendering. An empty slice omits the tags line.
    pub tags: &'a [String],
    /// Optional `supersedes` field — the `YYYY-MM-DD` date of an earlier
    /// entry this one replaces. Rendered as `**supersedes:** <date>`.
    /// `None` omits the line.
    pub supersedes: Option<&'a str>,
}

/// Render a single entry block as markdown.
///
/// Output ends with a trailing blank line so subsequent entries appended
/// to the same file don't run together. Output is fully deterministic
/// given the input; no global state is consulted.
///
/// Field order in the output is fixed: `## date — title`, then `**why:**`,
/// then optionally `**supersedes:**`, `**rejected:**`, `**risk:**`, `**tags:**`.
pub fn render_entry_block(input: &RenderInput<'_>) -> String {
    let mut out = format!(
        "## {} — {}\n**why:** {}\n",
        input.date, input.title, input.why
    );
    if let Some(s) = input.supersedes.filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("**supersedes:** {s}\n"));
    }
    if let Some(r) = input.rejected.filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("**rejected:** {r}\n"));
    }
    if let Some(r) = input.risk.filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("**risk:** {r}\n"));
    }
    let clean_tags: Vec<&str> = input
        .tags
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if !clean_tags.is_empty() {
        out.push_str(&format!("**tags:** {}\n", clean_tags.join(", ")));
    }
    out.push('\n');
    out
}

/// Create the logbook file with the header, if it doesn't already exist.
///
/// Returns `Ok(true)` if a new file was created, `Ok(false)` if the file
/// already existed (in which case it is left untouched). The header is
/// the constant [`crate::HEADER`].
///
/// Idempotent — safe to call on every `add` to auto-initialize.
pub fn init_file(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    fs::write(path, HEADER).map_err(|e| Error::io("create", path, e))?;
    Ok(true)
}

/// Read the full text of a logbook file.
///
/// Returns [`Error::NotFound`] if the file doesn't exist (so callers can
/// match on it and offer a friendly "run init first" hint). Wraps any
/// other I/O failure as [`Error::Io`].
pub fn read_text(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(Error::NotFound {
            path: path.to_path_buf(),
        });
    }
    fs::read_to_string(path).map_err(|e| Error::io("read", path, e))
}

/// Append `block` to `path` atomically.
///
/// The implementation stages to a sibling tempfile, then `rename()`s on
/// top of the original. POSIX `rename(2)` and Windows `MoveFileEx`/
/// `ReplaceFile` are both atomic at the filesystem level — a crashed
/// run, power loss, or `kill -9` mid-write cannot leave the file in a
/// partially-overwritten state. Either the new content is fully visible
/// or the old content is, never a mix.
///
/// If `path` does not yet exist, this still works (no existing content
/// to preserve, so the tempfile starts empty before appending `block`).
/// Callers wanting the standard header should call [`init_file`] first.
///
/// The tempfile lives next to `path` (same parent directory) so the
/// rename stays on one filesystem — cross-filesystem renames would
/// fail with `EXDEV`.
pub fn atomic_append(path: &Path, block: &str) -> Result<()> {
    let existing = if path.exists() {
        fs::read(path).map_err(|e| Error::io("read", path, e))?
    } else {
        Vec::new()
    };

    let tmp = tmp_path_for(path);
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|e| Error::io("open temp file", &tmp, e))?;
        f.write_all(&existing)
            .map_err(|e| Error::io("copy existing contents to", &tmp, e))?;
        f.write_all(block.as_bytes())
            .map_err(|e| Error::io("write new entry to", &tmp, e))?;
        // sync_all is best-effort: if it fails, the rename will still
        // succeed and the user will get a slightly older view on a
        // truly catastrophic power-loss. Acceptable for human-pace use.
        let _ = f.sync_all();
    }
    fs::rename(&tmp, path).map_err(|e| Error::io("rename temp file to", path, e))?;
    Ok(())
}

/// Build a tempfile path next to `path` with the same parent dir, so
/// `rename()` stays on the same filesystem (cross-fs rename would error).
fn tmp_path_for(path: &Path) -> std::path::PathBuf {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("md");
    path.with_extension(format!("{ext}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn ri<'a>(
        date: &'a str,
        title: &'a str,
        why: &'a str,
        rejected: Option<&'a str>,
        risk: Option<&'a str>,
        tags: &'a [String],
    ) -> RenderInput<'a> {
        RenderInput {
            date,
            title,
            why,
            rejected,
            risk,
            tags,
            supersedes: None,
        }
    }

    #[test]
    fn renders_supersedes_after_why() {
        let tags: Vec<String> = vec![];
        let mut input = ri("2026-05-16", "t", "w", None, None, &tags);
        input.supersedes = Some("2026-05-01");
        let out = render_entry_block(&input);
        assert_eq!(
            out,
            "## 2026-05-16 — t\n**why:** w\n**supersedes:** 2026-05-01\n\n"
        );
    }

    #[test]
    fn omits_empty_supersedes() {
        let tags: Vec<String> = vec![];
        let mut input = ri("2026-05-16", "t", "w", None, None, &tags);
        input.supersedes = Some("   ");
        let out = render_entry_block(&input);
        assert_eq!(out, "## 2026-05-16 — t\n**why:** w\n\n");
    }

    #[test]
    fn renders_minimal_entry() {
        let tags: Vec<String> = vec![];
        let out = render_entry_block(&ri("2026-05-16", "t", "w", None, None, &tags));
        assert_eq!(out, "## 2026-05-16 — t\n**why:** w\n\n");
    }

    #[test]
    fn renders_full_entry_in_canonical_order() {
        let tags = vec!["a".into(), "b".into()];
        let out = render_entry_block(&ri("2026-05-16", "t", "w", Some("rej"), Some("rsk"), &tags));
        let expected =
            "## 2026-05-16 — t\n**why:** w\n**rejected:** rej\n**risk:** rsk\n**tags:** a, b\n\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn omits_empty_optional_fields() {
        let tags: Vec<String> = vec![];
        let out = render_entry_block(&ri("2026-05-16", "t", "w", Some("   "), Some(""), &tags));
        // Whitespace-only rejected/risk should be dropped, not rendered as blank lines.
        assert_eq!(out, "## 2026-05-16 — t\n**why:** w\n\n");
    }

    #[test]
    fn trims_and_drops_empty_tags() {
        let tags = vec!["  refactor  ".into(), "".into(), "perf".into()];
        let out = render_entry_block(&ri("2026-05-16", "t", "w", None, None, &tags));
        assert!(out.contains("**tags:** refactor, perf\n"));
    }

    #[test]
    fn init_file_creates_then_is_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("logbook.md");
        assert!(init_file(&path).unwrap());
        assert!(!init_file(&path).unwrap()); // already exists
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("# logbook"));
    }

    #[test]
    fn read_text_returns_not_found_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.md");
        let err = read_text(&path).unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[test]
    fn atomic_append_writes_block_and_preserves_existing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("logbook.md");
        init_file(&path).unwrap();
        let original = std::fs::read_to_string(&path).unwrap();

        let block = "## 2026-05-16 — t\n**why:** w\n\n";
        atomic_append(&path, block).unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(after, format!("{original}{block}"));
    }

    #[test]
    fn atomic_append_leaves_no_tmp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("logbook.md");
        init_file(&path).unwrap();
        atomic_append(&path, "## 2026-05-16 — t\n**why:** w\n\n").unwrap();

        // No "logbook.md.tmp" lingering after a successful rename.
        let tmp = dir.path().join("logbook.md.tmp");
        assert!(
            !tmp.exists(),
            "tempfile should be renamed away, not left behind"
        );
    }
}
