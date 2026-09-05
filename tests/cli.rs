//! End-to-end CLI contract tests.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use tempfile::TempDir;

/// Helper: a tempdir + the absolute path to a `logbook.md` inside it,
/// passed via LOGBOOK_FILE so the binary writes there instead of cwd.
struct Sandbox {
    dir: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Sandbox {
            dir: TempDir::new().unwrap(),
        }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("logbook.md")
    }

    /// Construct a `logbook` invocation already configured to use this
    /// sandbox's path via LOGBOOK_FILE.
    fn cmd(&self) -> Command {
        let mut c = Command::cargo_bin("logbook").unwrap();
        c.env("LOGBOOK_FILE", self.path());
        c
    }

    fn seed(&self, body: &str) {
        std::fs::write(self.path(), body).unwrap();
    }
}

#[test]
fn init_creates_logbook_with_header() {
    let sb = Sandbox::new();
    sb.cmd()
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("created"));

    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.starts_with("# logbook"));
}

#[test]
fn init_is_idempotent() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn add_appends_entry_and_print_flag_echoes_block() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["add", "first decision", "--why", "because"])
        .arg("--print")
        .assert()
        .success()
        .stdout(predicate::str::contains("added:"))
        .stdout(predicate::str::contains("**why:** because"));

    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.contains("first decision"));
    assert!(body.contains("**why:** because"));
}

#[test]
fn add_auto_initializes_when_file_missing() {
    let sb = Sandbox::new();
    sb.cmd()
        .args(["add", "first", "--why", "w"])
        .assert()
        .success()
        .stdout(predicate::str::contains("auto-created"));
    assert!(sb.path().exists());
}

#[test]
fn add_supports_multiple_tag_flags() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args([
            "add", "t", "--why", "w", "--tag", "refactor", "--tag", "perf",
        ])
        .assert()
        .success();

    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.contains("**tags:** refactor, perf"));
}

#[test]
fn list_returns_entries_newest_first() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["add", "first", "--why", "a"])
        .assert()
        .success();
    sb.cmd()
        .args(["add", "second", "--why", "b"])
        .assert()
        .success();
    sb.cmd()
        .args(["add", "third", "--why", "c"])
        .assert()
        .success();

    let stdout = String::from_utf8(
        sb.cmd()
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    let third = stdout.find("third").expect("third entry present");
    let first = stdout.find("first").expect("first entry present");
    assert!(third < first, "newest entry should print before oldest");
}

#[test]
fn list_with_tag_filters_to_matching_entries_only() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["add", "tagged", "--why", "w", "--tag", "db"])
        .assert()
        .success();
    sb.cmd()
        .args(["add", "untagged", "--why", "w"])
        .assert()
        .success();

    sb.cmd()
        .args(["list", "--tag", "db"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tagged"))
        .stdout(predicate::str::contains("untagged").not());
}

#[test]
fn list_with_unknown_tag_emits_no_match_message() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["add", "t", "--why", "w", "--tag", "real"])
        .assert()
        .success();

    sb.cmd()
        .args(["list", "--tag", "missing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no entries match"));
}

#[test]
fn list_bad_since_returns_error_and_nonzero_exit() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["list", "--since", "banana"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--since must be YYYY-MM-DD"));
}

#[test]
fn filters_reject_a_reversed_date_range() {
    let sb = Sandbox::new();
    sb.seed("## 2026-01-01 — choice\n**why:** w\n");
    sb.cmd()
        .args(["list", "--since", "2026-02-01", "--until", "2026-01-01"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--since cannot be after --until"));
}

#[test]
fn search_is_case_insensitive() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["add", "WEBSOCKET MIGRATION", "--why", "w"])
        .assert()
        .success();

    sb.cmd()
        .args(["search", "websocket"])
        .assert()
        .success()
        .stdout(predicate::str::contains("WEBSOCKET MIGRATION"));
}

#[test]
fn search_with_no_hits_says_so() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd().args(["add", "t", "--why", "w"]).assert().success();
    sb.cmd()
        .args(["search", "nothing-matches-this"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no entries match"));
}

#[test]
fn last_returns_most_recent_entry_only() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["add", "first", "--why", "a"])
        .assert()
        .success();
    sb.cmd()
        .args(["add", "newest", "--why", "b"])
        .assert()
        .success();

    sb.cmd()
        .arg("last")
        .assert()
        .success()
        .stdout(predicate::str::contains("newest"))
        .stdout(predicate::str::contains("first").not());
}

