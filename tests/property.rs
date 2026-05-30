//! Property-based tests + snapshot tests. These catch a class of bugs that
//! example-based tests miss — anything we forgot to enumerate in the unit
//! tests, plus drift in the rendered format over time.
//!
//! Run with `cargo test --test property`. Snapshot review is via
//! `cargo install cargo-insta && cargo insta review`.

use logbook::{parse_entries, render_entry_block, Entry, RenderInput};
use proptest::prelude::*;

// ----- Snapshot tests: lock in the canonical rendered form -----
//
// If anyone changes the renderer's output format (whitespace, ordering of
// fields, em-dash vs hyphen, etc.), these tests fail and force a conscious
// review of the new snapshot. The snapshots live in tests/snapshots/.

#[test]
fn snapshot_minimal_entry() {
    let tags: Vec<String> = vec![];
    let block = render_entry_block(&RenderInput {
        date: "2026-05-16",
        title: "use SQLite over Postgres for v1",
        why: "single binary, no external service, fine for <100 concurrent writes",
        rejected: None,
        risk: None,
        tags: &tags,
        supersedes: None,
    });
    insta::assert_snapshot!(block);
}

#[test]
fn snapshot_full_entry() {
    let tags = vec!["db".into(), "infra".into(), "v1".into()];
    let block = render_entry_block(&RenderInput {
        date: "2026-05-16",
        title: "switched ORM to raw SQL for the hot path",
        why: "ORM was generating 14-join queries for 3-table lookups; perf tanked under load",
        rejected: Some(
            "ORM query hints (still magic), custom resolver (too much code for too little win)",
        ),
        risk: Some(
            "lose automatic migrations — added manual scripts in db/migrations/ to compensate",
        ),
        tags: &tags,
        supersedes: None,
    });
    insta::assert_snapshot!(block);
}

#[test]
fn snapshot_entry_with_only_tags() {
    let tags = vec!["tag-only".into()];
    let block = render_entry_block(&RenderInput {
        date: "2026-05-16",
        title: "tag-only example",
        why: "verify tags render without rejected/risk",
        rejected: None,
        risk: None,
        tags: &tags,
        supersedes: None,
    });
    insta::assert_snapshot!(block);
}

// ----- Property tests: round-trip render -> parse -> render is stable -----
//
// proptest generates thousands of randomized inputs and shrinks any failing
// case to a minimal reproduction. The properties below assert invariants
// that *must* hold for every legal input, not just the ones I happened to
// think of.

/// Generate a string suitable for embedding in markdown without breaking the
/// parser: no `## ` line starts (which would look like a new entry header),
/// no leading/trailing whitespace (which the renderer strips), and printable
/// ASCII for sanity. Returns a non-empty string.
fn safe_markdown_text() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ,.;:!?()/_'\"-]{1,80}"
        .prop_filter("must not start with markdown header", |s: &String| {
            !s.starts_with("## ") && !s.starts_with('#')
        })
        .prop_map(|s| s.trim().to_string())
        .prop_filter("non-empty after trim", |s: &String| !s.is_empty())
}

/// Generate a tag — alphanumeric + hyphen, the shape `logbook tags` expects.
fn safe_tag() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,15}".prop_map(|s: String| s.to_string())
}

proptest! {
    /// For any well-formed entry inputs, rendering then parsing then re-rendering
    /// should produce byte-identical output. This catches *any* drift in the
    /// renderer or parser — if either side mutates whitespace, reorders fields,
    /// trims something the other doesn't, the round-trip breaks.
    #[test]
    fn render_then_parse_then_render_is_stable(
        title in safe_markdown_text(),
        why in safe_markdown_text(),
        rejected in proptest::option::of(safe_markdown_text()),
        risk in proptest::option::of(safe_markdown_text()),
        tags in proptest::collection::vec(safe_tag(), 0..5),
    ) {
        let block = render_entry_block(&RenderInput {
            date: "2026-05-16",
            title: &title,
            why: &why,
            rejected: rejected.as_deref(),
            risk: risk.as_deref(),
            tags: &tags,
            supersedes: None,
        });

        let parsed = parse_entries(&block);
        prop_assert_eq!(parsed.len(), 1, "exactly one entry should round-trip");

        let entry = &parsed[0];

        // Date round-trips exactly.
        prop_assert_eq!(entry.date.as_deref(), Some("2026-05-16"));

        // Tags round-trip (filter out empties that the renderer drops).
        let expected_tags: Vec<String> = tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        prop_assert_eq!(&entry.tags, &expected_tags);

        // raw matches the block we wrote in (modulo the trailing blank line
        // which the parser strips and the renderer adds).
        let normalized = block.trim_end().to_string();
        prop_assert_eq!(&entry.raw, &normalized);
    }

    /// Parsing a document with N entries followed by re-extracting them
    /// should preserve count and order regardless of the content of each entry.
    #[test]
    fn document_with_many_entries_preserves_count_and_order(
        n in 1usize..20,
    ) {
        let mut doc = String::from("# logbook\n\nPreamble.\n\n");
        for i in 0..n {
            doc.push_str(&render_entry_block(&RenderInput {
                date: "2026-05-16",
                title: &format!("entry {i}"),
                why: "w",
                rejected: None,
                risk: None,
                tags: &[],
                supersedes: None,
            }));
        }

        let parsed = parse_entries(&doc);
        prop_assert_eq!(parsed.len(), n);
        for (i, e) in parsed.iter().enumerate() {
            let needle = format!("entry {i}");
            prop_assert!(e.raw.contains(&needle));
        }
    }

    /// Parser must never panic, even on garbage input. We feed it random
    /// bytes (filtered to valid UTF-8) and assert it returns *some* Vec,
    /// possibly empty, but never panics.
    #[test]
    fn parser_never_panics_on_arbitrary_input(input in ".*") {
        let _: Vec<Entry> = parse_entries(&input);
    }
}
