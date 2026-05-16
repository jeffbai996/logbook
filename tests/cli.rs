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
        Sandbox { dir: TempDir::new().unwrap() }
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
        .args(["add", "t", "--why", "w", "--tag", "refactor", "--tag", "perf"])
        .assert()
        .success();

    let body = std::fs::read_to_string(sb.path()).unwrap();
    assert!(body.contains("**tags:** refactor, perf"));
}

#[test]
fn list_returns_entries_newest_first() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd().args(["add", "first", "--why", "a"]).assert().success();
    sb.cmd().args(["add", "second", "--why", "b"]).assert().success();
    sb.cmd().args(["add", "third", "--why", "c"]).assert().success();

    let stdout = String::from_utf8(sb.cmd().arg("list").assert().success().get_output().stdout.clone()).unwrap();
    let third = stdout.find("third").expect("third entry present");
    let first = stdout.find("first").expect("first entry present");
    assert!(third < first, "newest entry should print before oldest");
}

#[test]
fn list_with_tag_filters_to_matching_entries_only() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd().args(["add", "tagged", "--why", "w", "--tag", "db"]).assert().success();
    sb.cmd().args(["add", "untagged", "--why", "w"]).assert().success();

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
    sb.cmd().args(["add", "t", "--why", "w", "--tag", "real"]).assert().success();

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
    sb.cmd().args(["add", "WEBSOCKET MIGRATION", "--why", "w"]).assert().success();

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
    sb.cmd().args(["add", "first", "--why", "a"]).assert().success();
    sb.cmd().args(["add", "newest", "--why", "b"]).assert().success();

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
    sb.cmd().args(["add", "a", "--why", "w", "--tag", "refactor"]).assert().success();
    sb.cmd().args(["add", "b", "--why", "w", "--tag", "refactor"]).assert().success();
    sb.cmd().args(["add", "c", "--why", "w", "--tag", "perf"]).assert().success();

    let out = String::from_utf8(sb.cmd().arg("tags").assert().success().get_output().stdout.clone()).unwrap();
    let refactor_pos = out.find("refactor").expect("refactor present");
    let perf_pos = out.find("perf").expect("perf present");
    assert!(refactor_pos < perf_pos, "higher-count tag should print first");
}

#[test]
fn stats_reports_counts_after_adds() {
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd().args(["add", "a", "--why", "w", "--tag", "x"]).assert().success();
    sb.cmd().args(["add", "b", "--why", "w", "--tag", "y"]).assert().success();

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
        .stdout(predicate::str::contains(expected.to_string_lossy().into_owned()));
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
            "--why", "preserve all fields",
            "--rejected", "alternatives we said no to",
            "--risk", "the thing that might break",
            "--tag", "test",
            "--tag", "integration",
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