#[test]
fn show_rejects_malformed_date() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["show", "banana"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--date must be YYYY-MM-DD"));
}

#[test]
fn show_with_no_matches_returns_friendly_message() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd().args(["add", "t", "--why", "w"]).assert().success();
    sb.cmd()
        .args(["show", "1999-01-01"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no entries on 1999-01-01"));
}

#[test]
fn tags_lists_in_descending_count_order() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["add", "a", "--why", "w", "--tag", "refactor"])
        .assert()
        .success();
    sb.cmd()
        .args(["add", "b", "--why", "w", "--tag", "refactor"])
        .assert()
        .success();
    sb.cmd()
        .args(["add", "c", "--why", "w", "--tag", "perf"])
        .assert()
        .success();

    let out = String::from_utf8(
        sb.cmd()
            .arg("tags")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    let refactor_pos = out.find("refactor").expect("refactor present");
    let perf_pos = out.find("perf").expect("perf present");
    assert!(
        refactor_pos < perf_pos,
        "higher-count tag should print first"
    );
}

#[test]
fn stats_reports_counts_after_adds() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["add", "a", "--why", "w", "--tag", "x"])
        .assert()
        .success();
    sb.cmd()
        .args(["add", "b", "--why", "w", "--tag", "y"])
        .assert()
        .success();

    sb.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total entries: 2"))
        .stdout(predicate::str::contains("unique tags:   2"));
}

#[test]
fn where_prints_resolved_path() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    let expected = sb.path().canonicalize().unwrap();
    sb.cmd()
        .arg("where")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            expected.to_string_lossy().into_owned(),
        ));
}

#[test]
fn missing_file_emits_notfound_error() {
    let sb = Sandbox::new();
    sb.cmd()
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no logbook file at"));
}

#[test]
fn add_then_read_round_trip_preserves_all_fields() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args([
            "add",
            "round-trip",
            "--why",
            "preserve all fields",
            "--rejected",
            "alternatives we said no to",
            "--risk",
            "the thing that might break",
            "--tag",
            "test",
            "--tag",
            "integration",
        ])
        .assert()
        .success();

    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.contains("## "));
    assert!(body.contains("— round-trip"));
    assert!(body.contains("**why:** preserve all fields"));
    assert!(body.contains("**rejected:** alternatives we said no to"));
    assert!(body.contains("**risk:** the thing that might break"));
    assert!(body.contains("**tags:** test, integration"));
}

#[test]
fn export_json_emits_valid_array() {
    let sb = Sandbox::new();
    sb.cmd()
        .args([
            "add",
            "switched ORM",
            "--why",
            "perf",
            "--rejected",
            "redis",
            "--risk",
            "migrations",
            "--tag",
            "db",
        ])
        .assert()
        .success();

    let out = sb
        .cmd()
        .args(["export", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("\"date\": \"20"));
    assert!(stdout.contains("\"title\": \"switched ORM\""));
    assert!(stdout.contains("\"why\": \"perf\""));
    assert!(stdout.contains("\"rejected\": \"redis\""));
    assert!(stdout.contains("\"risk\": \"migrations\""));
    assert!(stdout.contains("\"db\""));
}

#[test]
fn export_defaults_to_json() {
    let sb = Sandbox::new();
    sb.cmd().args(["add", "t", "--why", "w"]).assert().success();
    sb.cmd()
        .arg("export")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\": \"t\""));
}

