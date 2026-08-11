//! `$EDITOR` integration for composing the decision reason.

use crate::error::{Error, Result};
use std::path::Path;
use std::process::Command;

const WHY_TEMPLATE: &str = "\n\
# Write the WHY for this decision above — the reason you chose this design.\n\
# Lines starting with '#' are ignored. An empty message aborts the entry.\n";

fn resolve_editor() -> Result<String> {
    for var in ["EDITOR", "VISUAL"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return Ok(v);
            }
        }
    }
    Err(Error::NoEditor)
}

fn strip_comments(raw: &str) -> String {
    raw.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Spawn `editor_cmd` on `file`, blocking until it exits.
fn spawn_editor(editor_cmd: &str, file: &Path) -> Result<()> {
    let parts = split_editor_command(editor_cmd)?;
    let status = Command::new(&parts[0])
        .args(&parts[1..])
        .arg(file)
        .status()
        .map_err(|e| Error::Editor(format!("failed to spawn '{editor_cmd}': {e}")))?;
    if !status.success() {
        return Err(Error::Editor(format!("editor exited with status {status}")));
    }
    Ok(())
}

fn split_editor_command(command: &str) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote = None;

    for character in command.chars() {
        match (quote, character) {
            (Some(open), close) if open == close => quote = None,
            (Some(_), value) => current.push(value),
            (None, value @ ('\'' | '"')) => quote = Some(value),
            (None, value) if value.is_whitespace() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            (None, value) => current.push(value),
        }
    }
    if quote.is_some() {
        return Err(Error::Editor("unclosed quote in editor command".into()));
    }
    if !current.is_empty() {
        parts.push(current);
    }
    if parts.is_empty() {
        return Err(Error::Editor("empty editor command".into()));
    }
    Ok(parts)
}

/// Capture a decision reason with `$EDITOR`, falling back to `$VISUAL`.
pub fn capture_via_editor() -> Result<String> {
    let editor = resolve_editor()?;
    capture_with(&editor, WHY_TEMPLATE)
}

fn capture_with(editor: &str, template: &str) -> Result<String> {
    let mut path = std::env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.push(format!("logbook-why-{}-{}.md", std::process::id(), stamp));

    std::fs::write(&path, template).map_err(|e| Error::io("write editor temp file", &path, e))?;
    let spawn_result = spawn_editor(editor, &path);
    let read_result = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path);

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

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn strip_comments_handles_comments_whitespace_and_inline_hashes() {
        assert_eq!(strip_comments("real\n# nope\nmore"), "real\nmore");
        assert_eq!(strip_comments("\n  # indented comment\nbody\n\n"), "body");
        assert_eq!(strip_comments("# a\n#b\n   # c"), "");
        assert_eq!(strip_comments("uses C#  and F#"), "uses C#  and F#");
    }

    #[test]
    fn editor_command_supports_quoted_paths_and_arguments() {
        assert_eq!(
            split_editor_command(r#""C:\Program Files\Editor\edit.exe" --wait"#).unwrap(),
            [r"C:\Program Files\Editor\edit.exe", "--wait"]
        );
        assert!(split_editor_command("\"unterminated").is_err());
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

    fn fake_editor_appending(body: &str, path_has_spaces: bool) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.push(format!(
            "logbook{}editor-{}-{stamp}",
            if path_has_spaces { " " } else { "-" },
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let mut p = dir;
        #[cfg(unix)]
        p.push(format!(
            "logbook-fake-editor-{}-{stamp}.sh",
            std::process::id()
        ));
        #[cfg(windows)]
        p.push(format!(
            "logbook-fake-editor-{}-{stamp}.cmd",
            std::process::id()
        ));
        #[cfg(unix)]
        std::fs::write(&p, format!("#!/bin/sh\nprintf '{body}' >> \"$1\"\n")).unwrap();
        #[cfg(windows)]
        std::fs::write(
            &p,
            format!(
                "@echo off\r\necho {}>> \"%~1\"\r\n",
                body.trim_end_matches("\\n")
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        p
    }

    #[test]
    fn capture_with_reads_back_written_content() {
        let ed = fake_editor_appending("the reason\\n", false);
        let why = capture_with(ed.to_str().unwrap(), WHY_TEMPLATE).unwrap();
        assert_eq!(why, "the reason");
        std::fs::remove_dir_all(ed.parent().unwrap()).unwrap();
    }

    #[test]
    fn capture_with_accepts_a_quoted_editor_path() {
        let ed = fake_editor_appending("quoted path\\n", true);
        let command = format!("\"{}\"", ed.display());
        let why = capture_with(&command, WHY_TEMPLATE).unwrap();
        assert_eq!(why, "quoted path");
        std::fs::remove_dir_all(ed.parent().unwrap()).unwrap();
    }

    #[test]
    fn capture_with_empty_result_is_empty_entry_error() {
        let ed = fake_editor_appending("", false);
        assert!(matches!(
            capture_with(ed.to_str().unwrap(), WHY_TEMPLATE),
            Err(Error::EmptyEntry)
        ));
        std::fs::remove_dir_all(ed.parent().unwrap()).unwrap();
    }

    #[test]
    fn capture_with_failing_editor_is_editor_error() {
        assert!(matches!(capture_with("false", ""), Err(Error::Editor(_))));
    }

    fn restore(key: &str, prev: Option<String>) {
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}
