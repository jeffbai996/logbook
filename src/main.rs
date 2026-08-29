use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use logbook::{
    atomic_append, check_report_to_json, entries_to_json, entries_to_json_lines, init_file,
    is_valid_date, parse_entries, read_text, render_entry_block, resolve_logbook_path, today,
    validate_entries, Entry, Error, RenderInput, Result, ENV_VAR,
};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

macro_rules! out {
    ($($argument:tt)*) => {
        write_stdout(format_args!($($argument)*))
    };
}

macro_rules! outln {
    ($($argument:tt)*) => {
        write_stdout_line(format_args!($($argument)*))
    };
}

#[derive(Parser)]
#[command(name = "logbook", version, about = "Per-repo decision log CLI", long_about = None)]
struct Cli {
    /// When to colorize human-facing entry output
    #[arg(long, value_enum, default_value_t = ColorArg::Auto, global = true)]
    color: ColorArg,

    /// Use this file instead of discovering logbook.md (overrides LOGBOOK_FILE)
    #[arg(long, global = true, value_name = "PATH")]
    file: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Args, Clone, Default)]
struct FilterArgs {
    /// Match every supplied tag case-insensitively (repeatable)
    #[arg(long = "tag", value_name = "TAG")]
    tags: Vec<String>,
    /// Include entries on or after this date
    #[arg(long)]
    since: Option<String>,
    /// Include entries on or before this date
    #[arg(long)]
    until: Option<String>,
    /// Include only decisions not superseded by a later entry
    #[arg(long, conflicts_with = "superseded")]
    active: bool,
    /// Include only decisions superseded by a later entry
    #[arg(long)]
    superseded: bool,
    /// Stop after this many matching recent entries
    #[arg(long, value_name = "N")]
    limit: Option<NonZeroUsize>,
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
    /// Create the resolved logbook file if it doesn't exist
    Init {
        /// Also run `git add <logbook>` after creating it
        #[arg(long)]
        stage: bool,
    },

    /// Append a new entry
    Add {
        /// Short title for the entry
        title: String,
        /// The reason for the decision. If omitted, opens $EDITOR to compose it.
        #[arg(long)]
        why: Option<String>,
        /// Entry date instead of today (YYYY-MM-DD)
        #[arg(long)]
        date: Option<String>,
        /// Alternatives considered and why they were rejected
        #[arg(long)]
        rejected: Option<String>,
        /// Risk or tradeoff accepted with the decision
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
        /// Case-insensitive text search within matching entries
        #[arg(long, value_name = "TERM")]
        search: Option<String>,
        #[command(flatten)]
        filters: FilterArgs,
    },

    /// Case-insensitive search across entries
    Search {
        term: String,
        #[command(flatten)]
        filters: FilterArgs,
    },

    /// Print only the most recent entry
    Last,

    /// Print all entries from a given date (YYYY-MM-DD)
    Show {
        date: String,
        /// Match one exact title when several decisions share the date
        #[arg(long)]
        title: Option<String>,
    },

    /// Print the complete backward and forward supersession chain
    Trace {
        date: String,
        /// Exact title of the decision to trace
        #[arg(long)]
        title: Option<String>,
        /// Output format
        #[arg(long, value_enum, default_value_t = InspectionFormat::Human)]
        format: InspectionFormat,
    },

    /// Validate required fields, dates, and supersession links
    Check {
        /// Output format
        #[arg(long, value_enum, default_value_t = InspectionFormat::Human)]
        format: InspectionFormat,
    },

    /// List all distinct tags with usage counts
    Tags,

    /// Summary statistics: total entries, date range, entries this month
    Stats,

    /// Print the resolved logbook file path (honors LOGBOOK_FILE)
    Where,