#[test]
fn list_and_export_limit_to_recent_entries() {
    let sb = Sandbox::new();
    sb.seed(
        "## 2026-01-01 — first\n**why:** a\n\n\
         ## 2026-01-02 — second\n**why:** b\n\n\
         ## 2026-01-03 — third\n**why:** c\n",
    );

    sb.cmd()
        .args(["list", "--limit", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("third"))
        .stdout(predicate::str::contains("second"))
        .stdout(predicate::str::contains("first").not());

    sb.cmd()
        .args(["export", "--limit", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("third"))
        .stdout(predicate::str::contains("second").not());
}

/// Write an executable fake editor that appends `body` to its first argument.
#[cfg(unix)]
fn fake_editor(dir: &std::path::Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("fake-editor.sh");
    std::fs::write(&p, format!("#!/bin/sh\nprintf '{body}' >> \"$1\"\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// Write an editor that releases every caller only after all have opened it.
#[cfg(unix)]
fn barrier_editor(dir: &std::path::Path, participants: usize) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let markers = dir.join("editor-markers");
    std::fs::create_dir(&markers).unwrap();
    let path = dir.join("barrier-editor.sh");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\ntouch \"{}/$$\"\nattempts=0\nwhile [ \"$(find \"{}\" -type f | wc -l)\" -lt {participants} ]; do\n  attempts=$((attempts + 1))\n  if [ \"$attempts\" -ge 500 ]; then echo 'editor barrier timed out' >&2; exit 1; fi\n  sleep 0.01\ndone\nprintf 'coordinated reason\\n' >> \"$1\"\n",
            markers.display(),
            markers.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(unix)]
#[test]
fn add_without_why_opens_editor_and_captures() {
    let sb = Sandbox::new();
    let editor = fake_editor(sb.dir.path(), "editor-supplied reason\\n");
    sb.cmd()
        .env("EDITOR", &editor)
        .args(["add", "no-why-flag"])
        .assert()
        .success();

    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.contains("— no-why-flag"));
    assert!(body.contains("**why:** editor-supplied reason"));
}

#[test]
fn add_without_why_and_empty_editor_aborts() {
    let sb = Sandbox::new();
    // Editor that writes nothing (only the comment template remains) → abort.
    sb.cmd()
        .env("EDITOR", "true")
        .args(["add", "will-abort"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty entry"));

    // Nothing should have been written.
    assert!(
        !sb.path().exists()
            || !std::fs::read_to_string(sb.path())
                .unwrap()
                .contains("will-abort")
    );
}

#[test]
fn add_without_why_no_editor_configured_errors() {
    let sb = Sandbox::new();
    sb.cmd()
        .env_remove("EDITOR")
        .env_remove("VISUAL")
        .args(["add", "no-editor"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no editor configured"));
}

#[test]
fn add_with_why_flag_does_not_open_editor() {
    let sb = Sandbox::new();
    // EDITOR set to something that would FAIL if invoked — proves it isn't.
    sb.cmd()
        .env("EDITOR", "false")
        .args(["add", "has-why", "--why", "explicit reason"])
        .assert()
        .success();
    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.contains("**why:** explicit reason"));
}

#[test]
fn body_fields_reject_structural_lines_without_writing() {
    let sb = Sandbox::new();
    sb.seed("# logbook\n\n");
    let original = std::fs::read_to_string(sb.path()).unwrap();
    let cases = [
        vec!["add", "bad why", "--why", "opening\n## fake entry"],
        vec![
            "add",
            "bad rejected",
            "--why",
            "safe",
            "--rejected",
            "opening\n**risk:** injected",
        ],
        vec![
            "add",
            "bad risk",
            "--why",
            "safe",
            "--risk",
            "opening\n**tags:** injected",
        ],
    ];

    for args in cases {
        sb.cmd()
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("indent that line"));
        assert_eq!(std::fs::read_to_string(sb.path()).unwrap(), original);
    }
}

#[test]
fn piped_structural_why_is_rejected_without_writing() {
    let sb = Sandbox::new();
    sb.cmd()
        .args(["add", "bad pipe", "--why", "-"])
        .write_stdin("opening\n**supersedes:** 2026-01-01 — fake\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("reserved syntax"));
    assert!(!sb.path().exists());
}

#[cfg(unix)]
#[test]
fn editor_structural_why_is_rejected_without_writing() {
    let sb = Sandbox::new();
    let editor = fake_editor(sb.dir.path(), "opening\\n**risk:** injected\\n");
    sb.cmd()
        .env("EDITOR", editor)
        .args(["add", "bad editor"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("reserved syntax"));
    assert!(!sb.path().exists());
}

#[test]
fn body_fields_normalize_crlf_and_allow_indented_markers() {
    let sb = Sandbox::new();
    sb.cmd()
        .args([
            "add",
            "portable body",
            "--why",
            "first\r\n  ## literal heading\r\nlast",
            "--rejected",
            "one\r\n  **risk:** literal text",
        ])
        .assert()
        .success();

    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(!body.contains('\r'));
    let entry = logbook::parse_entries(&body).pop().unwrap();
    assert_eq!(
        entry.why.as_deref(),
        Some("first\n  ## literal heading\nlast")
    );
    assert_eq!(
        entry.rejected.as_deref(),
        Some("one\n  **risk:** literal text")
    );
}

#[cfg(unix)]
#[test]
fn concurrent_identical_adds_commit_only_one_decision() {
    let sb = Sandbox::new();
    sb.seed("# logbook\n\n");
    let editor = barrier_editor(sb.dir.path(), 2);
    let children: Vec<_> = (0..2)
        .map(|_| {
            std::process::Command::new(env!("CARGO_BIN_EXE_logbook"))
                .env("LOGBOOK_FILE", sb.path())
                .env("EDITOR", &editor)
                .args(["add", "one decision", "--date", "2026-01-01"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let outputs: Vec<_> = children
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();

    assert_eq!(
        outputs
            .iter()
            .filter(|output| output.status.success())
            .count(),
        1
    );
    assert!(outputs
        .iter()
        .filter(|output| !output.status.success())
        .all(|output| {
            String::from_utf8_lossy(&output.stderr).contains("a decision already exists")
        }));
    assert_eq!(
        logbook::parse_entries(&std::fs::read_to_string(sb.path()).unwrap()).len(),
        1
    );
}

#[test]
fn supersede_appends_entry_linking_to_old_date() {
    let sb = Sandbox::new();
    sb.cmd()
        .args(["add", "use ORM", "--why", "convenient"])
        .assert()
        .success();
    let old_date = {
        let body = std::fs::read_to_string(sb.path()).unwrap();
        body.lines()
            .find(|l| l.starts_with("## "))
            .and_then(|l| l.strip_prefix("## "))
            .and_then(|s| s.split_whitespace().next())
            .unwrap()
            .to_string()
    };

    sb.cmd()
        .args([
            "supersede",
            &old_date,
            "drop ORM for raw SQL",
            "--why",
            "ORM perf was bad",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("supersedes {old_date}")));

    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.contains("— drop ORM for raw SQL"));
    assert!(body.contains(&format!("**supersedes:** {old_date} — use ORM")));
}

#[test]
fn supersede_requires_old_title_when_a_date_is_ambiguous() {
    let sb = Sandbox::new();
    sb.cmd()
        .args(["add", "first", "--why", "a"])
        .assert()
        .success();
    sb.cmd()
        .args(["add", "second", "--why", "b"])
        .assert()
        .success();
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    sb.cmd()
        .args(["supersede", &date, "replacement", "--why", "c"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--old-title"));

    sb.cmd()
        .args([
            "supersede",
            &date,
            "replacement",
            "--why",
            "c",
            "--old-title",
            "second",
        ])
        .assert()
        .success();
    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.contains(&format!("**supersedes:** {date} — second")));
}

#[test]
fn supersede_missing_target_errors() {
    let sb = Sandbox::new();
    sb.cmd()
        .args(["add", "something", "--why", "w"])
        .assert()
        .success();
    sb.cmd()
        .args(["supersede", "1999-01-01", "new", "--why", "w"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no entry dated 1999-01-01"));
}

#[test]
fn supersede_appears_in_json_export() {
    let sb = Sandbox::new();
    sb.cmd()
        .args(["add", "old", "--why", "w"])
        .assert()
        .success();
    let old_date = {
        let body = std::fs::read_to_string(sb.path()).unwrap();
        body.lines()
            .find(|l| l.starts_with("## "))
            .and_then(|l| l.strip_prefix("## "))
            .and_then(|s| s.split_whitespace().next())
            .unwrap()
            .to_string()
    };
    sb.cmd()
        .args(["supersede", &old_date, "new", "--why", "better"])
        .assert()
        .success();
    sb.cmd()
        .args(["export", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "\"supersedes\": \"{old_date} — old\""
        )));
}

#[test]
fn add_rejects_a_duplicate_dated_title() {
    let sb = Sandbox::new();
    sb.cmd()
        .args([
            "add",
            "same decision",
            "--why",
            "first",
            "--date",
            "2026-01-01",
        ])
        .assert()
        .success();
    sb.cmd()
        .args([
            "add",
            "same decision",
            "--why",
            "second",
            "--date",
            "2026-01-01",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("a decision already exists"));
    assert_eq!(
        logbook::parse_entries(&std::fs::read_to_string(sb.path()).unwrap()).len(),
        1
    );
}

#[test]
fn supersede_rejects_an_already_superseded_decision() {
    let sb = Sandbox::new();
    sb.seed(
        "## 2026-01-01 — original\n**why:** a\n\n\
         ## 2026-02-01 — replacement\n**why:** b\n**supersedes:** 2026-01-01 — original\n",
    );
    sb.cmd()
        .args(["supersede", "2026-01-01", "other branch", "--why", "c"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already superseded"));
    assert_eq!(
        logbook::parse_entries(&std::fs::read_to_string(sb.path()).unwrap()).len(),
        2
    );
}

#[cfg(unix)]
#[test]
fn concurrent_supersedes_commit_only_one_successor() {
    let sb = Sandbox::new();
    sb.seed("## 2026-01-01 — original\n**why:** first\n\n");
    let editor = barrier_editor(sb.dir.path(), 2);
    let children: Vec<_> = (0..2)
        .map(|_| {
            std::process::Command::new(env!("CARGO_BIN_EXE_logbook"))
                .env("LOGBOOK_FILE", sb.path())
                .env("EDITOR", &editor)
                .args([
                    "supersede",
                    "2026-01-01",
                    "replacement",
                    "--date",
                    "2026-02-01",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let outputs: Vec<_> = children
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();

    assert_eq!(
        outputs
            .iter()
            .filter(|output| output.status.success())
            .count(),
        1
    );
    assert!(outputs
        .iter()
        .filter(|output| !output.status.success())
        .all(|output| { String::from_utf8_lossy(&output.stderr).contains("already superseded") }));
    let entries = logbook::parse_entries(&std::fs::read_to_string(sb.path()).unwrap());
    assert_eq!(entries.len(), 2);
    assert!(logbook::validate_entries(&entries).is_empty());
}

#[test]
fn list_combines_date_and_tag_filters() {
    let sb = Sandbox::new();
    sb.seed(
        "## 2026-01-10 — old db\n**why:** a\n**tags:** db\n\n\
         ## 2026-02-15 — current db\n**why:** b\n**tags:** DB\n\n\
         ## 2026-03-20 — current ui\n**why:** c\n**tags:** ui\n",
    );
    sb.cmd()
        .args([
            "list",
            "--tag",
            "db",
            "--since",
            "2026-02-01",
            "--until",
            "2026-02-28",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("current db"))
        .stdout(predicate::str::contains("old db").not())
        .stdout(predicate::str::contains("current ui").not());
}

#[test]
fn add_with_stage_stages_the_logbook() {
    let sb = Sandbox::new();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(sb.dir.path())
        .assert()
        .success();

    sb.cmd()
        .current_dir(sb.dir.path())
        .args(["add", "decision", "--why", "w", "--stage"])
        .assert()
        .success();

    Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(sb.dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("logbook.md"));
}

#[test]
fn stage_failure_reports_that_the_decision_was_saved() {
    let sb = Sandbox::new();
    let path = sb.dir.path().join("decision logs/logbook.md");
    std::fs::create_dir(path.parent().unwrap()).unwrap();
    let mut command = Command::cargo_bin("logbook").unwrap();
    command
        .env("LOGBOOK_FILE", &path)
        .args(["add", "saved once", "--why", "because", "--stage"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("logbook was updated"))
        .stderr(predicate::str::contains(format!(
            "git add -- \"{}\"",
            path.display()
        )));

    let entries = logbook::parse_entries(&std::fs::read_to_string(path).unwrap());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title.as_deref(), Some("saved once"));
}

#[test]
fn piped_output_has_no_ansi_codes() {
    // assert_cmd runs with stdout piped (not a TTY), so Auto must NOT colorize.
    let sb = Sandbox::new();
    sb.cmd().args(["add", "t", "--why", "w"]).assert().success();
    let out = sb.cmd().arg("last").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        !stdout.contains('\x1b'),
        "piped output must contain no ANSI escapes"
    );
    assert!(stdout.contains("## "));
}

#[test]
fn a_closed_stdout_pipe_exits_cleanly() {
    let sb = Sandbox::new();
    sb.seed(&format!(
        "## 2026-01-01 — large output\n**why:** {}\n",
        "x".repeat(2_000_000)
    ));

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_logbook"))
        .env("LOGBOOK_FILE", sb.path())
        .arg("list")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut first_byte = [0];
    stdout.read_exact(&mut first_byte).unwrap();
    drop(stdout);

    assert!(child.wait().unwrap().success());
}

#[cfg(unix)]
#[test]
fn a_closed_stdout_pipe_does_not_skip_staging() {
    let sb = Sandbox::new();
    sb.seed("# logbook\n\n");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(sb.dir.path())
        .assert()
        .success();
    let title = "x".repeat(100_000);
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_logbook"))
        .current_dir(sb.dir.path())
        .env("LOGBOOK_FILE", sb.path())
        .args(["add", &title, "--why", "kept", "--stage"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut first_byte = [0];
    stdout.read_exact(&mut first_byte).unwrap();
    drop(stdout);

    assert!(child.wait().unwrap().success());
    let cached = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(sb.dir.path())
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(cached.stdout).unwrap().trim(),
        "logbook.md"
    );
    assert!(std::fs::read_to_string(sb.path()).unwrap().contains(&title));
}

#[test]
fn color_always_injects_ansi_even_when_piped() {
    let sb = Sandbox::new();
    sb.cmd().args(["add", "t", "--why", "w"]).assert().success();
    let out = sb
        .cmd()
        .args(["--color", "always", "last"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains('\x1b'),
        "--color always must emit ANSI escapes"
    );
}

#[test]
fn color_never_suppresses_ansi() {
    let sb = Sandbox::new();
    sb.cmd().args(["add", "t", "--why", "w"]).assert().success();
    let out = sb
        .cmd()
        .args(["--color", "never", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains('\x1b'));
}

#[test]
fn export_json_never_colorized_even_with_color_always() {
    // Machine output must stay clean regardless of the color flag.
    let sb = Sandbox::new();
    sb.cmd().args(["add", "t", "--why", "w"]).assert().success();
    let out = sb
        .cmd()
        .args(["--color", "always", "export", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        !stdout.contains('\x1b'),
        "json export must never contain ANSI"
    );
}

#[test]
fn discovers_repository_logbook_from_nested_directories() {
    let sb = Sandbox::new();
    let nested = sb.dir.path().join("src/deep");
    std::fs::create_dir_all(sb.dir.path().join(".git")).unwrap();
    std::fs::create_dir_all(&nested).unwrap();
    sb.seed("## 2026-08-01 — repository choice\n**why:** visible from below\n");

    let mut cmd = Command::cargo_bin("logbook").unwrap();
    cmd.current_dir(&nested)
        .env_remove("LOGBOOK_FILE")
        .arg("last")
        .assert()
        .success()
        .stdout(predicate::str::contains("repository choice"));
}

#[test]
fn add_from_nested_directory_creates_one_logbook_at_repository_root() {
    let sb = Sandbox::new();
    let nested = sb.dir.path().join("src/deep");
    std::fs::create_dir_all(sb.dir.path().join(".git")).unwrap();
    std::fs::create_dir_all(&nested).unwrap();

    let mut cmd = Command::cargo_bin("logbook").unwrap();
    cmd.current_dir(&nested)
        .env_remove("LOGBOOK_FILE")
        .args(["add", "root decision", "--why", "one repo, one file"])
        .assert()
        .success();

    assert!(sb.path().exists());
    assert!(!nested.join("logbook.md").exists());
}

#[test]
fn file_flag_overrides_environment_path() {
    let sb = Sandbox::new();
    let environment = sb.dir.path().join("environment.md");
    let explicit = sb.dir.path().join("explicit.md");
    let mut cmd = Command::cargo_bin("logbook").unwrap();
    cmd.env("LOGBOOK_FILE", &environment)
        .arg("--file")
        .arg(&explicit)
        .args(["add", "explicit target", "--why", "because"])
        .assert()
        .success();

    assert!(explicit.exists());
    assert!(!environment.exists());
}

#[cfg(unix)]
#[test]
fn cli_refuses_to_write_through_a_symlink() {
    use std::os::unix::fs::symlink;

    let sb = Sandbox::new();
    let target = sb.dir.path().join("decisions.md");
    std::fs::write(&target, "# original\n").unwrap();
    symlink(&target, sb.path()).unwrap();

    sb.cmd()
        .args(["add", "must not fork", "--why", "because"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "refusing to write through symlink",
        ));

    assert!(std::fs::symlink_metadata(sb.path())
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "# original\n");
}

#[test]
fn init_can_stage_the_new_logbook() {
    let sb = Sandbox::new();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(sb.dir.path())
        .assert()
        .success();

    sb.cmd()
        .current_dir(sb.dir.path())
        .args(["init", "--stage"])
        .assert()
        .success()
        .stdout(predicate::str::contains("staged"));
    Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(sb.dir.path())
        .assert()
        .success()
        .stdout(predicate::eq("logbook.md\n"));
}

#[test]
fn add_accepts_a_real_override_date_and_rejects_impossible_dates() {
    let sb = Sandbox::new();
    sb.cmd()
        .args([
            "add",
            "dated decision",
            "--why",
            "migration",
            "--date",
            "2024-02-29",
        ])
        .assert()
        .success();
    sb.cmd()
        .args(["add", "bad date", "--why", "nope", "--date", "2023-02-29"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("YYYY-MM-DD"));
    assert!(std::fs::read_to_string(sb.path())
        .unwrap()
        .contains("## 2024-02-29 — dated decision"));
}

#[test]
fn piped_multiline_why_survives_markdown_and_json_export() {
    let sb = Sandbox::new();
    sb.cmd()
        .args(["add", "piped reason", "--why", "-"])
        .write_stdin("first paragraph\nsecond line\n\nthird paragraph\n")
        .assert()
        .success();

    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.contains("first paragraph\nsecond line\n\nthird paragraph"));
    sb.cmd()
        .arg("export")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "first paragraph\\nsecond line\\n\\nthird paragraph",
        ));
}

#[test]
fn list_active_and_composable_filters_find_current_decisions() {
    let sb = Sandbox::new();
    sb.seed(
        "## 2026-01-01 — old storage\n**why:** sqlite\n**tags:** storage, local\n\n\
         ## 2026-02-01 — new storage\n**why:** postgres for writes\n**supersedes:** 2026-01-01 — old storage\n**tags:** storage, server\n\n\
         ## 2026-03-01 — unrelated\n**why:** css\n**tags:** ui\n",
    );

    sb.cmd()
        .args([
            "list", "--active", "--tag", "storage", "--tag", "server", "--search", "writes",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("new storage"))
        .stdout(predicate::str::contains("## 2026-01-01 — old storage").not())
        .stdout(predicate::str::contains("unrelated").not());
}

#[test]
fn superseded_filter_selects_only_replaced_decisions() {
    let sb = Sandbox::new();
    sb.seed(
        "## 2026-01-01 — old storage\n**why:** sqlite\n\n\
         ## 2026-02-01 — new storage\n**why:** postgres\n**supersedes:** 2026-01-01 — old storage\n",
    );

    sb.cmd()
        .args(["list", "--superseded"])
        .assert()
        .success()
        .stdout(predicate::str::contains("old storage"))
        .stdout(predicate::str::contains("new storage").not());
    sb.cmd()
        .args(["export", "--superseded"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\": \"old storage\""))
        .stdout(predicate::str::contains("\"active\": false"));
}

#[test]
fn export_filters_recent_matches_and_supports_json_lines() {
    let sb = Sandbox::new();
    sb.seed(
        "## 2026-01-01 — first\n**why:** match\n**tags:** x\n\n\
         ## 2026-01-02 — second\n**why:** match\n**tags:** x\n\n\
         ## 2026-01-03 — third\n**why:** other\n**tags:** x\n",
    );
    let output = sb
        .cmd()
        .args([
            "export", "--format", "jsonl", "--search", "match", "--limit", "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();
    assert_eq!(output.lines().count(), 1);
    assert!(output.contains("second"));
    assert!(!output.contains("first"));
    assert!(output.contains("\"active\":true"));
}

#[test]
fn trace_prints_the_complete_supersession_chain() {
    let sb = Sandbox::new();
    sb.seed(
        "## 2026-01-01 — first\n**why:** a\n\n\
         ## 2026-02-01 — second\n**why:** b\n**supersedes:** 2026-01-01 — first\n\n\
         ## 2026-03-01 — third\n**why:** c\n**supersedes:** 2026-02-01 — second\n",
    );
    let output = sb
        .cmd()
        .args(["trace", "2026-02-01"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();
    let first = output.find("— first").unwrap();
    let second = output.find("— second").unwrap();
    let third = output.find("— third").unwrap();
    assert!(first < second && second < third);
}

#[test]
fn trace_json_returns_the_complete_chain_in_decision_order() {
    let sb = Sandbox::new();
    sb.seed(
        "## 2026-01-01 — first\n**why:** a\n\n\
         ## 2026-02-01 — second\n**why:** b\n**supersedes:** 2026-01-01 — first\n\n\
         ## 2026-03-01 — third\n**why:** c\n**supersedes:** 2026-02-01 — second\n",
    );
    let output = sb
        .cmd()
        .args(["trace", "2026-02-01", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();
    let first = output.find("\"title\": \"first\"").unwrap();
    let second = output.find("\"title\": \"second\"").unwrap();
    let third = output.find("\"title\": \"third\"").unwrap();
    assert!(first < second && second < third);
    assert!(output.contains("\"active\": false"));
    assert!(output.contains("\"active\": true"));
}

#[test]
fn check_reports_all_structural_problems_and_accepts_valid_files() {
    let sb = Sandbox::new();
    sb.seed("## 2026-02-30 — broken\n**why:**\n**supersedes:** 1999-01-01 — absent\n");
    sb.cmd()
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid calendar date"))
        .stderr(predicate::str::contains("missing a non-empty why"))
        .stderr(predicate::str::contains(
            "does not identify an earlier entry",
        ));

    sb.seed("## 2026-02-28 — sound\n**why:** valid\n");
    sb.cmd()
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("1 entries (1 active"));
}

#[test]
fn check_json_reports_machine_readable_success_and_failure() {
    let sb = Sandbox::new();
    sb.seed("## 2026-02-30 — broken\n**why:**\n");
    sb.cmd()
        .args(["check", "--format", "json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"valid\": false"))
        .stdout(predicate::str::contains("\"entry\": 1"))
        .stdout(predicate::str::contains("invalid calendar date"))
        .stderr(predicate::str::contains("invalid calendar date").not());

    sb.seed("## 2026-02-28 — sound\n**why:** valid\n");
    sb.cmd()
        .args(["check", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"valid\": true"))
        .stdout(predicate::str::contains("\"entries\": 1"))
        .stdout(predicate::str::contains("\"issues\": []"));
}

#[test]
fn completions_generates_scripts_without_a_logbook() {
    let sb = Sandbox::new();
    sb.cmd()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_logbook()"))
        .stdout(predicate::str::contains("completions"));
    sb.cmd()
        .args(["completions", "powershell"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Register-ArgumentCompleter"));
}

#[test]
fn show_title_disambiguates_same_day_entries() {
    let sb = Sandbox::new();
    sb.seed(
        "## 2026-01-01 — first\n**why:** a\n\n\
         ## 2026-01-01 — second\n**why:** b\n",
    );
    sb.cmd()
        .args(["show", "2026-01-01", "--title", "second"])
        .assert()
        .success()
        .stdout(predicate::str::contains("— second"))
        .stdout(predicate::str::contains("— first").not());
}

#[test]
fn supersede_prints_the_written_block_and_stats_report_decision_state() {
    let sb = Sandbox::new();
    sb.seed("## 2026-01-01 — old\n**why:** a\n");
    sb.cmd()
        .args([
            "supersede",
            "2026-01-01",
            "new",
            "--why",
            "b",
            "--date",
            "2026-02-01",
            "--print",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("**supersedes:** 2026-01-01 — old"));
    sb.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("active:        1"))
        .stdout(predicate::str::contains("superseded:    1"));
}
