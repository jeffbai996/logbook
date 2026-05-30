//! `$EDITOR` integration for composing an entry's `why` text interactively.
//!
//! When `logbook add` is invoked without `--why`, it opens the user's editor
//! on a temp file (git-commit style), then reads the saved content back as the
//! `why`. Comment lines (starting with `#`) are stripped so a help template
//! can be shown without polluting the entry.
//!
//! The spawn is isolated behind [`capture_via_editor`] so the pure parsing
//! logic ([`strip_comments`]) and editor resolution ([`resolve_editor`]) can be
//! unit-tested without real interactivity.

use crate::error::{Error, Result};
use std::path::Path;
use std::process::Command;

/// Template shown in the editor when composing a `why`. Comment lines are
/// stripped on read, so this guides without polluting the entry.
pub const WHY_TEMPLATE: &str = "\n\
# Write the WHY for this decision above — the reason you chose this design.\n\
# Lines starting with '#' are ignored. An empty message aborts the entry.\n";

/// Resolve which editor command to spawn.
///
/// Checks `$EDITOR` first, then `$VISUAL` (matching git's precedence is
/// `GIT_EDITOR > core.editor > VISUAL > EDITOR`, but for a standalone tool the
/// common convention is `EDITOR` then `VISUAL`). Returns [`Error::NoEditor`] if
/// neither is set or both are empty/whitespace.
pub fn resolve_editor() -> Result<String> {
    for var in ["EDITOR", "VISUAL"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return Ok(v);
            }
        }
    }
    Err(Error::NoEditor)
}

/// Strip comment lines (those whose first non-whitespace char is `#`) and
/// trim surrounding whitespace. Returns the cleaned body.
///
/// # Example
///
/// ```
/// use logbook::editor::strip_comments;
///
/// let raw = "the real reason\n# a comment\nmore reason\n";
/// assert_eq!(strip_comments(raw), "the real reason\nmore reason");
/// ```
pub fn strip_comments(raw: &str) -> String {
    raw.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Spawn `editor_cmd` on `file`, blocking until it exits.
///
/// `editor_cmd` may contain arguments (e.g. `"code --wait"`); it's split on
/// whitespace and the file path appended as the final argument. Returns
/// [`Error::Editor`] if the process fails to start or exits non-zero.
fn spawn_editor(editor_cmd: &str, file: &Path) -> Result<()> {
    let mut parts = editor_cmd.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| Error::Editor("empty editor command".into()))?;
    let status = Command::new(program)
        .args(parts)
        .arg(file)
        .status()
        .map_err(|e| Error::Editor(format!("failed to spawn '{editor_cmd}': {e}")))?;
    if !status.success() {
        return Err(Error::Editor(format!("editor exited with status {status}")));
    }
    Ok(())
}

/// Open the resolved editor on a temp file seeded with [`WHY_TEMPLATE`], read
/// the result back, strip comments, and return the cleaned `why` text.
///
/// Returns [`Error::EmptyEntry`] if the cleaned content is empty (user saved
/// nothing or only comments), [`Error::NoEditor`] if no editor is configured,
/// or [`Error::Editor`] on a spawn/exit failure. The temp file lives in the
/// system temp dir and is removed before returning.
pub fn capture_via_editor() -> Result<String> {
    let editor = resolve_editor()?;
    capture_with(&editor, WHY_TEMPLATE)
}

