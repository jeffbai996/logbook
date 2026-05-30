//! Terminal coloring for human-facing output.
//!
//! Color is applied only to interactive, human-read output (`list`, `last`,
//! `show`, `search`) — never to `export` (machine-readable) and never when
//! stdout is piped. The decision of *whether* to colorize ([`should_colorize`])
//! is separated from *how* ([`colorize_block`]) so the policy is unit-testable
//! without a real terminal, and so piped output stays byte-identical to the
//! uncolored markdown (no ANSI leaking into `| grep`, redirects, or `export`).
//!
//! Honors the [`NO_COLOR`](https://no-color.org/) convention and a
//! `--color <auto|always|never>` flag.

/// When to emit ANSI color codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    /// Color iff stdout is a TTY and `NO_COLOR` is unset (the default).
    Auto,
    /// Always color, even when piped.
    Always,
    /// Never color.
    Never,
}

// ANSI escapes — kept private; callers go through colorize_block.
const BOLD_CYAN: &str = "\x1b[1;36m"; // entry header (## date — title)
const BOLD: &str = "\x1b[1m"; // **field:** labels
const RESET: &str = "\x1b[0m";

/// Decide whether to emit color, given the choice, whether stdout is a TTY,
/// and whether `NO_COLOR` is set in the environment.
///
/// Policy:
/// - `Never` → never.
/// - `Always` → always (ignores TTY and `NO_COLOR`, matching the explicit
///   override semantics most tools use for `--color=always`).
/// - `Auto` → only when stdout is a TTY *and* `NO_COLOR` is unset.
///
/// # Example
///
/// ```
/// use logbook::color::{should_colorize, ColorChoice};
///
/// assert!(!should_colorize(ColorChoice::Auto, false, false)); // piped → no
/// assert!(should_colorize(ColorChoice::Auto, true, false));   // tty → yes
/// assert!(!should_colorize(ColorChoice::Auto, true, true));   // NO_COLOR → no
/// assert!(should_colorize(ColorChoice::Always, false, true)); // forced → yes
/// assert!(!should_colorize(ColorChoice::Never, true, false)); // never → no
/// ```
pub fn should_colorize(choice: ColorChoice, stdout_is_tty: bool, no_color_set: bool) -> bool {
    match choice {
        ColorChoice::Never => false,
        ColorChoice::Always => true,
        ColorChoice::Auto => stdout_is_tty && !no_color_set,
    }
}

/// Colorize a rendered entry block: bold-cyan the `## …` header line and bold
/// the `**field:**` labels. Every other byte is preserved exactly.
///
/// When color is disabled the caller should skip this entirely and print the
/// raw block — but calling this with the intent to colorize is the only path
/// that injects escapes, so uncolored output can never accidentally differ.
///
/// # Example
///
/// ```
/// use logbook::color::colorize_block;
///
/// let raw = "## 2026-05-16 — t\n**why:** w\n";
/// let out = colorize_block(raw);
/// assert!(out.contains("\x1b[1;36m## 2026-05-16 — t\x1b[0m"));
/// assert!(out.contains("\x1b[1m**why:**\x1b[0m w"));
/// ```
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
    // Preserve a trailing newline if the input had one (lines() drops it).
    if raw.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// If `line` begins with a `**label:**` marker, return the line with just that
/// label bolded; otherwise `None`.
fn bold_label(line: &str) -> Option<String> {
    if !line.starts_with("**") {
        return None;
    }
    // Find the closing `:**` of the label.
    let close = line.find(":**")?;
    let label_end = close + 3; // include ":**"
    let label = &line[..label_end];
    let rest = &line[label_end..];
    Some(format!("{BOLD}{label}{RESET}{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_off_when_piped() {
        assert!(!should_colorize(ColorChoice::Auto, false, false));
    }

    #[test]
    fn auto_on_when_tty_and_no_no_color() {
        assert!(should_colorize(ColorChoice::Auto, true, false));
    }

    #[test]
    fn auto_off_when_no_color_set_even_on_tty() {
        assert!(!should_colorize(ColorChoice::Auto, true, true));
    }

    #[test]
    fn always_colors_even_piped_and_no_color() {
        assert!(should_colorize(ColorChoice::Always, false, true));
    }

    #[test]
    fn never_off_even_on_tty() {
        assert!(!should_colorize(ColorChoice::Never, true, false));
    }

    #[test]
    fn colorize_bolds_header_and_labels() {
        let raw = "## 2026-05-16 — t\n**why:** w\n**tags:** a, b";
        let out = colorize_block(raw);
        assert!(out.contains("\x1b[1;36m## 2026-05-16 — t\x1b[0m"));
        assert!(out.contains("\x1b[1m**why:**\x1b[0m w"));
        assert!(out.contains("\x1b[1m**tags:**\x1b[0m a, b"));
    }

    #[test]
    fn colorize_preserves_non_marker_lines_verbatim() {
        let raw = "## h\nplain body line\n**why:** w";
        let out = colorize_block(raw);
        assert!(out.contains("\nplain body line\n"));
    }

    #[test]
    fn colorize_preserves_trailing_newline() {
        assert!(colorize_block("## h\n").ends_with('\n'));
        assert!(!colorize_block("## h").ends_with('\n'));
    }

    #[test]
    fn stripping_color_codes_recovers_original() {
        // The colorized output, with escapes removed, equals the input — proving
        // colorize only ADDS escapes and never mutates real content.
        let raw = "## 2026-05-16 — t\n**why:** w\n**rejected:** redis\n";
        let colored = colorize_block(raw);
        let stripped = colored
            .replace(BOLD_CYAN, "")
            .replace(BOLD, "")
            .replace(RESET, "");
        assert_eq!(stripped, raw);
    }
}