    /// Generate a shell completion script
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Export entries as structured data
    Export {
        /// Output format
        #[arg(long, value_enum, default_value_t = ExportFormat::Json)]
        format: ExportFormat,
        /// Case-insensitive text search within matching entries
        #[arg(long, value_name = "TERM")]
        search: Option<String>,
        #[command(flatten)]
        filters: FilterArgs,
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
        /// Date for the new decision instead of today (YYYY-MM-DD)
        #[arg(long)]
        date: Option<String>,
        /// Alternatives considered and why they were rejected
        #[arg(long)]
        rejected: Option<String>,
        /// Risk or tradeoff accepted with the decision
        #[arg(long)]
        risk: Option<String>,
        /// One or more tags (repeatable)
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
        /// Title of the old decision (required when the date is ambiguous)
        #[arg(long, value_name = "TITLE")]
        old_title: Option<String>,
        /// Also run `git add <logbook>` after writing
        #[arg(long)]
        stage: bool,
        /// Echo the rendered entry block to stdout after writing
        #[arg(long)]
        print: bool,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ExportFormat {
    /// JSON array of entry objects
    Json,
    /// One compact JSON object per line
    Jsonl,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum InspectionFormat {
    /// Terminal-native entry or diagnostic output
    Human,
    /// Stable JSON for scripts and coding agents
    Json,
}

struct NewEntry {
    title: String,
    why: Option<String>,
    date: Option<String>,
    rejected: Option<String>,
    risk: Option<String>,
    tags: Vec<String>,
    stage: bool,
    print: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let colorize = logbook::should_colorize(
        cli.color.into(),
        std::io::IsTerminal::is_terminal(&std::io::stdout()),
        std::env::var_os("NO_COLOR").is_some(),
    );
    let result = resolve_logbook_path(cli.file.as_deref())
        .and_then(|path| dispatch(cli.cmd, &path, colorize));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Output(error)) if error.kind() == std::io::ErrorKind::BrokenPipe => {
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(std::io::stderr().lock(), "error: {e}");
            ExitCode::from(1)
        }
    }
}

fn write_stdout(arguments: std::fmt::Arguments<'_>) -> Result<()> {
    std::io::stdout()
        .lock()
        .write_fmt(arguments)
        .map_err(Error::Output)
}

fn write_stdout_line(arguments: std::fmt::Arguments<'_>) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_fmt(arguments)
        .and_then(|()| stdout.write_all(b"\n"))
        .map_err(Error::Output)
}

/// Print an entry's raw block, colorized iff `colorize`.
fn emit(raw: &str, colorize: bool) -> Result<()> {
    if colorize {
        outln!("{}\n", logbook::colorize_block(raw))
    } else {
        outln!("{raw}\n")
    }
}

fn dispatch(cmd: Cmd, path: &Path, colorize: bool) -> Result<()> {
    match cmd {
        Cmd::Init { stage } => init(path, stage),
        Cmd::Add {
            title,
            why,
            date,
            rejected,
            risk,
            tags,
            stage,
            print,
        } => add(
            path,
            NewEntry {
                title,
                why,
                date,
                rejected,
                risk,
                tags,
                stage,
                print,
            },
        ),
        Cmd::List { search, filters } => list(path, search.as_deref(), &filters, colorize, None),
        Cmd::Search { term, filters } => list(path, Some(&term), &filters, colorize, Some(&term)),
        Cmd::Last => last(path, colorize),
        Cmd::Show { date, title } => show(path, &date, title.as_deref(), colorize),
        Cmd::Trace {
            date,
            title,
            format,
        } => trace(path, &date, title.as_deref(), format, colorize),
        Cmd::Check { format } => check(path, format),
        Cmd::Tags => tags_cmd(path),
        Cmd::Stats => stats(path),
        Cmd::Where => print_where(path),
        Cmd::Completions { shell } => completions(shell),
        Cmd::Export {
            format,
            search,
            filters,
        } => export(path, format, search.as_deref(), &filters),
        Cmd::Supersede {
            old_date,
            title,
            why,
            date,
            rejected,
            risk,
            tags,
            old_title,
            stage,
            print,
        } => supersede(
            path,
            old_date,
            old_title,
            NewEntry {
                title,
                why,
                date,
                rejected,
                risk,
                tags,
                stage,
                print,
            },
        ),
    }
}

fn init(path: &Path, stage: bool) -> Result<()> {
    let created = init_file(path)?;
    if stage && created {
        git_add(path)?;
    }
    if created {
        outln!("created {}", path.display())?;
    } else {
        outln!("{} already exists, leaving it alone", path.display())?;
    }
    if stage && created {
        outln!("staged {}", path.display())?;
    }
    Ok(())
}

fn add(path: &Path, entry: NewEntry) -> Result<()> {
    let title = validated_title(entry.title)?;
    let date = validated_new_date(entry.date)?;
    let tags = normalized_tags(entry.tags)?;
    if path.exists() {
        reject_duplicate_reference(&load_entries(path)?, &date, &title)?;
    }
    let why = resolve_why(entry.why)?;

    let created = init_file(path)?;

    let block = render_entry_block(&RenderInput {
        date: &date,
        title: &title,
        why: &why,
        rejected: entry.rejected.as_deref(),
        risk: entry.risk.as_deref(),
        tags: &tags,
        supersedes: None,
    });
    atomic_append(path, &block)?;
    if entry.stage {
        git_add(path)?;
    }

    if created {
        outln!("auto-created {}", path.display())?;
    }
    outln!("added: {date} — {title}")?;

    if entry.print {
        outln!("---")?;
        out!("{block}")?;
    }

    if entry.stage {
        outln!("staged {}", path.display())?;
    }

    Ok(())
}

fn resolve_why(value: Option<String>) -> Result<String> {
    let value = match value {
        Some(value) if value == "-" => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .map_err(Error::Stdin)?;
            input
        }
        Some(value) => value,
        None => logbook::capture_via_editor()?,
    };
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(Error::EmptyEntry);
    }
    Ok(value)
}

