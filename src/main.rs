use anyhow::{bail, Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const LOGBOOK_FILE: &str = "logbook.md";
const HEADER: &str = "# logbook\n\nAppend-only record of architectural decisions for this project.\nNewest entries at the bottom. Generated and maintained by `logbook` — https://github.com/jeffbai996/logbook\n\n";

#[derive(Parser)]
#[command(name = "logbook", version, about = "Per-repo decision log CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create logbook.md at the current directory if it doesn't exist
    Init,

    /// Append a new entry
    Add {
        /// Short title for the entry
        title: String,

        /// Why this decision was made
        #[arg(long)]
        why: String,

        /// Alternatives that were rejected, with brief reasons
        #[arg(long)]
        rejected: Option<String>,

        /// What could go wrong with this choice
        #[arg(long)]
        risk: Option<String>,

        /// One or more tags (repeatable: --tag refactor --tag db)
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,

        /// Also run `git add logbook.md` after writing
        #[arg(long)]
        stage: bool,
    },

    /// Print all entries, newest first
    List {
        /// Filter entries to those carrying this tag (case-insensitive)
        #[arg(long)]
        tag: Option<String>,
    },

    /// Case-insensitive search across entries
    Search {
        /// Term to search for
        term: String,
    },

    /// Print only the most recent entry
    Last,

    /// Print all entries from a given date (YYYY-MM-DD)
    Show {
        /// Date string, e.g. 2026-05-15
        date: String,
    },

    /// List all distinct tags with usage counts
    Tags,

    /// Summary statistics: total entries, date range, entries this month
    Stats,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init => init(),
        Cmd::Add {
            title,
            why,
            rejected,
            risk,
            tags,
            stage,
        } => add(title, why, rejected, risk, tags, stage),
        Cmd::List { tag } => list(tag.as_deref()),
        Cmd::Search { term } => search(&term),
        Cmd::Last => last(),
        Cmd::Show { date } => show(&date),
        Cmd::Tags => tags_cmd(),
        Cmd::Stats => stats(),
    }
}

fn logbook_path() -> PathBuf {
    PathBuf::from(LOGBOOK_FILE)
}

fn ensure_exists() -> Result<()> {
    if !logbook_path().exists() {
        bail!("no logbook.md in current directory. Run `logbook init` first.");
    }
    Ok(())
}

fn init() -> Result<()> {
    let path = logbook_path();
    if path.exists() {
        println!("logbook.md already exists, leaving it alone");
        return Ok(());
    }
    fs::write(&path, HEADER).context("failed to create logbook.md")?;
    println!("created logbook.md");
    Ok(())
}

fn add(
    title: String,
    why: String,
    rejected: Option<String>,
    risk: Option<String>,
    tags: Vec<String>,
    stage: bool,
) -> Result<()> {
    let path = logbook_path();
    if !path.exists() {
        fs::write(&path, HEADER).context("failed to auto-init logbook.md")?;
        println!("auto-created logbook.md");
    }

    let date = Local::now().format("%Y-%m-%d").to_string();
    let mut block = format!("## {date} — {title}\n**why:** {why}\n");
    if let Some(r) = rejected.filter(|s| !s.trim().is_empty()) {
        block.push_str(&format!("**rejected:** {r}\n"));
    }
    if let Some(r) = risk.filter(|s| !s.trim().is_empty()) {
        block.push_str(&format!("**risk:** {r}\n"));
    }
    let clean_tags: Vec<String> = tags
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if !clean_tags.is_empty() {
        block.push_str(&format!("**tags:** {}\n", clean_tags.join(", ")));
    }
    block.push('\n');

    let mut f = OpenOptions::new()
        .append(true)
        .open(&path)
        .context("failed to open logbook.md for append")?;
    f.write_all(block.as_bytes())
        .context("failed to write entry")?;

    println!("added: {date} — {title}");

    if stage {
        git_add(&path)?;
        println!("staged logbook.md");
    }

    Ok(())
}

/// One parsed entry. We keep the raw markdown around for printing, plus
/// the structured bits we need for filtering/stats.
struct Entry {
    raw: String,
    date: Option<String>,
    tags: Vec<String>,
}

fn read_entries() -> Result<Vec<Entry>> {
    ensure_exists()?;
    let text = fs::read_to_string(logbook_path()).context("failed to read logbook.md")?;
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
    Ok(entries)
}

