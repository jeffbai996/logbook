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
    /// Canonical references to later decisions that supersede this entry.
    pub superseded_by: Vec<String>,
}

impl Entry {
    /// Return the canonical `date — title` reference when both fields exist.
    pub fn reference(&self) -> Option<String> {
        Some(format!(
            "{} — {}",
            self.date.as_deref()?,
            self.title.as_deref()?
        ))
    }

    /// Whether no later entry is known to supersede this decision.
    pub fn is_active(&self) -> bool {
        self.superseded_by.is_empty()
    }
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
    link_supersessions(&mut entries);
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
        why: text_field(lines, "**why:**"),
        rejected: text_field(lines, "**rejected:**"),
        risk: text_field(lines, "**risk:**"),
        supersedes: text_field(lines, "**supersedes:**"),
        tags,
        superseded_by: Vec::new(),
    }
}

const FIELD_PREFIXES: [&str; 5] = [
    "**why:**",
    "**supersedes:**",
    "**rejected:**",
    "**risk:**",
    "**tags:**",
];

fn text_field(lines: &[&str], prefix: &str) -> Option<String> {
    let index = lines.iter().position(|line| line.starts_with(prefix))?;
    let mut value = vec![lines[index].strip_prefix(prefix).unwrap_or_default().trim()];
    for line in &lines[index + 1..] {
        if FIELD_PREFIXES.iter().any(|field| line.starts_with(field)) {
            break;
        }
        value.push(line);
    }
    while value.last().is_some_and(|line| line.trim().is_empty()) {
        value.pop();
    }
    let value = value.join("\n").trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn link_supersessions(entries: &mut [Entry]) {
    for child in 0..entries.len() {
        let Some(reference) = entries[child].supersedes.as_deref() else {
            continue;
        };
        let targets = matching_prior_entries(entries, child, reference);
        if let [target] = targets.as_slice() {
            if let Some(child_reference) = entries[child].reference() {
                entries[*target].superseded_by.push(child_reference);
            }
        }
    }
}

pub(crate) fn matching_prior_entries(
    entries: &[Entry],
    before: usize,
    reference: &str,
) -> Vec<usize> {
    let titled = reference.contains(" — ");
    entries[..before]
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let matches = if titled {
                entry.reference().as_deref() == Some(reference)
            } else {
                entry.date.as_deref() == Some(reference)
            };
            matches.then_some(index)
        })
        .collect()
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
        assert!(entry.superseded_by.is_empty());
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
    fn dated_header_without_the_delimiter_has_no_title() {
        let entry = &parse_entries("## 2026-05-16\n**why:** w\n")[0];
        assert_eq!(entry.date.as_deref(), Some("2026-05-16"));
        assert_eq!(entry.title, None);
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

    #[test]
    fn multiline_fields_survive_parsing() {
        let entry = &parse_entries(
            "## 2026-05-16 — t\n**why:** first paragraph\nsecond line\n\nthird paragraph\n**risk:** first risk\nsecond risk\n**tags:** docs\n",
        )[0];
        assert_eq!(
            entry.why.as_deref(),
            Some("first paragraph\nsecond line\n\nthird paragraph")
        );
        assert_eq!(entry.risk.as_deref(), Some("first risk\nsecond risk"));
    }

    #[test]
    fn derives_forward_supersession_links_for_titled_and_legacy_references() {
        let entries = parse_entries(
            "## 2026-01-01 — first\n**why:** a\n\n\
             ## 2026-01-02 — second\n**why:** b\n**supersedes:** 2026-01-01 — first\n\n\
             ## 2026-01-03 — third\n**why:** c\n**supersedes:** 2026-01-02\n",
        );
        assert_eq!(entries[0].superseded_by, ["2026-01-02 — second"]);
        assert_eq!(entries[1].superseded_by, ["2026-01-03 — third"]);
        assert!(entries[2].is_active());
    }
}