fn validated_title(value: String) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(Error::InvalidEntry("title cannot be empty".into()));
    }
    if value.contains('\n') || value.contains('\r') {
        return Err(Error::InvalidEntry("title must fit on one line".into()));
    }
    Ok(value)
}

fn validated_new_date(value: Option<String>) -> Result<String> {
    let value = value.unwrap_or_else(today);
    validate_date_arg("date", &value)?;
    Ok(value)
}

fn normalized_tags(tags: Vec<String>) -> Result<Vec<String>> {
    let mut normalized = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            return Err(Error::InvalidEntry("tags cannot be empty".into()));
        }
        if tag.contains(',') || tag.contains('\n') || tag.contains('\r') {
            return Err(Error::InvalidEntry(
                "tags cannot contain commas or newlines".into(),
            ));
        }
        let folded = tag.to_lowercase();
        if !normalized
            .iter()
            .any(|existing: &String| existing.to_lowercase() == folded)
        {
            normalized.push(tag.to_string());
        }
    }
    Ok(normalized)
}

fn supersede(
    path: &Path,
    old_date: String,
    old_title: Option<String>,
    entry: NewEntry,
) -> Result<()> {
    validate_date_arg("old_date", &old_date)?;

    let existing = load_entries(path)?;
    let dated: Vec<&Entry> = existing
        .iter()
        .filter(|e| e.date.as_deref() == Some(&old_date))
        .collect();
    if dated.is_empty() {
        return Err(Error::SupersedeTargetMissing(old_date));
    }
    let target = match old_title.as_deref() {
        Some(wanted) => {
            let matches: Vec<&Entry> = dated
                .into_iter()
                .filter(|e| e.title.as_deref() == Some(wanted))
                .collect();
            match matches.as_slice() {
                [entry] => *entry,
                [] => {
                    return Err(Error::SupersedeTitleMissing {
                        date: old_date,
                        title: wanted.to_string(),
                    })
                }
                _ => return Err(Error::SupersedeTargetAmbiguous(old_date)),
            }
        }
        None => match dated.as_slice() {
            [entry] => *entry,
            _ => return Err(Error::SupersedeTargetAmbiguous(old_date)),
        },
    };
    let reference = match target.title.as_deref() {
        Some(old_title) => format!("{old_date} — {old_title}"),
        None => old_date,
    };
    if !target.is_active() {
        return Err(Error::SupersedeTargetInactive(reference));
    }

    let title = validated_title(entry.title)?;
    let date = validated_new_date(entry.date)?;
    let tags = normalized_tags(entry.tags)?;
    reject_duplicate_reference(&existing, &date, &title)?;
    let why = resolve_why(entry.why)?;

    let block = render_entry_block(&RenderInput {
        date: &date,
        title: &title,
        why: &why,
        rejected: entry.rejected.as_deref(),
        risk: entry.risk.as_deref(),
        tags: &tags,
        supersedes: Some(&reference),
    });
    atomic_append(path, &block)?;
    if entry.stage {
        git_add(path)?;
    }
    outln!("added: {date} — {title} (supersedes {reference})")?;

    if entry.print {
        outln!("---")?;
        out!("{block}")?;
    }

    if entry.stage {
        outln!("staged {}", path.display())?;
    }
    Ok(())
}

