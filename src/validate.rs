//! Structural checks for parsed decision logs.

use crate::parse::matching_prior_entries;
use crate::{is_valid_date, Entry};
use std::collections::BTreeMap;

/// One problem found by [`validate_entries`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    /// One-based entry number in document order.
    pub entry: usize,
    /// Human-readable description of the problem.
    pub message: String,
}

/// Validate required fields, calendar dates, and supersession links.
pub fn validate_entries(entries: &[Entry]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut references: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut successors: BTreeMap<usize, Vec<usize>> = BTreeMap::new();

    for (index, entry) in entries.iter().enumerate() {
        match entry.date.as_deref() {
            Some(date) if is_valid_date(date) => {}
            Some(date) => issue(&mut issues, index, format!("invalid calendar date {date}")),
            None => issue(&mut issues, index, "heading is missing a YYYY-MM-DD date"),
        }
        if entry.title.as_deref().is_none_or(str::is_empty) {
            issue(&mut issues, index, "heading is missing a title");
        }
        if entry.why.as_deref().is_none_or(str::is_empty) {
            issue(&mut issues, index, "entry is missing a non-empty why field");
        }
        if let Some(reference) = entry.reference() {
            references.entry(reference).or_default().push(index);
        }
        if let Some(reference) = entry.supersedes.as_deref() {
            match matching_prior_entries(entries, index, reference).as_slice() {
                [target] => successors.entry(*target).or_default().push(index),
                [] => issue(
                    &mut issues,
                    index,
                    format!("supersedes target {reference:?} does not identify an earlier entry"),
                ),
                _ => issue(
                    &mut issues,
                    index,
                    format!("supersedes target {reference:?} is ambiguous"),
                ),
            }
        }
    }

    for (reference, indexes) in references {
        if indexes.len() > 1 {
            for index in indexes {
                issue(
                    &mut issues,
                    index,
                    format!("duplicate decision reference {reference:?}"),
                );
            }
        }
    }
    for (target, children) in successors {
        if children.len() > 1 {
            issue(
                &mut issues,
                target,
                format!(
                    "decision is superseded by {} different entries",
                    children.len()
                ),
            );
        }
    }

    issues.sort_by_key(|issue| issue.entry);
    issues
}

fn issue(issues: &mut Vec<ValidationIssue>, zero_based: usize, message: impl Into<String>) {
    issues.push(ValidationIssue {
        entry: zero_based + 1,
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_entries;

    #[test]
    fn accepts_a_valid_supersession_chain() {
        let entries = parse_entries(
            "## 2026-01-01 — first\n**why:** a\n\n\
             ## 2026-01-02 — second\n**why:** b\n**supersedes:** 2026-01-01 — first\n",
        );
        assert!(validate_entries(&entries).is_empty());
    }

    #[test]
    fn reports_required_fields_dates_and_broken_links_together() {
        let entries = parse_entries(
            "## 2026-02-30 — broken\n**why:**\n**supersedes:** 1999-01-01 — absent\n",
        );
        let issues = validate_entries(&entries);
        let messages: Vec<&str> = issues.iter().map(|issue| issue.message.as_str()).collect();
        assert!(messages
            .iter()
            .any(|message| message.contains("invalid calendar")));
        assert!(messages
            .iter()
            .any(|message| message.contains("missing a non-empty why")));
        assert!(messages
            .iter()
            .any(|message| message.contains("does not identify")));
    }

    #[test]
    fn rejects_duplicate_references() {
        let entries = parse_entries(
            "## 2026-01-01 — same\n**why:** a\n\n\
             ## 2026-01-01 — same\n**why:** b\n",
        );
        let issues = validate_entries(&entries);
        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("duplicate decision reference")));
    }

    #[test]
    fn rejects_branched_supersession() {
        let entries = parse_entries(
            "## 2026-01-01 — original\n**why:** a\n\n\
             ## 2026-01-02 — branch one\n**why:** b\n**supersedes:** 2026-01-01 — original\n\n\
             ## 2026-01-03 — branch two\n**why:** c\n**supersedes:** 2026-01-01 — original\n",
        );
        let issues = validate_entries(&entries);
        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("superseded by 2 different entries")));
    }
}
