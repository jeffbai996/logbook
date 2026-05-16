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
