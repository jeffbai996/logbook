//! File I/O for the logbook. Renders new entry blocks and writes them
//! atomically by staging to a sibling tempfile and renaming on top of the
//! target, so a crashed run cannot leave a half-written entry behind.

use crate::error::{Error, Result};
use crate::HEADER;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Inputs needed to render an entry block. Kept as a struct so the call
/// site reads cleanly and future fields don't break callers.
#[derive(Debug, Clone)]
pub struct RenderInput<'a> {
    pub date: &'a str,
    pub title: &'a str,
    pub why: &'a str,
    pub rejected: Option<&'a str>,
    pub risk: Option<&'a str>,
    pub tags: &'a [String],
}

/// Render a single entry block, ending with a trailing blank line so
/// subsequent entries don't run together.
pub fn render_entry_block(input: &RenderInput<'_>) -> String {
    let mut out = format!(
        "## {} — {}\n**why:** {}\n",
        input.date, input.title, input.why
    );
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
/// Returns true if a new file was created, false if it already existed.
pub fn init_file(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    fs::write(path, HEADER).map_err(|e| Error::io("create", path, e))?;
    Ok(true)
}

/// Read the full contents of a logbook file, returning `NotFound` if the
/// file does not exist.
pub fn read_text(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(Error::NotFound {
            path: path.to_path_buf(),
        });
    }
    fs::read_to_string(path).map_err(|e| Error::io("read", path, e))
}

/// Append `block` to `path` atomically: stage to a sibling tempfile,
/// then `rename()` on top of the original. The rename is atomic on POSIX
/// and on NTFS (via `ReplaceFile`), so a crashed write cannot corrupt
/// the existing file.
///
/// Caller is responsible for ensuring `path` exists (call `init_file`
/// first); we don't auto-init from the library layer.
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
        }
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
