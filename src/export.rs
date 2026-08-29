//! Stable JSON export for parsed entries.

use crate::Entry;

/// Render entries as a pretty-printed JSON array in document order.
pub fn entries_to_json(entries: &[Entry]) -> String {
    if entries.is_empty() {
        return "[]".to_string();
    }
    let mut out = String::from("[\n");
    for (i, entry) in entries.iter().enumerate() {
        out.push_str("  {\n");
        out.push_str(&format!(
            "    \"date\": {},\n",
            opt_str(entry.date.as_deref())
        ));
        out.push_str(&format!(
            "    \"title\": {},\n",
            opt_str(entry.title.as_deref())
        ));
        out.push_str(&format!(
            "    \"why\": {},\n",
            opt_str(entry.why.as_deref())
        ));
        out.push_str(&format!(
            "    \"rejected\": {},\n",
            opt_str(entry.rejected.as_deref())
        ));
        out.push_str(&format!(
            "    \"risk\": {},\n",
            opt_str(entry.risk.as_deref())
        ));
        out.push_str(&format!(
            "    \"supersedes\": {},\n",
            opt_str(entry.supersedes.as_deref())
        ));
        out.push_str(&format!("    \"tags\": {},\n", str_array(&entry.tags)));
        out.push_str(&format!("    \"active\": {},\n", entry.is_active()));
        out.push_str(&format!(
            "    \"superseded_by\": {}\n",
            str_array(&entry.superseded_by)
        ));
        out.push_str("  }");
        if i + 1 < entries.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    out
}

/// Render one compact JSON object per line in document order.
pub fn entries_to_json_lines(entries: &[Entry]) -> String {
    entries
        .iter()
        .map(|entry| {
            format!(
                "{{\"date\":{},\"title\":{},\"why\":{},\"rejected\":{},\"risk\":{},\"supersedes\":{},\"tags\":{},\"active\":{},\"superseded_by\":{}}}",
                opt_str(entry.date.as_deref()),
                opt_str(entry.title.as_deref()),
                opt_str(entry.why.as_deref()),
                opt_str(entry.rejected.as_deref()),
                opt_str(entry.risk.as_deref()),
                opt_str(entry.supersedes.as_deref()),
                compact_str_array(&entry.tags),
                entry.is_active(),
                compact_str_array(&entry.superseded_by),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn opt_str(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

fn str_array(items: &[String]) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }
    let mut out = String::from("[\n");
    for (i, item) in items.iter().enumerate() {
        out.push_str(&format!("      {}", json_string(item)));
        if i + 1 < items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("    ]");
    out
}

fn compact_str_array(items: &[String]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|item| json_string(item))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_entries;

    #[test]
    fn empty_entries_render_as_empty_array() {
        assert_eq!(entries_to_json(&[]), "[]");
    }

    #[test]
    fn serializes_the_stable_entry_shape() {
        let entries = parse_entries(
            "## 2026-05-16 — switched ORM\n**why:** perf\n**supersedes:** 2026-05-01 — old ORM\n**rejected:** redis\n**risk:** migrations\n**tags:** db, perf\n",
        );
        let json = entries_to_json(&entries);
        for expected in [
            "\"date\": \"2026-05-16\"",
            "\"title\": \"switched ORM\"",
            "\"why\": \"perf\"",
            "\"supersedes\": \"2026-05-01 — old ORM\"",
            "\"rejected\": \"redis\"",
            "\"risk\": \"migrations\"",
            "\"db\"",
            "\"perf\"",
            "\"active\": true",
            "\"superseded_by\": []",
        ] {
            assert!(json.contains(expected), "{expected}");
        }
    }

    #[test]
    fn absent_fields_and_tags_have_stable_values() {
        let json = entries_to_json(&parse_entries("## 2026-05-16 — t\n**why:** w\n"));
        assert!(json.contains("\"rejected\": null"));
        assert!(json.contains("\"risk\": null"));
        assert!(json.contains("\"supersedes\": null"));
        assert!(json.contains("\"tags\": []"));
    }

    #[test]
    fn escapes_json_special_characters() {
        assert_eq!(
            json_string("\"\\\n\r\t\u{08}\u{0C}\u{01}"),
            "\"\\\"\\\\\\n\\r\\t\\b\\f\\u0001\""
        );
    }

    #[test]
    fn json_lines_are_compact_and_include_derived_state() {
        let entries = parse_entries(
            "## 2026-01-01 — old\n**why:** a\n\n\
             ## 2026-01-02 — new\n**why:** b\n**supersedes:** 2026-01-01 — old\n",
        );
        let lines = entries_to_json_lines(&entries);
        assert_eq!(lines.lines().count(), 2);
        assert!(lines.lines().next().unwrap().contains("\"active\":false"));
        assert!(lines.contains("\"superseded_by\":[\"2026-01-02 — new\"]"));
    }
}
