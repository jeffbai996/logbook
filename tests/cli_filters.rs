//! End-to-end CLI tests focused on the date-range and tag-aggregation
//! logic in `main.rs` that the baseline `cli.rs` suite doesn't exercise
//! directly: combined `--since`/`--until` windows, `--until` alone,
//! date filters skipping undated entries, `show` selecting only the
//! matching date among many, tag case-folding/merging, and the `--stage`
//! git shell-out.
//!
//! These seed a logbook file with hand-written, fixed dates (rather than
//! shelling out `add`, which always stamps *today*), so the date math is
//! deterministic and independent of the wall clock.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;
use tempfile::TempDir;

/// Tempdir + an absolute `logbook.md` path inside it, wired through the
/// `LOGBOOK_FILE` env var so the binary never touches the real cwd and
/// tests stay parallel-safe.
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

    fn cmd(&self) -> Command {
        let mut c = Command::cargo_bin("logbook").unwrap();
        c.env("LOGBOOK_FILE", self.path());
        c
    }

    /// Write a logbook body verbatim, bypassing `add` so dates are fixed.
    fn seed(&self, body: &str) {
        std::fs::write(self.path(), body).unwrap();
    }
}

/// Three dated entries spanning two months plus one deliberately undated
/// entry, so date-range filters have something to include AND exclude.
const FIXTURE: &str = "\
# logbook

Preamble that should be ignored.

## 2026-01-10 — january entry
**why:** first
**tags:** db, perf

## 2026-02-15 — february entry
**why:** second
**tags:** DB, refactor

## 2026-03-20 — march entry
**why:** third
**tags:** perf

## not-a-date — undated entry
**why:** has no shape-valid date
**tags:** orphan
";

#[test]
fn list_with_since_and_until_keeps_only_the_window() {
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .args(["list", "--since", "2026-02-01", "--until", "2026-02-28"])
        .assert()
        .success()
        .stdout(predicate::str::contains("february entry"))
        .stdout(predicate::str::contains("january entry").not())
        .stdout(predicate::str::contains("march entry").not());
}

#[test]
fn list_with_until_alone_excludes_later_entries() {
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .args(["list", "--until", "2026-02-15"])
        .assert()
        .success()
        .stdout(predicate::str::contains("january entry"))
        .stdout(predicate::str::contains("february entry"))
        .stdout(predicate::str::contains("march entry").not());
}

#[test]
fn list_with_since_drops_undated_entries() {
    // An entry whose heading isn't YYYY-MM-DD has date == None; once a
    // `--since` filter is active it must be excluded, never treated as
    // "matches everything".
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .args(["list", "--since", "2026-01-01"])
        .assert()
        .success()
        .stdout(predicate::str::contains("january entry"))
        .stdout(predicate::str::contains("undated entry").not());
}

#[test]
fn list_with_inverted_window_matches_nothing() {
    // since later than until: no date can satisfy both bounds.
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .args(["list", "--since", "2026-03-01", "--until", "2026-01-01"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no entries match the given filters"));
}

#[test]
fn list_with_bad_until_errors_with_nonzero_exit() {
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .args(["list", "--until", "2026-3-1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--until must be YYYY-MM-DD"));
}

#[test]
fn list_tag_filter_is_case_insensitive_across_entries() {
    // Entry 1 has "db", entry 2 has "DB" — a lowercase filter must catch both.
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .args(["list", "--tag", "db"])
        .assert()
        .success()
        .stdout(predicate::str::contains("january entry"))
        .stdout(predicate::str::contains("february entry"))
        .stdout(predicate::str::contains("march entry").not());
}

#[test]
fn show_returns_only_the_entry_for_that_date() {
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .args(["show", "2026-02-15"])
        .assert()
        .success()
        .stdout(predicate::str::contains("february entry"))
        .stdout(predicate::str::contains("january entry").not())
        .stdout(predicate::str::contains("march entry").not());
}

#[test]
fn show_for_a_date_with_no_entry_is_friendly() {
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .args(["show", "2026-12-31"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no entries on 2026-12-31"));
}

#[test]
fn tags_merges_case_variants_and_orders_by_count() {
    // "db"/"DB" fold to one tag with count 2; "perf" also appears twice;
    // equal counts break alphabetically (ascending), so "db" precedes
    // "perf", and both rank above the count-1 tags.
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .arg("tags")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"(?s)db\s+2.*perf\s+2").unwrap())
        .stdout(predicate::str::contains("refactor"))
        .stdout(predicate::str::contains("orphan"));
}

#[test]
fn stats_reports_full_range_and_unique_tag_count() {
    // Range spans the earliest dated entry to the latest; undated entry
    // contributes its tag to the unique-tag set but not to the range.
    // Distinct tags after lowercasing: db, perf, refactor, orphan = 4.
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total entries: 4"))
        .stdout(predicate::str::contains("2026-01-10 → 2026-03-20"))
        .stdout(predicate::str::contains("unique tags:   4"));
}

#[test]
fn search_matches_body_text_not_just_headings() {
    let sb = Sandbox::new();
    sb.seed(FIXTURE);
    sb.cmd()
        .args(["search", "shape-valid"])
        .assert()
        .success()
        .stdout(predicate::str::contains("undated entry"))
        .stdout(predicate::str::contains("january entry").not());
}

#[test]
fn add_with_stage_runs_git_add_in_a_repo() {
    // --stage shells out to `git add`. Run inside a real git repo so the
    // command succeeds; assert the file got staged.
    let sb = Sandbox::new();
    let repo = sb.dir.path();

    Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo)
        .assert()
        .success();

    sb.cmd()
        .current_dir(repo)
        .args(["add", "decision", "--why", "w", "--stage"])
        .assert()
        .success()
        .stdout(predicate::str::contains("staged"));

    // The logbook file should now be in git's index.
    let staged = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(repo)
        .output()
        .unwrap();
    let names = String::from_utf8_lossy(&staged.stdout);
    assert!(
        names.contains("logbook.md"),
        "expected logbook.md to be staged, git index had: {names}"
    );
}

#[test]
fn last_on_empty_initialized_file_reports_no_entries() {
    // init writes only the header (no `## ` entries), so `last` must hit
    // the empty branch rather than echoing the preamble.
    let sb = Sandbox::new();
    sb.cmd().arg("init").assert().success();
    sb.cmd()
        .arg("last")
        .assert()
        .success()
        .stdout(predicate::str::contains("(no entries yet)"));
}
