use clap::{Parser, Subcommand};
use logbook::{
    atomic_append, entries_to_json, init_file, is_date_shaped, logbook_path, parse_entries,
    read_text, render_entry_block, today, Entry, Error, RenderInput, Result, ENV_VAR,
};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, ExitCode};

#[derive(Parser)]
#[command(name = "logbook", version, about = "Per-repo decision log CLI", long_about = None)]
struct Cli {
    /// When to colorize human-facing output (list/last/show/search)
    #[arg(long, value_enum, default_value_t = ColorArg::Auto, global = true)]
    color: ColorArg,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ColorArg {
    /// Color only when stdout is a terminal and NO_COLOR is unset
    Auto,
    /// Always color
    Always,
    /// Never color
    Never,
}

impl From<ColorArg> for logbook::ColorChoice {
    fn from(a: ColorArg) -> Self {
        match a {
            ColorArg::Auto => logbook::ColorChoice::Auto,
            ColorArg::Always => logbook::ColorChoice::Always,
            ColorArg::Never => logbook::ColorChoice::Never,
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// Create the logbook file at the current directory if it doesn't exist
    Init,

    /// Append a new entry
    Add {
        /// Short title for the entry
        title: String,
        /// The reason for the decision. If omitted, opens $EDITOR to compose it.
        #[arg(long)]
        why: Option<String>,
        #[arg(long)]
        rejected: Option<String>,
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
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        until: Option<String>,
    },

    /// Case-insensitive search across entries
    Search { term: String },

    /// Print only the most recent entry
    Last,

    /// Print all entries from a given date (YYYY-MM-DD)
    Show { date: String },

    /// List all distinct tags with usage counts
    Tags,

    /// Summary statistics: total entries, date range, entries this month
    Stats,

    /// Print the resolved logbook file path (honors LOGBOOK_FILE)
    Where,

    /// Export all entries as structured data (currently JSON)
    Export {
        /// Output format
        #[arg(long, value_enum, default_value_t = ExportFormat::Json)]
        format: ExportFormat,
    },

    /// Append a new entry that formally supersedes an earlier one
    Supersede {
        /// Date (YYYY-MM-DD) of the entry being superseded — must exist
        old_date: String,
        /// Short title for the new (superseding) entry
        title: String,
        /// The reason for the change. If omitted, opens $EDITOR to compose it.
        #[arg(long)]
        why: Option<String>,
        #[arg(long)]
        rejected: Option<String>,
        #[arg(long)]
        risk: Option<String>,
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
        /// Also run `git add <logbook>` after writing
        #[arg(long)]
        stage: bool,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ExportFormat {
    /// JSON array of entry objects
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // Resolve colorization once: the flag + live TTY/NO_COLOR state.
    let colorize = logbook::should_colorize(
        cli.color.into(),
        std::io::IsTerminal::is_terminal(&std::io::stdout()),
        std::env::var_os("NO_COLOR").is_some(),
    );
    match dispatch(cli.cmd, colorize) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

/// Print an entry's raw block, colorized iff `colorize`. Trailing blank line
/// matches the previous `println!("{}\n", raw)` spacing.
fn emit(raw: &str, colorize: bool) {
    if colorize {
        println!("{}\n", logbook::colorize_block(raw));
    } else {
        println!("{raw}\n");
    }
}

fn dispatch(cmd: Cmd, colorize: bool) -> Result<()> {
    match cmd {
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
        Cmd::List { tag, since, until } => {
            list(tag.as_deref(), since.as_deref(), until.as_deref(), colorize)
        }
        Cmd::Search { term } => search(&term, colorize),
        Cmd::Last => last(colorize),
        Cmd::Show { date } => show(&date, colorize),
        Cmd::Tags => tags_cmd(),
        Cmd::Stats => stats(),
        Cmd::Where => print_where(),
        Cmd::Export { format } => export(format),
        Cmd::Supersede {
            old_date,
            title,
            why,
            rejected,
            risk,
            tags,
            stage,
        } => supersede(old_date, title, why, rejected, risk, tags, stage),
    }
}

fn init() -> Result<()> {
    let path = logbook_path();
    if init_file(&path)? {
        println!("created {}", path.display());
    } else {
        println!("{} already exists, leaving it alone", path.display());
    }
    Ok(())
}

fn add(
    title: String,
    why: Option<String>,
    rejected: Option<String>,
    risk: Option<String>,
    tags: Vec<String>,
    stage: bool,
    print: bool,
) -> Result<()> {
    // Resolve `why`: use the flag if given, otherwise open $EDITOR to compose it.
    let why = match why {
        Some(w) => w,
        None => logbook::editor::capture_via_editor()?,
    };

    let path = logbook_path();
    if init_file(&path)? {
        println!("auto-created {}", path.display());
    }

    let date = today();
    let block = render_entry_block(&RenderInput {
        date: &date,
        title: &title,
        why: &why,
        rejected: rejected.as_deref(),
        risk: risk.as_deref(),
        tags: &tags,
        supersedes: None,
    });
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

#[allow(clippy::too_many_arguments)]
fn supersede(
    old_date: String,
    title: String,
    why: Option<String>,
    rejected: Option<String>,
    risk: Option<String>,
    tags: Vec<String>,
    stage: bool,
) -> Result<()> {
    validate_date_arg("old_date", &old_date)?;

    // The target entry must exist — you can't supersede a decision never recorded.
    let existing = load_entries()?;
    if !existing
        .iter()
        .any(|e| e.date.as_deref() == Some(&old_date))
    {
        return Err(Error::SupersedeTargetMissing(old_date));
    }

    let why = match why {
        Some(w) => w,
        None => logbook::editor::capture_via_editor()?,
    };

    let path = logbook_path();
    let date = today();
    let block = render_entry_block(&RenderInput {
        date: &date,
        title: &title,
        why: &why,
        rejected: rejected.as_deref(),
        risk: risk.as_deref(),
        tags: &tags,
        supersedes: Some(&old_date),
    });
    atomic_append(&path, &block)?;
    println!("added: {date} — {title} (supersedes {old_date})");

    if stage {
        git_add(&path)?;
        println!("staged {}", path.display());
    }
    Ok(())
}

fn validate_date_arg(flag: &str, value: &str) -> Result<()> {
    if !is_date_shaped(value) {
        return Err(Error::BadDate {
            flag: flag.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

fn load_entries() -> Result<Vec<Entry>> {
    let text = read_text(&logbook_path())?;
    Ok(parse_entries(&text))
}

fn list(
    tag_filter: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
    colorize: bool,
) -> Result<()> {
    if let Some(s) = since {
        validate_date_arg("since", s)?;
    }
    if let Some(u) = until {
        validate_date_arg("until", u)?;
    }

    let entries = load_entries()?;
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
        emit(&entry.raw, colorize);
        hits += 1;
    }
    if hits == 0 {
        println!("no entries match the given filters");
    }
    Ok(())
}

fn search(term: &str, colorize: bool) -> Result<()> {
    let entries = load_entries()?;
    let needle = term.to_lowercase();
    let mut hits = 0;
    for entry in entries.iter().rev() {
        if entry.raw.to_lowercase().contains(&needle) {
            emit(&entry.raw, colorize);
            hits += 1;
        }
    }
    if hits == 0 {
        println!("no entries match \"{term}\"");
    }
    Ok(())
}

fn last(colorize: bool) -> Result<()> {
    let entries = load_entries()?;
    match entries.last() {
        Some(e) => {
            if colorize {
                println!("{}", logbook::colorize_block(&e.raw));
            } else {
                println!("{}", e.raw);
            }
        }
        None => println!("(no entries yet)"),
    }
    Ok(())
}

fn show(date: &str, colorize: bool) -> Result<()> {
    validate_date_arg("date", date)?;
    let entries = load_entries()?;
    let mut hits = 0;
    for entry in entries.iter() {
        if entry.date.as_deref() == Some(date) {
            emit(&entry.raw, colorize);
            hits += 1;
        }
    }
    if hits == 0 {
        println!("no entries on {date}");
    }
    Ok(())
}

fn tags_cmd() -> Result<()> {
    let entries = load_entries()?;
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
    let entries = load_entries()?;
    let total = entries.len();
    if total == 0 {
        println!("(no entries yet)");
        return Ok(());
    }
    let dates: Vec<&str> = entries.iter().filter_map(|e| e.date.as_deref()).collect();
    let first = dates.iter().min().copied().unwrap_or("?");
    let last_date = dates.iter().max().copied().unwrap_or("?");
    let this_month_prefix = chrono::Local::now().format("%Y-%m").to_string();
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
    println!("date range:    {first} → {last_date}");
    println!("this month:    {this_month}");
    println!("unique tags:   {unique_tags}");
    Ok(())
}

fn export(format: ExportFormat) -> Result<()> {
    let entries = load_entries()?;
    match format {
        ExportFormat::Json => println!("{}", entries_to_json(&entries)),
    }
    Ok(())
}

fn print_where() -> Result<()> {
    let p = logbook_path();
    let abs = p.canonicalize().unwrap_or_else(|_| p.clone());
    println!("{}", abs.display());
    if !p.exists() {
        eprintln!("(file does not exist yet — run `logbook init`)");
        eprintln!("(env var: {ENV_VAR})");
    }
    Ok(())
}

fn git_add(path: &Path) -> Result<()> {
    let status = Command::new("git")
        .arg("add")
        .arg(path)
        .status()
        .map_err(|e| Error::Git(format!("failed to spawn git add: {e}")))?;
    if !status.success() {
        return Err(Error::Git(format!("git add exited with status {status}")));
    }
    Ok(())
}