fn validate_date_arg(flag: &str, value: &str) -> Result<()> {
    if !is_valid_date(value) {
        return Err(Error::BadDate {
            flag: flag.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

fn reject_duplicate_reference(entries: &[Entry], date: &str, title: &str) -> Result<()> {
    let reference = format!("{date} — {title}");
    if entries
        .iter()
        .any(|entry| entry.reference().as_deref() == Some(&reference))
    {
        return Err(Error::DuplicateDecision(reference));
    }
    Ok(())
}

fn load_entries(path: &Path) -> Result<Vec<Entry>> {
    let text = read_text(path)?;
    Ok(parse_entries(&text))
}

fn list(
    path: &Path,
    search: Option<&str>,
    filters: &FilterArgs,
    colorize: bool,
    no_hit_term: Option<&str>,
) -> Result<()> {
    validate_filters(filters)?;
    let entries = load_entries(path)?;
    if entries.is_empty() {
        outln!("(no entries yet)")?;
        return Ok(());
    }
    let hits = matching_entries(&entries, search, filters, true);
    for entry in &hits {
        emit(&entry.raw, colorize)?;
    }
    if hits.is_empty() {
        match no_hit_term {
            Some(term) => outln!("no entries match \"{term}\"")?,
            None => outln!("no entries match the given filters")?,
        }
    }
    Ok(())
}

fn validate_filters(filters: &FilterArgs) -> Result<()> {
    if let Some(since) = filters.since.as_deref() {
        validate_date_arg("since", since)?;
    }
    if let Some(until) = filters.until.as_deref() {
        validate_date_arg("until", until)?;
    }
    if filters
        .since
        .as_deref()
        .zip(filters.until.as_deref())
        .is_some_and(|(since, until)| since > until)
    {
        return Err(Error::InvalidEntry(
            "--since cannot be after --until".into(),
        ));
    }
    for tag in &filters.tags {
        if tag.trim().is_empty() {
            return Err(Error::InvalidEntry("tag filters cannot be empty".into()));
        }
    }
    Ok(())
}

fn matching_entries<'a>(
    entries: &'a [Entry],
    search: Option<&str>,
    filters: &FilterArgs,
    newest_first: bool,
) -> Vec<&'a Entry> {
    let search = search.map(str::to_lowercase);
    let tags: Vec<String> = filters
        .tags
        .iter()
        .map(|tag| tag.trim().to_lowercase())
        .collect();
    let mut hits: Vec<&Entry> = entries
        .iter()
        .filter(|entry| {
            (!filters.active || entry.is_active())
                && (!filters.superseded || !entry.is_active())
                && tags
                    .iter()
                    .all(|needle| entry.tags.iter().any(|tag| tag.to_lowercase() == *needle))
                && filters
                    .since
                    .as_deref()
                    .is_none_or(|since| entry.date.as_deref().is_some_and(|date| date >= since))
                && filters
                    .until
                    .as_deref()
                    .is_none_or(|until| entry.date.as_deref().is_some_and(|date| date <= until))
                && search
                    .as_deref()
                    .is_none_or(|needle| entry.raw.to_lowercase().contains(needle))
        })
        .collect();
    if newest_first {
        hits.reverse();
        if let Some(limit) = filters.limit {
            hits.truncate(limit.get());
        }
    } else if let Some(limit) = filters.limit {
        let keep_from = hits.len().saturating_sub(limit.get());
        hits.drain(..keep_from);
    }
    hits
}

fn last(path: &Path, colorize: bool) -> Result<()> {
    let entries = load_entries(path)?;
    match entries.last() {
        Some(e) => {
            if colorize {
                outln!("{}", logbook::colorize_block(&e.raw))?;
            } else {
                outln!("{}", e.raw)?;
            }
        }
        None => outln!("(no entries yet)")?,
    }
    Ok(())
}

fn show(path: &Path, date: &str, title: Option<&str>, colorize: bool) -> Result<()> {
    validate_date_arg("date", date)?;
    let entries = load_entries(path)?;
    let mut hits = 0;
    for entry in entries.iter() {
        if entry.date.as_deref() == Some(date)
            && title.is_none_or(|title| entry.title.as_deref() == Some(title))
        {
            emit(&entry.raw, colorize)?;
            hits += 1;
        }
    }
    if hits == 0 {
        outln!("no entries on {date}")?;
    }
    Ok(())
}

fn trace(
    path: &Path,
    date: &str,
    title: Option<&str>,
    format: InspectionFormat,
    colorize: bool,
) -> Result<()> {
    validate_date_arg("date", date)?;
    let entries = load_entries(path)?;
    let selected = select_entry(&entries, date, title)?;

    let mut chain = vec![selected];
    let mut current = selected;
    while let Some(reference) = entries[current].supersedes.as_deref() {
        let parents = matching_prior(&entries, current, reference);
        match parents.as_slice() {
            [parent] => {
                chain.push(*parent);
                current = *parent;
            }
            _ => {
                return Err(Error::InvalidEntry(
                    "cannot trace an invalid supersession link; run `logbook check`".into(),
                ))
            }
        }
    }
    chain.reverse();

    current = selected;
    loop {
        let children: Vec<usize> = entries
            .iter()
            .enumerate()
            .skip(current + 1)
            .filter_map(|(index, entry)| {
                let reference = entry.supersedes.as_deref()?;
                (matching_prior(&entries, index, reference).as_slice() == [current])
                    .then_some(index)
            })
            .collect();
        match children.as_slice() {
            [] => break,
            [child] => {
                chain.push(*child);
                current = *child;
            }
            _ => {
                return Err(Error::InvalidEntry(
                    "cannot trace a branched supersession; run `logbook check`".into(),
                ))
            }
        }
    }

    match format {
        InspectionFormat::Human => {
            for index in chain {
                emit(&entries[index].raw, colorize)?;
            }
        }
        InspectionFormat::Json => {
            let selected: Vec<Entry> = chain
                .into_iter()
                .map(|index| entries[index].clone())
                .collect();
            outln!("{}", entries_to_json(&selected))?;
        }
    }
    Ok(())
}

fn select_entry(entries: &[Entry], date: &str, title: Option<&str>) -> Result<usize> {
    let matches: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            (entry.date.as_deref() == Some(date)
                && title.is_none_or(|title| entry.title.as_deref() == Some(title)))
            .then_some(index)
        })
        .collect();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(Error::InvalidEntry(format!(
            "no decision matches {date}{}",
            title.map(|title| format!(" — {title}")).unwrap_or_default()
        ))),
        _ => Err(Error::InvalidEntry(format!(
            "more than one decision matches {date}; pass --title"
        ))),
    }
}