fn make_entry(lines: &[&str]) -> Entry {
    let mut raw = lines.join("\n");
    while raw.ends_with('\n') || raw.ends_with(' ') {
        raw.pop();
    }
    // Header form: "## YYYY-MM-DD — title"
    let header = lines.first().copied().unwrap_or("");
    let date = header
        .strip_prefix("## ")
        .and_then(|s| s.split_whitespace().next())
        .filter(|d| d.len() == 10 && d.chars().filter(|c| *c == '-').count() == 2)
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

fn list(tag_filter: Option<&str>) -> Result<()> {
    let entries = read_entries()?;
    if entries.is_empty() {
        println!("(no entries yet)");
        return Ok(());
    }
    let needle = tag_filter.map(|t| t.to_lowercase());
    let mut hits = 0;
    for entry in entries.iter().rev() {
        if let Some(ref n) = needle {
            if !entry.tags.iter().any(|t| t.to_lowercase() == *n) {
                continue;
            }
        }
        println!("{}\n", entry.raw);
        hits += 1;
    }
    if hits == 0 {
        if let Some(t) = tag_filter {
            println!("no entries tagged \"{t}\"");
        }
    }
    Ok(())
}

fn search(term: &str) -> Result<()> {
    let entries = read_entries()?;
    let needle = term.to_lowercase();
    let mut hits = 0;
    for entry in entries.iter().rev() {
        if entry.raw.to_lowercase().contains(&needle) {
            println!("{}\n", entry.raw);
            hits += 1;
        }
    }
    if hits == 0 {
        println!("no entries match \"{term}\"");
    }
    Ok(())
}

fn last() -> Result<()> {
    let entries = read_entries()?;
    match entries.last() {
        Some(e) => println!("{}", e.raw),
        None => println!("(no entries yet)"),
    }
    Ok(())
}

fn show(date: &str) -> Result<()> {
    // Validate the date shape — we accept anything that looks like YYYY-MM-DD.
    if date.len() != 10 || date.chars().filter(|c| *c == '-').count() != 2 {
        bail!("date must be in YYYY-MM-DD form (got: \"{date}\")");
    }
    let entries = read_entries()?;
    let mut hits = 0;
    for entry in entries.iter() {
        if entry.date.as_deref() == Some(date) {
            println!("{}\n", entry.raw);
            hits += 1;
        }
    }
    if hits == 0 {
        println!("no entries on {date}");
    }
    Ok(())
}

fn tags_cmd() -> Result<()> {
    let entries = read_entries()?;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &entries {
        for t in &entry.tags {
            *counts.entry(t.to_lowercase()).or_insert(0) += 1;
        }
    }
    if counts.is_empty() {
        println!("(no tags yet — add entries with --tag <name>)");
        return Ok(());
    }
    // Sort by descending count, then alpha
    let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let max_name = rows.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    for (name, count) in rows {
        println!("{name:<max_name$}  {count}");
    }
    Ok(())
}

fn stats() -> Result<()> {
    let entries = read_entries()?;
    let total = entries.len();
    if total == 0 {
        println!("(no entries yet)");
        return Ok(());
    }
    let dates: Vec<&str> = entries.iter().filter_map(|e| e.date.as_deref()).collect();
    let first = dates.iter().min().copied().unwrap_or("?");
    let last = dates.iter().max().copied().unwrap_or("?");
    let this_month_prefix = Local::now().format("%Y-%m").to_string();
    let this_month = dates
        .iter()
        .filter(|d| d.starts_with(&this_month_prefix))
        .count();
    let unique_tags = {
        let mut s = std::collections::HashSet::new();
        for e in &entries {
            for t in &e.tags {
                s.insert(t.to_lowercase());
            }
        }
        s.len()
    };

    println!("total entries: {total}");
    println!("date range:    {first} → {last}");
    println!("this month:    {this_month}");
    println!("unique tags:   {unique_tags}");
    Ok(())
}

fn git_add(path: &Path) -> Result<()> {
    let status = Command::new("git")
        .arg("add")
        .arg(path)
        .status()
        .context("failed to run git add — is git installed and is this a git repo?")?;
    if !status.success() {
        bail!("git add exited with status {status}");
    }
    Ok(())
}
