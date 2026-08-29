//! Repository-aware logbook path resolution.

use crate::{Error, Result, DEFAULT_LOGBOOK_FILE, ENV_VAR};
use std::path::{Path, PathBuf};

/// Resolve an explicit path, `$LOGBOOK_FILE`, or the nearest repository logbook.
///
/// Relative overrides are resolved from the current directory. Without an
/// override, resolution walks upward to the nearest Git root and returns the
/// first existing `logbook.md`. If none exists, the prospective path is
/// `logbook.md` at that root, or in the current directory outside Git.
pub fn resolve_logbook_path(explicit: Option<&Path>) -> Result<PathBuf> {
    let current = std::env::current_dir()
        .map_err(|error| Error::io("resolve current directory from", ".", error))?;
    resolve_logbook_path_from(&current, explicit, std::env::var_os(ENV_VAR).as_deref())
}

fn resolve_logbook_path_from(
    current: &Path,
    explicit: Option<&Path>,
    environment: Option<&std::ffi::OsStr>,
) -> Result<PathBuf> {
    if let Some(path) =
        explicit.or_else(|| environment.filter(|value| !value.is_empty()).map(Path::new))
    {
        return Ok(if path.is_absolute() {
            path.to_path_buf()
        } else {
            current.join(path)
        });
    }

    let git_root = current
        .ancestors()
        .find(|directory| directory.join(".git").exists());
    for directory in current.ancestors() {
        let candidate = directory.join(DEFAULT_LOGBOOK_FILE);
        if candidate.is_file() {
            return Ok(candidate);
        }
        if git_root == Some(directory) {
            break;
        }
    }
    Ok(git_root.unwrap_or(current).join(DEFAULT_LOGBOOK_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discovers_a_logbook_from_a_nested_repository_directory() {
        let directory = tempdir().unwrap();
        std::fs::create_dir(directory.path().join(".git")).unwrap();
        std::fs::write(directory.path().join(DEFAULT_LOGBOOK_FILE), "# logbook\n").unwrap();
        let nested = directory.path().join("src/deep");
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(
            resolve_logbook_path_from(&nested, None, Some(std::ffi::OsStr::new(""))).unwrap(),
            directory.path().join(DEFAULT_LOGBOOK_FILE)
        );
    }

    #[test]
    fn defaults_to_the_nearest_git_root_without_crossing_it() {
        let directory = tempdir().unwrap();
        std::fs::write(directory.path().join(DEFAULT_LOGBOOK_FILE), "# parent\n").unwrap();
        let repository = directory.path().join("repo");
        let nested = repository.join("src");
        std::fs::create_dir_all(repository.join(".git")).unwrap();
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(
            resolve_logbook_path_from(&nested, None, None).unwrap(),
            repository.join(DEFAULT_LOGBOOK_FILE)
        );
    }

    #[test]
    fn explicit_path_wins_over_environment_and_relative_paths_use_current_dir() {
        let directory = tempdir().unwrap();
        assert_eq!(
            resolve_logbook_path_from(
                directory.path(),
                Some(Path::new("docs/choices.md")),
                Some(std::ffi::OsStr::new("ignored.md")),
            )
            .unwrap(),
            directory.path().join("docs/choices.md")
        );
    }
}
