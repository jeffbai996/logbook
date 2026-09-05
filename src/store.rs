//! Entry rendering and filesystem operations.

use crate::{Error, Result, HEADER};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(10);

#[cfg(windows)]
const WINDOWS_LOCK_DELETE_RETRIES: usize = 50;

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
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            if let Err(error) = file
                .write_all(HEADER.as_bytes())
                .and_then(|()| file.sync_all())
            {
                drop(file);
                let _ = fs::remove_file(path);
                return Err(Error::io("create", path, error));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(Error::io("create", path, error)),
    }
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

/// Serialize appenders and atomically replace `path` with `block` appended.
pub fn atomic_append(path: &Path, block: &str) -> Result<()> {
    atomic_append_checked(path, block, |_| Ok(()))
}

/// Validate the current text and append `block` under one write lock.
///
/// The callback runs after the editor or other input flow has completed, so
/// callers can re-check semantic invariants without holding the lock while a
/// human writes.
pub fn atomic_append_checked<F>(path: &Path, block: &str, check: F) -> Result<()>
where
    F: FnOnce(&str) -> Result<()>,
{
    let _lock = AppendLock::acquire(path)?;
    let (existing, permissions) = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(Error::InvalidEntry(format!(
                "refusing to write through symlink at {}; pass --file with the target path instead",
                path.display()
            )))
        }
        Ok(metadata) => {
            let text = fs::read_to_string(path).map_err(|error| Error::io("read", path, error))?;
            (text, Some(metadata.permissions()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (String::new(), None),
        Err(error) => return Err(Error::io("read metadata for", path, error)),
    };
    check(&existing)?;

    let tmp = tmp_path_for(path);
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|error| Error::io("open temp file", &tmp, error))?;
        file.write_all(existing.as_bytes())
            .map_err(|error| Error::io("copy existing contents to", &tmp, error))?;
        file.write_all(append_separator(&existing).as_bytes())
            .map_err(|error| Error::io("separate new entry in", &tmp, error))?;
        file.write_all(block.as_bytes())
            .map_err(|error| Error::io("write new entry to", &tmp, error))?;
        file.sync_all()
            .map_err(|error| Error::io("sync", &tmp, error))?;
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

fn append_separator(existing: &str) -> &'static str {
    if existing.is_empty() || existing.ends_with("\n\n") || existing.ends_with("\r\n\r\n") {
        ""
    } else if existing.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    }
}

struct AppendLock {
    path: PathBuf,
}

impl AppendLock {
    fn acquire(path: &Path) -> Result<Self> {
        Self::acquire_with_timeout(path, Duration::from_secs(5))
    }

