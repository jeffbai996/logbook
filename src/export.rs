//! JSON export for logbook entries.
//!
//! Emits a JSON array of entry objects for tooling integrations — indexing
//! into a search system, ingesting into a memory store, feeding an LLM agent
//! a repo's decision history as structured data rather than raw markdown.
//!
//! JSON is hand-rendered rather than pulling in `serde` + `serde_json`: the
//! schema is a fixed, flat shape (six string/array fields), so a small
//! dependency-free encoder keeps logbook's "single binary, three deps" ethos
//! intact. The encoder escapes per the JSON spec (RFC 8259) — quotes,
//! backslashes, control characters.

use crate::parse::Entry;

/// Render a slice of entries as a pretty-printed JSON array.
///
/// Each entry becomes an object with keys `date`, `title`, `why`, `rejected`,
/// `risk`, `tags`. Absent optional fields (`date`, `title`, `why`, `rejected`,
/// `risk`) serialize as JSON `null`; `tags` is always an array (empty if none).
/// Field order is fixed and stable so the output diffs cleanly.
///
/// An empty slice renders as `[]`.
///
/// # Example
///
/// ```
/// use logbook::{parse_entries, entries_to_json};
///
/// let entries = parse_entries("## 2026-05-16 — t\n**why:** w\n**tags:** a, b\n");
/// let json = entries_to_json(&entries);
/// assert!(json.contains("\"date\": \"2026-05-16\""));
/// assert!(json.contains("\"tags\": [\n      \"a\",\n      \"b\"\n    ]"));
/// ```
pub fn entries_to_json(entries: &[Entry]) -> String {
    if entries.is_empty() {
        return "[]".to_string();
    }
    let mut out = String::from("[\n");
    for (i, e) in entries.iter().enumerate() {
        out.push_str("  {\n");
        out.push_str(&format!("    \"date\": {},\n", opt_str(e.date.as_deref())));
        out.push_str(&format!(
            "    \"title\": {},\n",
            opt_str(e.title.as_deref())
        ));
        out.push_str(&format!("    \"why\": {},\n", opt_str(e.why.as_deref())));
        out.push_str(&format!(
            "    \"rejected\": {},\n",
            opt_str(e.rejected.as_deref())
        ));
        out.push_str(&format!("    \"risk\": {},\n", opt_str(e.risk.as_deref())));
        out.push_str(&format!(
            "    \"supersedes\": {},\n",
            opt_str(e.supersedes.as_deref())
        ));
        out.push_str(&format!("    \"tags\": {}\n", str_array(&e.tags)));
        out.push_str("  }");
        if i + 1 < entries.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    out
}

/// Render an optional string as a JSON string literal, or `null` if `None`.
fn opt_str(v: Option<&str>) -> String {
    match v {
        Some(s) => json_string(s),
        None => "null".to_string(),
    }
}

/// Render a string slice as a JSON array of strings, indented to sit under
/// a 4-space-indented key. Empty slice → `[]`.
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

/// Encode a string as a JSON string literal per RFC 8259, including the
/// surrounding quotes. Escapes `"`, `\`, and the control characters that
/// must be escaped; emits `\uXXXX` for other control chars below 0x20.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
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
    use crate::parse::parse_entries;

    #[test]
    fn empty_entries_render_as_empty_array() {
        assert_eq!(entries_to_json(&[]), "[]");
    }

    #[test]
    fn full_entry_serializes_all_fields() {
        let entries = parse_entries(
            "## 2026-05-16 — switched ORM\n**why:** perf\n**rejected:** redis\n**risk:** migrations\n**tags:** db, perf\n",
        );
        let json = entries_to_json(&entries);
        assert!(json.contains("\"date\": \"2026-05-16\""));
        assert!(json.contains("\"title\": \"switched ORM\""));
        assert!(json.contains("\"why\": \"perf\""));
        assert!(json.contains("\"rejected\": \"redis\""));
        assert!(json.contains("\"risk\": \"migrations\""));
        assert!(json.contains("\"db\""));
        assert!(json.contains("\"perf\""));
    }

    #[test]
    fn absent_optionals_render_as_null() {
        let entries = parse_entries("## 2026-05-16 — t\n**why:** w\n");
        let json = entries_to_json(&entries);
        assert!(json.contains("\"rejected\": null"));
        assert!(json.contains("\"risk\": null"));
    }

    #[test]
    fn no_tags_renders_empty_array() {
        let entries = parse_entries("## 2026-05-16 — t\n**why:** w\n");
        let json = entries_to_json(&entries);
        assert!(json.contains("\"tags\": []"));
    }

    #[test]
    fn special_characters_are_escaped() {
        let entries = parse_entries("## 2026-05-16 — quote \" and back\\slash\n**why:** line1\n");
        let json = entries_to_json(&entries);
        assert!(json.contains("\\\""), "double-quote must be escaped");
        assert!(json.contains("\\\\"), "backslash must be escaped");
        // The produced JSON must itself be valid (no raw unescaped quote breaks it).
        assert_eq!(json.matches("\"title\":").count(), 1);
    }

    #[test]
    fn multiple_entries_form_a_comma_separated_array() {
        let entries =
            parse_entries("## 2026-05-15 — a\n**why:** x\n\n## 2026-05-16 — b\n**why:** y\n");
        let json = entries_to_json(&entries);
        assert!(json.starts_with("[\n"));
        assert!(json.trim_end().ends_with(']'));
        // Two objects → exactly one separating "},\n  {".
        assert_eq!(json.matches("  },\n  {").count(), 1);
    }

    #[test]
    fn output_is_valid_json_roundtrip_shape() {
        // Light structural check without pulling in a JSON parser dep: balanced
        // brackets and the expected key count for one entry.
        let entries = parse_entries("## 2026-05-16 — t\n**why:** w\n**tags:** a\n");
        let json = entries_to_json(&entries);
        assert_eq!(json.matches('[').count(), json.matches(']').count());
        assert_eq!(json.matches('{').count(), json.matches('}').count());
        for key in [
            "date",
            "title",
            "why",
            "rejected",
            "risk",
            "supersedes",
            "tags",
        ] {
            assert!(json.contains(&format!("\"{key}\":")), "missing key {key}");
        }
    }
}
