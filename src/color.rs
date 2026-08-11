//! ANSI styling for human-facing entry output.

/// When to emit ANSI color codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    /// Color only on a terminal when `NO_COLOR` is unset.
    Auto,
    /// Always color.
    Always,
    /// Never color.
    Never,
}

const BOLD_CYAN: &str = "\x1b[1;36m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Apply the configured TTY and `NO_COLOR` policy.
pub fn should_colorize(choice: ColorChoice, stdout_is_tty: bool, no_color_set: bool) -> bool {
    match choice {
        ColorChoice::Never => false,
        ColorChoice::Always => true,
        ColorChoice::Auto => stdout_is_tty && !no_color_set,
    }
}

/// Bold the entry header and field labels without changing their text.
pub fn colorize_block(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 32);
    for (i, line) in raw.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if line.starts_with("## ") {
            out.push_str(&format!("{BOLD_CYAN}{line}{RESET}"));
        } else if let Some(rest) = bold_label(line) {
            out.push_str(&rest);
        } else {
            out.push_str(line);
        }
    }
    if raw.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn bold_label(line: &str) -> Option<String> {
    if !line.starts_with("**") {
        return None;
    }
    let close = line.find(":**")?;
    let label_end = close + 3;
    Some(format!(
        "{BOLD}{}{RESET}{}",
        &line[..label_end],
        &line[label_end..]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_policy_covers_auto_and_overrides() {
        assert!(!should_colorize(ColorChoice::Auto, false, false));
        assert!(should_colorize(ColorChoice::Auto, true, false));
        assert!(!should_colorize(ColorChoice::Auto, true, true));
        assert!(should_colorize(ColorChoice::Always, false, true));
        assert!(!should_colorize(ColorChoice::Never, true, false));
    }

    #[test]
    fn colorizes_headers_and_labels_only() {
        let raw = "## 2026-05-16 — t\n**why:** w\nplain body line\n";
        let out = colorize_block(raw);
        assert!(out.contains("\x1b[1;36m## 2026-05-16 — t\x1b[0m"));
        assert!(out.contains("\x1b[1m**why:**\x1b[0m w"));
        assert!(out.contains("\nplain body line\n"));
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn removing_ansi_recovers_the_input() {
        let raw = "## 2026-05-16 — t\n**why:** w\n**rejected:** redis\n";
        let stripped = colorize_block(raw)
            .replace(BOLD_CYAN, "")
            .replace(BOLD, "")
            .replace(RESET, "");
        assert_eq!(stripped, raw);
    }
}
