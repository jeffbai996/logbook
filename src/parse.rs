//! Parser for the logbook markdown format. Entries are blocks starting with
//! `## ` headers; lines before the first header (e.g. the file header
//! written by `init`) are ignored.

/// A single parsed entry. `raw` is the markdown block as written, with
/// trailing whitespace trimmed; `date` and `tags` are extracted for
/// filtering and statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub raw: String,
    pub date: Option<String>,
    pub tags: Vec<String>,
}

/// Parse the full text of a logbook file into entries, in document order.
/// Entries are detected by `## ` headers at the start of a line; any text
/// before the first header is treated as the file preamble and discarded.
pub fn parse_entries(text: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut current: Vec<&str> = Vec::new();

    for line in text.lines() {
        if line.starts_with("## ") {
            if !current.is_empty() {
                entries.push(make_entry(&current));
                current.clear();
            }
            current.push(line);
        } else if !current.is_empty() {
            current.push(line);
        }
    }
    if !current.is_empty() {
        entries.push(make_entry(&current));
    }
    entries
}

fn make_entry(lines: &[&str]) -> Entry {
    let mut raw = lines.join("\n");
    while raw.ends_with('\n') || raw.ends_with(' ') {
        raw.pop();
    }

    let header = lines.first().copied().unwrap_or("");
    let date = header
        .strip_prefix("## ")
        .and_then(|s| s.split_whitespace().next())
        .filter(|d| crate::is_date_shaped(d))
        .map(|d| d.to_string());

    let mut tags: Vec<String> = Vec::new();
    for line in lines {
        if let Some(rest) = line.strip_prefix("**tags:**") {
            tags = rest
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            break;
        }
    }

    Entry { raw, date, tags }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_no_entries() {
        assert_eq!(parse_entries(""), vec![]);
    }

    #[test]
    fn preamble_without_entries_is_ignored() {
        let text = "# logbook\n\nSome preamble text\nthat predates any entry.\n";
        assert_eq!(parse_entries(text), vec![]);
    }

    #[test]
    fn single_entry_parses_date_and_no_tags() {
        let text = "## 2026-05-16 — first decision\n**why:** because\n";
        let entries = parse_entries(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date.as_deref(), Some("2026-05-16"));
        assert!(entries[0].tags.is_empty());
        assert!(entries[0].raw.starts_with("## 2026-05-16"));
    }

    #[test]
    fn entry_with_tags_extracts_them() {
        let text = "## 2026-05-16 — t\n**why:** w\n**tags:** refactor, perf, db\n";
        let entries = parse_entries(text);
        assert_eq!(entries[0].tags, vec!["refactor", "perf", "db"]);
    }

    #[test]
    fn tag_whitespace_and_empties_are_trimmed() {
        let text = "## 2026-05-16 — t\n**why:** w\n**tags:**  refactor ,, perf  ,\n";
        let entries = parse_entries(text);
        assert_eq!(entries[0].tags, vec!["refactor", "perf"]);
    }

    #[test]
    fn multiple_entries_preserve_document_order() {
        let text = "\
## 2026-05-14 — first
**why:** a

## 2026-05-15 — second
**why:** b

## 2026-05-16 — third
**why:** c
";
        let entries = parse_entries(text);
        let titles: Vec<&str> = entries
            .iter()
            .map(|e| e.raw.lines().next().unwrap_or(""))
            .collect();
        assert_eq!(
            titles,
            vec![
                "## 2026-05-14 — first",
                "## 2026-05-15 — second",
                "## 2026-05-16 — third",
            ]
        );
    }

    #[test]
    fn malformed_date_in_header_is_left_as_none() {
        let text = "## not-a-date — wat\n**why:** w\n";
        let entries = parse_entries(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date, None);
    }

    #[test]
    fn preamble_followed_by_entry_drops_preamble_only() {
        let text = "# logbook\n\nSome preamble.\n\n## 2026-05-16 — real\n**why:** w\n";
        let entries = parse_entries(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date.as_deref(), Some("2026-05-16"));
    }

    #[test]
    fn entry_raw_has_no_trailing_whitespace() {
        let text = "## 2026-05-16 — t\n**why:** w\n\n\n";
        let entries = parse_entries(text);
        assert!(!entries[0].raw.ends_with('\n'));
        assert!(!entries[0].raw.ends_with(' '));
    }

    #[test]
    fn second_tags_line_is_ignored() {
        // First tags: line wins; later text shouldn't override (append-only spec).
        let text = "\
## 2026-05-16 — t
**why:** w
**tags:** real, tags
**tags:** these, should, be, ignored
";
        let entries = parse_entries(text);
        assert_eq!(entries[0].tags, vec!["real", "tags"]);
    }
}
