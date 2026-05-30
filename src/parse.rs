//! Parser for the logbook markdown format.
//!
//! Entries are blocks starting with `## ` headers; lines before the first
//! header (e.g. the file header written by `init`) are treated as preamble
//! and discarded. Within an entry, only two pieces of structure are
//! extracted: the date in the heading (if it's shaped like `YYYY-MM-DD`)
//! and the comma-separated values of the first `**tags:**` line. The full
//! block is preserved verbatim in [`Entry::raw`] so subcommands like
//! `list` and `search` can echo the original markdown back to stdout
//! without re-rendering.

/// A single parsed entry from a logbook file.
///
/// `raw` is the markdown block as written, trailing whitespace trimmed,
/// for re-emission. `date` and `tags` are extracted for filtering and
/// statistics; they may be empty/None on malformed input.
///
/// # Example
///
/// ```
/// use logbook::parse_entries;
///
/// let entries = parse_entries("## 2026-05-16 — t\n**why:** w\n**tags:** a, b\n");
/// assert_eq!(entries[0].date.as_deref(), Some("2026-05-16"));
/// assert_eq!(entries[0].tags, vec!["a", "b"]);
/// assert!(entries[0].raw.contains("**why:** w"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The full markdown block as written, trailing whitespace trimmed.
    pub raw: String,
    /// The `YYYY-MM-DD` date from the heading, or `None` if the heading
    /// didn't begin with a shape-valid date.
    pub date: Option<String>,
    /// The title from the heading — everything after `## <date> — `, or
    /// the whole heading text if it didn't follow the `date — title` shape.
    /// `None` only when the heading is empty.
    pub title: Option<String>,
    /// The `**why:**` field value, or `None` if absent.
    pub why: Option<String>,
    /// The `**rejected:**` field value, or `None` if absent.
    pub rejected: Option<String>,
    /// The `**risk:**` field value, or `None` if absent.
    pub risk: Option<String>,
    /// The `**supersedes:**` field value (a `YYYY-MM-DD` date this entry
    /// replaces), or `None` if absent.
    pub supersedes: Option<String>,
    /// Tags from the first `**tags:**` line within the entry, with
    /// per-tag whitespace trimmed and empty entries dropped. May be empty.
    pub tags: Vec<String>,
}

/// Parse the full text of a logbook file into entries, in document order.
///
/// Entries are detected by `## ` headers at the start of a line. Any text
/// before the first header is treated as the file preamble and discarded.
/// Never panics, never errors — invalid markdown simply produces fewer
/// or no entries.
///
/// # Example
///
/// ```
/// use logbook::parse_entries;
///
/// let text = "# logbook\n\nPreamble.\n\n## 2026-05-15 — a\n**why:** first\n\n## 2026-05-16 — b\n**why:** second\n";
/// let entries = parse_entries(text);
/// assert_eq!(entries.len(), 2);
/// assert_eq!(entries[0].date.as_deref(), Some("2026-05-15"));
/// assert_eq!(entries[1].date.as_deref(), Some("2026-05-16"));
/// ```
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
    let heading_body = header.strip_prefix("## ").unwrap_or(header);
    let date = heading_body
        .split_whitespace()
        .next()
        .filter(|d| crate::is_date_shaped(d))
        .map(|d| d.to_string());

    // Title = heading minus the leading `<date> — `. If the heading didn't
    // follow that shape, fall back to the whole heading body (so a malformed
    // header still yields a usable title rather than None).
    let title = if date.is_some() {
        heading_body
            .split_once(" — ")
            .map(|(_, t)| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .or_else(|| Some(heading_body.trim().to_string()))
    } else {
        let t = heading_body.trim();
        (!t.is_empty()).then(|| t.to_string())
    };

    let why = first_field(lines, "**why:**");
    let rejected = first_field(lines, "**rejected:**");
    let risk = first_field(lines, "**risk:**");
    let supersedes = first_field(lines, "**supersedes:**");

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

    Entry {
        raw,
        date,
        title,
        why,
        rejected,
        risk,
        supersedes,
        tags,
    }
}

/// Return the trimmed value of the first line matching `prefix`
/// (e.g. `"**why:**"`), or `None` if no such line or the value is empty.
fn first_field(lines: &[&str], prefix: &str) -> Option<String> {
    for line in lines {
        if let Some(rest) = line.strip_prefix(prefix) {
            let v = rest.trim();
            return (!v.is_empty()).then(|| v.to_string());
        }
    }
    None
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
    fn extracts_title_why_rejected_risk() {
        let text = "## 2026-05-16 — switched ORM\n**why:** perf\n**rejected:** redis\n**risk:** migrations\n**tags:** db\n";
        let e = &parse_entries(text)[0];
        assert_eq!(e.title.as_deref(), Some("switched ORM"));
        assert_eq!(e.why.as_deref(), Some("perf"));
        assert_eq!(e.rejected.as_deref(), Some("redis"));
        assert_eq!(e.risk.as_deref(), Some("migrations"));
    }

    #[test]
    fn absent_optional_fields_are_none() {
        let text = "## 2026-05-16 — t\n**why:** w\n";
        let e = &parse_entries(text)[0];
        assert_eq!(e.why.as_deref(), Some("w"));
        assert_eq!(e.rejected, None);
        assert_eq!(e.risk, None);
        assert_eq!(e.supersedes, None);
    }

    #[test]
    fn extracts_supersedes_date() {
        let text = "## 2026-05-16 — t\n**why:** w\n**supersedes:** 2026-05-01\n";
        let e = &parse_entries(text)[0];
        assert_eq!(e.supersedes.as_deref(), Some("2026-05-01"));
    }

    #[test]
    fn title_with_em_dash_in_body_keeps_full_title() {
        // The split is on the FIRST " — " (date separator); a title that itself
        // contains " — " should keep everything after the first separator.
        let text = "## 2026-05-16 — switched ORM — finally\n**why:** w\n";
        let e = &parse_entries(text)[0];
        assert_eq!(e.title.as_deref(), Some("switched ORM — finally"));
    }

    #[test]
    fn malformed_header_title_falls_back_to_heading_text() {
        let text = "## not-a-date wat\n**why:** w\n";
        let e = &parse_entries(text)[0];
        assert_eq!(e.date, None);
        assert_eq!(e.title.as_deref(), Some("not-a-date wat"));
    }

    #[test]
    fn empty_why_value_is_none() {
        let text = "## 2026-05-16 — t\n**why:**   \n";
        let e = &parse_entries(text)[0];
        assert_eq!(e.why, None);
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
