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
    let mut out = format!("## {} — {}\n**why:** {}\n", input.date, input.title, input.why);
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
