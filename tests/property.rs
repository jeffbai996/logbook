use logbook::{parse_entries, render_entry_block, RenderInput};
use proptest::prelude::*;

fn safe_text() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ,.;:!?()/_'\"-]{1,80}"
        .prop_map(|s| s.trim().to_string())
        .prop_filter("non-empty after trimming", |s| !s.is_empty())
}

fn safe_tag() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,15}"
}

proptest! {
    #[test]
    fn rendered_entry_round_trips(
        title in safe_text(),
        why in safe_text(),
        rejected in proptest::option::of(safe_text()),
        risk in proptest::option::of(safe_text()),
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
        prop_assert_eq!(parsed.len(), 1);
        let entry = &parsed[0];
        prop_assert_eq!(entry.date.as_deref(), Some("2026-05-16"));
        prop_assert_eq!(entry.title.as_deref(), Some(title.as_str()));
        prop_assert_eq!(entry.why.as_deref(), Some(why.as_str()));
        prop_assert_eq!(entry.rejected.as_deref(), rejected.as_deref());
        prop_assert_eq!(entry.risk.as_deref(), risk.as_deref());
        prop_assert_eq!(&entry.tags, &tags);
        prop_assert!(entry.superseded_by.is_empty());
        prop_assert_eq!(&entry.raw, block.trim_end());
    }
}
