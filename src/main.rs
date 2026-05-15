use anyhow::{bail, Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_LOGBOOK_FILE: &str = "logbook.md";
const ENV_VAR: &str = "LOGBOOK_FILE";

const HEADER: &str = "# logbook\n\nAppend-only record of architectural decisions for this project.\nNewest entries at the bottom. Generated and maintained by `logbook` — https://github.com/jeffbai996/logbook\n\n";

#[derive(Parser)]
#[command(name = "logbook", version, about = "Per-repo decision log CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create the logbook file at the current directory if it doesn't exist
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

        /// Also run `git add <logbook>` after writing
        #[arg(long)]
        stage: bool,

        /// Echo the rendered entry block to stdout after writing
        #[arg(long)]
        print: bool,
    },

    /// Print entries, newest first, with optional filters
    List {
        /// Filter entries to those carrying this tag (case-insensitive)
        #[arg(long)]
        tag: Option<String>,

        /// Only entries on or after this date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Only entries on or before this date (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,
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

    /// Print the resolved logbook file path (honoring LOGBOOK_FILE env var)
    Where,
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
            print,
        } => add(title, why, rejected, risk, tags, stage, print),
        Cmd::List { tag, since, until } => list(tag.as_deref(), since.as_deref(), until.as_deref()),
        Cmd::Search { term } => search(&term),
        Cmd::Last => last(),
        Cmd::Show { date } => show(&date),
        Cmd::Tags => tags_cmd(),
        Cmd::Stats => stats(),
        Cmd::Where => print_where(),
    }
}

/// Resolve the logbook file path. `LOGBOOK_FILE` overrides; default is `./logbook.md`.
fn logbook_path() -> PathBuf {
    env::var_os(ENV_VAR)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LOGBOOK_FILE))
}

fn ensure_exists() -> Result<()> {
    let p = logbook_path();
    if !p.exists() {
        bail!(
            "no logbook file at {}. Run `logbook init` first (or set {} to point elsewhere).",
            p.display(),
            ENV_VAR
        );
    }
    Ok(())
}

fn init() -> Result<()> {
    let path = logbook_path();
    if path.exists() {
        println!("{} already exists, leaving it alone", path.display());
        return Ok(());
    }
    fs::write(&path, HEADER)
        .with_context(|| format!("failed to create {}", path.display()))?;
    println!("created {}", path.display());
    Ok(())
}

/// Atomically append by reading current contents, appending in memory, writing
/// to a sibling tempfile, then renaming. Renames are atomic on POSIX and on
/// NTFS via ReplaceFile semantics, so a crashed run can't leave a half-written
/// entry behind.
fn atomic_append(path: &Path, block: &str) -> Result<()> {
    let existing = if path.exists() {
        fs::read(path).with_context(|| format!("failed to read {}", path.display()))?
    } else {
        Vec::new()
    };
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("md")
    ));
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .with_context(|| format!("failed to open temp file {}", tmp.display()))?;
        f.write_all(&existing)
            .with_context(|| format!("failed to copy existing contents to {}", tmp.display()))?;
        f.write_all(block.as_bytes())
            .with_context(|| format!("failed to write new entry to {}", tmp.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).with_context(|| {
        format!(
            "failed to rename {} → {} (logbook may be inconsistent)",
            tmp.display(),
            path.display()
        )
    })?;
    Ok(())
}

fn add(
    title: String,
    why: String,
    rejected: Option<String>,
    risk: Option<String>,
    tags: Vec<String>,
    stage: bool,
    print: bool,
) -> Result<()> {
    let path = logbook_path();
    if !path.exists() {
        fs::write(&path, HEADER)
            .with_context(|| format!("failed to auto-init {}", path.display()))?;
        println!("auto-created {}", path.display());
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

    atomic_append(&path, &block)?;

    println!("added: {date} — {title}");

    if print {
        println!("---");
        print!("{block}");
    }

    if stage {
        git_add(&path)?;
        println!("staged {}", path.display());
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
    let p = logbook_path();
    let text = fs::read_to_string(&p).with_context(|| format!("failed to read {}", p.display()))?;
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
    let header = lines.first().copied().unwrap_or("");
    let date = header
        .strip_prefix("## ")
        .and_then(|s| s.split_whitespace().next())
        .filter(|d| is_date_shaped(d))
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

fn is_date_shaped(s: &str) -> bool {
    s.len() == 10
        && s.chars().filter(|c| *c == '-').count() == 2
        && s.chars().filter(|c| c.is_ascii_digit() || *c == '-').count() == s.len()
}

fn validate_date_arg(name: &str, value: &str) -> Result<()> {
    if !is_date_shaped(value) {
        bail!("--{name} must be YYYY-MM-DD (got: \"{value}\")");
    }
    Ok(())
}

fn list(
    tag_filter: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<()> {
    if let Some(s) = since {
        validate_date_arg("since", s)?;
    }
    if let Some(u) = until {
        validate_date_arg("until", u)?;
    }

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
        if let Some(s) = since {
            match entry.date.as_deref() {
                Some(d) if d >= s => {}
                _ => continue,
            }
        }
        if let Some(u) = until {
            match entry.date.as_deref() {
                Some(d) if d <= u => {}
                _ => continue,
            }
        }
        println!("{}\n", entry.raw);
        hits += 1;
    }
    if hits == 0 {
        println!("no entries match the given filters");
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
    validate_date_arg("date", date)?;
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

fn print_where() -> Result<()> {
    let p = logbook_path();
    let abs = p.canonicalize().unwrap_or_else(|_| p.clone());
    println!("{}", abs.display());
    if !p.exists() {
        eprintln!("(file does not exist yet — run `logbook init`)");
    }
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