    fn acquire_with_timeout(path: &Path, timeout: Duration) -> Result<Self> {
        let lock_path = lock_path_for(path);
        let started = Instant::now();
        #[cfg(windows)]
        let mut access_denied_retries = 0;
        loop {
            match fs::create_dir(&lock_path) {
                Ok(()) => return Ok(Self { path: lock_path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    #[cfg(windows)]
                    {
                        access_denied_retries = 0;
                    }
                    if started.elapsed() >= timeout {
                        return Err(Error::Locked(lock_path));
                    }
                    std::thread::sleep(LOCK_RETRY_DELAY);
                }
                // Windows can report ERROR_ACCESS_DENIED while a just-released
                // lock directory is still being deleted. Retry that brief state,
                // but preserve persistent permission failures as I/O errors.
                #[cfg(windows)]
                Err(error)
                    if error.kind() == std::io::ErrorKind::PermissionDenied
                        && access_denied_retries < WINDOWS_LOCK_DELETE_RETRIES =>
                {
                    access_denied_retries += 1;
                    std::thread::sleep(LOCK_RETRY_DELAY);
                }
                Err(error) => return Err(Error::io("create write lock", &lock_path, error)),
            }
        }
    }
}

impl Drop for AppendLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

fn lock_path_for(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("logbook"));
    name.push(".lock");
    path.with_file_name(name)
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
    fn concurrent_initialization_never_truncates_an_existing_file() {
        let dir = tempdir().unwrap();
        let path = std::sync::Arc::new(dir.path().join("logbook.md"));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let path = std::sync::Arc::clone(&path);
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    init_file(&path).unwrap()
                })
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(|created| *created)
                .count(),
            1
        );

        atomic_append(&path, "## 2026-05-16 — kept\n**why:** w\n\n").unwrap();
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let path = std::sync::Arc::clone(&path);
                std::thread::spawn(move || init_file(&path).unwrap())
            })
            .collect();
        assert!(handles.into_iter().all(|handle| !handle.join().unwrap()));
        assert!(read_text(&path).unwrap().contains("— kept"));
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
    fn atomic_append_inserts_a_parseable_boundary_after_hand_edits() {
        let block = "## 2026-05-17 — new\n**why:** new reason\n\n";
        for (index, existing) in [
            "",
            "## 2026-05-16 — old\n**why:** old reason",
            "## 2026-05-16 — old\n**why:** old reason\n",
            "## 2026-05-16 — old\n**why:** old reason\n\n",
            "## 2026-05-16 — old\r\n**why:** old reason\r\n",
            "## 2026-05-16 — old\r\n**why:** old reason\r\n\r\n",
        ]
        .into_iter()
        .enumerate()
        {
            let dir = tempdir().unwrap();
            let path = dir.path().join(format!("logbook-{index}.md"));
            fs::write(&path, existing).unwrap();

            atomic_append(&path, block).unwrap();

            let text = read_text(&path).unwrap();
            assert!(text.starts_with(existing));
            assert_eq!(
                crate::parse_entries(&text).last().unwrap().title.as_deref(),
                Some("new")
            );
            assert_eq!(
                crate::parse_entries(&text).len(),
                if existing.is_empty() { 1 } else { 2 }
            );
        }
    }

    #[test]
    fn an_existing_lock_is_never_stolen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("logbook.md");
        let lock = lock_path_for(&path);
        fs::create_dir(&lock).unwrap();

        let error = match AppendLock::acquire_with_timeout(&path, Duration::ZERO) {
            Ok(_) => panic!("existing lock should block acquisition"),
            Err(error) => error,
        };

        assert!(matches!(error, Error::Locked(ref value) if value == &lock));
        assert!(lock.is_dir());
    }

    #[test]
    fn temp_paths_are_unique_and_adjacent() {
        let path = Path::new("somewhere/logbook.md");
        let first = tmp_path_for(path);
        let second = tmp_path_for(path);
        assert_ne!(first, second);
        assert_eq!(first.parent(), path.parent());
    }

    #[test]
    fn concurrent_appends_do_not_lose_entries() {
        let dir = tempdir().unwrap();
        let path = std::sync::Arc::new(dir.path().join("logbook.md"));
        init_file(&path).unwrap();

        let handles: Vec<_> = (0..16)
            .map(|index| {
                let path = std::sync::Arc::clone(&path);
                std::thread::spawn(move || {
                    atomic_append(
                        &path,
                        &format!("## 2026-05-16 — entry {index}\n**why:** w\n\n"),
                    )
                    .unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let text = read_text(&path).unwrap();
        assert_eq!(crate::parse_entries(&text).len(), 16);
        assert!(!lock_path_for(&path).exists());
    }

    #[cfg(unix)]
    #[test]
    fn atomic_append_refuses_to_replace_a_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("decisions.md");
        let link = dir.path().join("logbook.md");
        fs::write(&target, "# original\n").unwrap();
        symlink(&target, &link).unwrap();

        let error = atomic_append(&link, "## 2026-05-16 — no\n**why:** no\n\n").unwrap_err();

        assert!(matches!(
            error,
            Error::InvalidEntry(ref message)
                if message.contains("refusing to write through symlink")
        ));
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(read_text(&target).unwrap(), "# original\n");
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