fn matching_prior(entries: &[Entry], before: usize, reference: &str) -> Vec<usize> {
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

fn check(path: &Path, format: InspectionFormat) -> Result<()> {
    let entries = load_entries(path)?;
    let issues = validate_entries(&entries);
    if !issues.is_empty() {
        match format {
            InspectionFormat::Human => {
                let mut stderr = std::io::stderr().lock();
                for issue in &issues {
                    let _ = writeln!(stderr, "entry {}: {}", issue.entry, issue.message);
                }
            }
            InspectionFormat::Json => outln!("{}", check_report_to_json(&entries, &issues))?,
        }
        return Err(Error::CheckFailed(issues.len()));
    }
    let active = entries.iter().filter(|entry| entry.is_active()).count();
    match format {
        InspectionFormat::Human => outln!(
            "ok: {} entries ({active} active, {} superseded)",
            entries.len(),
            entries.len() - active
        )?,
        InspectionFormat::Json => outln!("{}", check_report_to_json(&entries, &issues))?,
    }
    Ok(())
}

fn completions(shell: Shell) -> Result<()> {
    let mut command = Cli::command();
    let mut output = Vec::new();
    clap_complete::generate(shell, &mut command, "logbook", &mut output);
    std::io::stdout()
        .lock()
        .write_all(&output)
        .map_err(Error::Output)
}

fn tags_cmd(path: &Path) -> Result<()> {
    let entries = load_entries(path)?;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &entries {
        for t in &entry.tags {
            *counts.entry(t.to_lowercase()).or_insert(0) += 1;
        }
    }
    if counts.is_empty() {
        outln!("(no tags yet — add entries with --tag <name>)")?;
        return Ok(());
    }
    let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let max_name = rows.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    for (name, count) in rows {
        outln!("{name:<max_name$}  {count}")?;
    }
    Ok(())
}

fn stats(path: &Path) -> Result<()> {
    let entries = load_entries(path)?;
    let total = entries.len();
    if total == 0 {
        outln!("(no entries yet)")?;
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
    let active = entries.iter().filter(|entry| entry.is_active()).count();

    outln!("total entries: {total}")?;
    outln!("active:        {active}")?;
    outln!("superseded:    {}", total - active)?;
    outln!("date range:    {first} → {last_date}")?;
    outln!("this month:    {this_month}")?;
    outln!("unique tags:   {unique_tags}")?;
    Ok(())
}

fn export(
    path: &Path,
    format: ExportFormat,
    search: Option<&str>,
    filters: &FilterArgs,
) -> Result<()> {
    validate_filters(filters)?;
    let entries = load_entries(path)?;
    let selected: Vec<Entry> = matching_entries(&entries, search, filters, false)
        .into_iter()
        .cloned()
        .collect();
    match format {
        ExportFormat::Json => outln!("{}", entries_to_json(&selected))?,
        ExportFormat::Jsonl => {
            let output = entries_to_json_lines(&selected);
            if !output.is_empty() {
                outln!("{output}")?;
            }
        }
    }
    Ok(())
}

fn print_where(path: &Path) -> Result<()> {
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    outln!("{}", abs.display())?;
    if !path.exists() {
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "(file does not exist yet — run `logbook init`)");
        let _ = writeln!(stderr, "(env var: {ENV_VAR})");
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
