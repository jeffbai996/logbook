//! Line-oriented parser for the logbook Markdown format.

/// A parsed entry. `raw` preserves the original entry block for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub raw: String,
    pub date: Option<String>,
    pub title: Option<String>,
    pub why: Option<String>,
    pub rejected: Option<String>,
    pub risk: Option<String>,
    /// Date and, for new entries, title of the superseded decision.
    pub supersedes: Option<String>,
    pub tags: Vec<String>,
}

/// Parse entries in document order, ignoring text before the first `## `.
pub fn parse_entries(text: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut current = Vec::new();

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
    let raw = lines.join("\n").trim_end().to_string();
    let heading = lines[0].strip_prefix("## ").unwrap_or(lines[0]);
    let date = heading
        .split_whitespace()
        .next()
        .filter(|date| crate::is_date_shaped(date))
        .map(str::to_string);

    let title = if date.is_some() {
        heading
            .split_once(" — ")
            .map(|(_, title)| title.trim().to_string())
            .filter(|title| !title.is_empty())
            .or_else(|| Some(heading.trim().to_string()))
    } else {
        let title = heading.trim();
        (!title.is_empty()).then(|| title.to_string())
    };

    let tags = lines
        .iter()
        .find_map(|line| line.strip_prefix("**tags:**"))
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Entry {
        raw,
        date,
        title,
        why: first_field(lines, "**why:**"),
        rejected: first_field(lines, "**rejected:**"),
        risk: first_field(lines, "**risk:**"),
        supersedes: first_field(lines, "**supersedes:**"),
        tags,
    }
}

fn first_field(lines: &[&str], prefix: &str) -> Option<String> {
    lines.iter().find_map(|line| {
        line.strip_prefix(prefix).and_then(|value| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_preamble_only_has_no_entries() {
        assert!(parse_entries("").is_empty());
        assert!(parse_entries("# logbook\n\npreamble\n").is_empty());
    }

    #[test]
    fn parses_all_canonical_fields() {
        let entry = &parse_entries(
            "## 2026-05-16 — switched ORM\n**why:** perf\n**supersedes:** 2026-05-01 — old ORM\n**rejected:** redis\n**risk:** migrations\n**tags:** db, perf\n",
        )[0];
        assert_eq!(entry.date.as_deref(), Some("2026-05-16"));
        assert_eq!(entry.title.as_deref(), Some("switched ORM"));
        assert_eq!(entry.why.as_deref(), Some("perf"));
        assert_eq!(entry.supersedes.as_deref(), Some("2026-05-01 — old ORM"));
        assert_eq!(entry.rejected.as_deref(), Some("redis"));
        assert_eq!(entry.risk.as_deref(), Some("migrations"));
        assert_eq!(entry.tags, ["db", "perf"]);
    }

    #[test]
    fn preserves_document_order_and_drops_preamble() {
        let entries = parse_entries(
            "# logbook\n\npreamble\n\n## 2026-05-15 — first\n**why:** a\n\n## 2026-05-16 — second\n**why:** b\n",
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title.as_deref(), Some("first"));
        assert_eq!(entries[1].title.as_deref(), Some("second"));
    }

    #[test]
    fn malformed_header_keeps_a_useful_title_without_a_date() {
        let entry = &parse_entries("## not-a-date wat\n**why:** w\n")[0];
        assert_eq!(entry.date, None);
        assert_eq!(entry.title.as_deref(), Some("not-a-date wat"));
    }

    #[test]
    fn title_can_contain_an_em_dash() {
        let entry = &parse_entries("## 2026-05-16 — switched ORM — finally\n**why:** w\n")[0];
        assert_eq!(entry.title.as_deref(), Some("switched ORM — finally"));
    }

    #[test]
    fn tag_parsing_trims_empties_and_uses_the_first_field() {
        let entry = &parse_entries(
            "## 2026-05-16 — t\n**why:** w\n**tags:**  refactor ,, perf  ,\n**tags:** ignored\n",
        )[0];
        assert_eq!(entry.tags, ["refactor", "perf"]);
    }

    #[test]
    fn missing_or_empty_optional_fields_are_none() {
        let entry = &parse_entries("## 2026-05-16 — t\n**why:**   \n")[0];
        assert_eq!(entry.why, None);
        assert_eq!(entry.rejected, None);
        assert_eq!(entry.risk, None);
        assert_eq!(entry.supersedes, None);
        assert!(entry.tags.is_empty());
    }

    #[test]
    fn raw_entry_has_no_trailing_whitespace() {
        let entry = &parse_entries("## 2026-05-16 — t\n**why:** w\n\n  \n")[0];
        assert_eq!(entry.raw, "## 2026-05-16 — t\n**why:** w");
    }
}