/// Inner worker: write `template` to a temp file, run `editor` on it, read and
/// clean the result. Separated so tests can pass a fake editor command (e.g. a
/// script that writes a fixed body) without touching `$EDITOR`.
pub fn capture_with(editor: &str, template: &str) -> Result<String> {
    let mut path = std::env::temp_dir();
    // Unique-ish name without a random dep: pid + a monotonic-ish nanos stamp.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.push(format!("logbook-why-{}-{}.md", std::process::id(), stamp));

    std::fs::write(&path, template).map_err(|e| Error::io("write editor temp file", &path, e))?;
    let spawn_result = spawn_editor(editor, &path);
    let read_result = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path); // best-effort cleanup either way

    spawn_result?;
    let raw = read_result.map_err(|e| Error::io("read editor temp file", &path, e))?;
    let cleaned = strip_comments(&raw);
    if cleaned.is_empty() {
        return Err(Error::EmptyEntry);
    }
    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env vars are process-global; serialize the tests that mutate EDITOR/VISUAL
    // so parallel execution can't race one test's set against another's remove.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn strip_comments_removes_hash_lines() {
        assert_eq!(strip_comments("real\n# nope\nmore"), "real\nmore");
    }

    #[test]
    fn strip_comments_trims_and_handles_indented_hash() {
        assert_eq!(strip_comments("\n  # indented comment\nbody\n\n"), "body");
    }

    #[test]
    fn strip_comments_all_comments_is_empty() {
        assert_eq!(strip_comments("# a\n#b\n   # c"), "");
    }

    #[test]
    fn strip_comments_preserves_hash_inside_line() {
        // Only leading-# lines are comments; a '#' mid-line stays.
        assert_eq!(strip_comments("uses C#  and F#"), "uses C#  and F#");
    }

    #[test]
    fn resolve_editor_prefers_editor_over_visual() {
        let _g = ENV_LOCK.lock().unwrap();
        let prev_e = std::env::var("EDITOR").ok();
        let prev_v = std::env::var("VISUAL").ok();
        std::env::set_var("EDITOR", "ed-cmd");
        std::env::set_var("VISUAL", "vis-cmd");
        assert_eq!(resolve_editor().unwrap(), "ed-cmd");
        restore("EDITOR", prev_e);
        restore("VISUAL", prev_v);
    }

    #[test]
    fn resolve_editor_falls_back_to_visual() {
        let _g = ENV_LOCK.lock().unwrap();
        let prev_e = std::env::var("EDITOR").ok();
        let prev_v = std::env::var("VISUAL").ok();
        std::env::remove_var("EDITOR");
        std::env::set_var("VISUAL", "vis-only");
        assert_eq!(resolve_editor().unwrap(), "vis-only");
        restore("EDITOR", prev_e);
        restore("VISUAL", prev_v);
    }

    #[test]
    fn resolve_editor_errors_when_neither_set() {
        let _g = ENV_LOCK.lock().unwrap();
        let prev_e = std::env::var("EDITOR").ok();
        let prev_v = std::env::var("VISUAL").ok();
        std::env::remove_var("EDITOR");
        std::env::remove_var("VISUAL");
        assert!(matches!(resolve_editor(), Err(Error::NoEditor)));
        restore("EDITOR", prev_e);
        restore("VISUAL", prev_v);
    }

    /// Write a tiny executable shell script that appends `body` to the file it's
    /// passed as $1, and return its path. Avoids needing quotes in the editor
    /// command (spawn_editor splits on whitespace, which real editors tolerate
    /// but a `sh -c '...'` one-liner would not).
    fn fake_editor_appending(body: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        p.push(format!(
            "logbook-fake-editor-{}-{stamp}.sh",
            std::process::id()
        ));
        std::fs::write(&p, format!("#!/bin/sh\nprintf '{body}' >> \"$1\"\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        p
    }

    #[test]
    fn capture_with_reads_back_written_content() {
        let ed = fake_editor_appending("the reason\\n");
        let why = capture_with(ed.to_str().unwrap(), WHY_TEMPLATE).unwrap();
        assert_eq!(why, "the reason");
        let _ = std::fs::remove_file(&ed);
    }

    #[test]
    fn capture_with_empty_result_is_empty_entry_error() {
        // Editor that writes nothing real (only the template's comments remain).
        let editor = "true"; // no-op
        assert!(matches!(
            capture_with(editor, WHY_TEMPLATE),
            Err(Error::EmptyEntry)
        ));
    }

    #[test]
    fn capture_with_failing_editor_is_editor_error() {
        let editor = "false"; // exits non-zero
        assert!(matches!(capture_with(editor, ""), Err(Error::Editor(_))));
    }

    fn restore(key: &str, prev: Option<String>) {
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}
