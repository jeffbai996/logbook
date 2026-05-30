//! End-to-end CLI tests. Each test spawns the actual `logbook` binary in
//! an isolated tempdir and exercises one piece of behaviour. The LOGBOOK_FILE
//! env var is used to avoid the binary touching cwd, which keeps tests
//! parallel-safe.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;
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
        // Be a good citizen: don't inherit the user's actual env that
        // might point LOGBOOK_FILE somewhere else.
        c
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
    // No init — go straight to add.
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
    // Never initialize. list should fail with NotFound.
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
    // No --format → JSON by default.
    sb.cmd()
        .arg("export")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\": \"t\""));
}

#[test]
fn export_empty_logbook_is_empty_array() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .args(["export", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[]"));
}

#[test]
fn export_unknown_format_errors() {
    let sb = Sandbox::new();
    sb.cmd().args(["add", "t", "--why", "w"]).assert().success();
    sb.cmd()
        .args(["export", "--format", "yaml"])
        .assert()
        .failure();
}

/// Write an executable fake-editor script that appends `body` to the file it's
/// given as $1. spawn_editor splits the editor command on whitespace, so a bare
/// script path (no args/quotes) is the portable way to fake an editor.
#[cfg(unix)]
fn fake_editor(dir: &std::path::Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("fake-editor.sh");
    std::fs::write(&p, format!("#!/bin/sh\nprintf '{body}' >> \"$1\"\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
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
fn supersede_appends_entry_linking_to_old_date() {
    let sb = Sandbox::new();
    // Seed an original decision.
    sb.cmd()
        .args(["add", "use ORM", "--why", "convenient"])
        .assert()
        .success();
    let old_date = {
        // Grab the date the original got (today, but read it from the file to be exact).
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
    assert!(body.contains(&format!("**supersedes:** {old_date}")));
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
fn supersede_malformed_date_errors() {
    let sb = Sandbox::new();
    sb.cmd().args(["add", "x", "--why", "w"]).assert().success();
    sb.cmd()
        .args(["supersede", "not-a-date", "new", "--why", "w"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("YYYY-MM-DD"));
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
            "\"supersedes\": \"{old_date}\""
        )));
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
