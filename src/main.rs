use anyhow::{bail, Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand};
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

        /// Also run `git add logbook.md` after writing
        #[arg(long)]
        stage: bool,
    },

    /// Print all entries, newest first
    List,

    /// Case-insensitive search across entries
    Search {
        /// Term to search for
        term: String,
    },

    /// Print only the most recent entry
    Last,
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
            stage,
        } => add(title, why, rejected, risk, stage),
        Cmd::List => list(),
        Cmd::Search { term } => search(&term),
        Cmd::Last => last(),
    }
}

fn logbook_path() -> PathBuf {
    PathBuf::from(LOGBOOK_FILE)
}

fn ensure_exists() -> Result<()> {
    if !logbook_path().exists() {
        bail!(
            "no logbook.md in current directory. Run `logbook init` first."
        );
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
    stage: bool,
) -> Result<()> {
    let path = logbook_path();
    if !path.exists() {
        // be friendly: auto-init on first add rather than erroring
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

fn read_entries() -> Result<Vec<String>> {
    ensure_exists()?;
    let text = fs::read_to_string(logbook_path()).context("failed to read logbook.md")?;
    let mut entries: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            if !current.is_empty() {
                entries.push(current.join("\n"));
                current.clear();
            }
            current.push(line);
        } else if !current.is_empty() {
            current.push(line);
        }
    }
    if !current.is_empty() {
        entries.push(current.join("\n"));
    }
    // trim trailing blank lines from each entry
    for e in entries.iter_mut() {
        while e.ends_with('\n') || e.ends_with(' ') {
            e.pop();
        }
    }
    Ok(entries)
}

fn list() -> Result<()> {
    let entries = read_entries()?;
    if entries.is_empty() {
        println!("(no entries yet)");
        return Ok(());
    }
    for entry in entries.iter().rev() {
        println!("{entry}\n");
    }
    Ok(())
}

fn search(term: &str) -> Result<()> {
    let entries = read_entries()?;
    let needle = term.to_lowercase();
    let mut hits = 0;
    for entry in entries.iter().rev() {
        if entry.to_lowercase().contains(&needle) {
            println!("{entry}\n");
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
        Some(e) => println!("{e}"),
        None => println!("(no entries yet)"),
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
