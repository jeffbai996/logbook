//! Entry rendering and filesystem operations.

use crate::{Error, Result, HEADER};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Borrowed fields for rendering one canonical entry.
#[derive(Debug, Clone)]
pub struct RenderInput<'a> {
    pub date: &'a str,
    pub title: &'a str,
    pub why: &'a str,
    pub rejected: Option<&'a str>,
    pub risk: Option<&'a str>,
    pub tags: &'a [String],
    /// Reference to the superseded decision, normally `date — title`.
    pub supersedes: Option<&'a str>,
}

/// Render a canonical Markdown block ending with a blank line.
pub fn render_entry_block(input: &RenderInput<'_>) -> String {
    let mut out = format!(
        "## {} — {}\n**why:** {}\n",
        input.date, input.title, input.why
    );
    if let Some(value) = non_empty(input.supersedes) {
        out.push_str(&format!("**supersedes:** {value}\n"));
    }
    if let Some(value) = non_empty(input.rejected) {
        out.push_str(&format!("**rejected:** {value}\n"));
    }
    if let Some(value) = non_empty(input.risk) {
        out.push_str(&format!("**risk:** {value}\n"));
    }
    let tags: Vec<&str> = input
        .tags
        .iter()
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty())
        .collect();
    if !tags.is_empty() {
        out.push_str(&format!("**tags:** {}\n", tags.join(", ")));
    }
    out.push('\n');
    out
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

/// Create a new logbook header without changing an existing file.
pub fn init_file(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    fs::write(path, HEADER).map_err(|error| Error::io("create", path, error))?;
    Ok(true)
}

/// Read the configured logbook as UTF-8 text.
pub fn read_text(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(Error::NotFound {
            path: path.to_path_buf(),
        });
    }
    fs::read_to_string(path).map_err(|error| Error::io("read", path, error))
}

/// Replace `path` atomically with its existing bytes followed by `block`.
pub fn atomic_append(path: &Path, block: &str) -> Result<()> {
    let (existing, permissions) = if path.exists() {
        let bytes = fs::read(path).map_err(|error| Error::io("read", path, error))?;
        let permissions = fs::metadata(path)
            .map_err(|error| Error::io("read metadata for", path, error))?
            .permissions();
        (bytes, Some(permissions))
    } else {
        (Vec::new(), None)
    };

    let tmp = tmp_path_for(path);
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|error| Error::io("open temp file", &tmp, error))?;
        file.write_all(&existing)
            .map_err(|error| Error::io("copy existing contents to", &tmp, error))?;
        file.write_all(block.as_bytes())
            .map_err(|error| Error::io("write new entry to", &tmp, error))?;
        let _ = file.sync_all();
        drop(file);
        if let Some(permissions) = permissions {
            fs::set_permissions(&tmp, permissions)
                .map_err(|error| Error::io("preserve permissions on", &tmp, error))?;
        }
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(Error::io("rename temp file to", path, error));
    }
    Ok(())
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let count = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("logbook"));
    name.push(format!(".tmp-{}-{stamp}-{count}", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn input<'a>(tags: &'a [String]) -> RenderInput<'a> {
        RenderInput {
            date: "2026-05-16",
            title: "t",
            why: "w",
            rejected: None,
            risk: None,
            tags,
            supersedes: None,
        }
    }

    #[test]
    fn renders_minimal_and_full_entries() {
        assert_eq!(
            render_entry_block(&input(&[])),
            "## 2026-05-16 — t\n**why:** w\n\n"
        );
        let tags = vec![" a ".into(), "".into(), "b".into()];
        let mut full = input(&tags);
        full.supersedes = Some("2026-05-01 — old");
        full.rejected = Some("rej");
        full.risk = Some("rsk");
        assert_eq!(
            render_entry_block(&full),
            "## 2026-05-16 — t\n**why:** w\n**supersedes:** 2026-05-01 — old\n**rejected:** rej\n**risk:** rsk\n**tags:** a, b\n\n"
        );
    }

    #[test]
    fn omits_blank_optional_fields_and_tags() {
        let tags = vec![" ".into()];
        let mut value = input(&tags);
        value.supersedes = Some(" ");
        value.rejected = Some("");
        value.risk = Some("   ");
        assert_eq!(
            render_entry_block(&value),
            "## 2026-05-16 — t\n**why:** w\n\n"
        );
    }

    #[test]
    fn init_is_idempotent_and_read_reports_missing_files() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("logbook.md");
        assert!(matches!(read_text(&path), Err(Error::NotFound { .. })));
        assert!(init_file(&path).unwrap());
        assert!(!init_file(&path).unwrap());
        assert!(read_text(&path).unwrap().starts_with("# logbook"));
    }

    #[test]
    fn atomic_append_preserves_existing_content_and_cleans_up() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("logbook.md");
        init_file(&path).unwrap();
        let original = read_text(&path).unwrap();
        let block = "## 2026-05-16 — t\n**why:** w\n\n";
        atomic_append(&path, block).unwrap();
        assert_eq!(read_text(&path).unwrap(), format!("{original}{block}"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn temp_paths_are_unique_and_adjacent() {
        let path = Path::new("somewhere/logbook.md");
        let first = tmp_path_for(path);
        let second = tmp_path_for(path);
        assert_ne!(first, second);
        assert_eq!(first.parent(), path.parent());
    }

    #[cfg(unix)]
    #[test]
    fn atomic_append_preserves_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("logbook.md");
        init_file(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        atomic_append(&path, "## 2026-05-16 — t\n**why:** w\n\n").unwrap();
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
