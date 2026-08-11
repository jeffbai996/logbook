# logbook

[![crates.io](https://img.shields.io/crates/v/logbook.svg)](https://crates.io/crates/logbook)
[![docs.rs](https://docs.rs/logbook/badge.svg)](https://docs.rs/logbook)
[![CI](https://github.com/jeffbai996/logbook/actions/workflows/ci.yml/badge.svg)](https://github.com/jeffbai996/logbook/actions/workflows/ci.yml)

`logbook` is a tiny CLI for keeping a decision log in each repository.

Code tells you what it does. Git tells you what changed. Logbook tells you why.

Each repository gets one append-only `logbook.md`: no service, database,
account, plugin system, or generated project structure. It sits between terse
commit messages and full architecture decision records.

## Install

With Rust 1.85 or newer:

```bash
cargo install logbook
```

Prebuilt archives for Linux, macOS, and Windows are available from the
[latest GitHub release](https://github.com/jeffbai996/logbook/releases/latest).
The release includes x86_64 and ARM64 Linux/macOS builds, a static musl Linux
build, and an x86_64 Windows build.

From a checkout:

```bash
git clone https://github.com/jeffbai996/logbook.git
cd logbook
cargo install --path .
```

## 60-second example

Run this inside a repository:

```bash
logbook init
logbook add "use SQLite for local state" \
  --why "it keeps installation to one binary and one data file" \
  --rejected "Postgres requires a service for a single-user tool" \
  --risk "write concurrency is limited" \
  --tag storage \
  --stage

logbook last
logbook search SQLite
```

`add` also creates `logbook.md` if `init` was skipped. `--stage` runs
`git add logbook.md`; logbook never commits for you.

Omit `--why` to write the reason in `$EDITOR` (or `$VISUAL`):

```bash
logbook add "keep the parser line-oriented"
```

## Core commands

| Command | Purpose |
|---|---|
| `logbook init` | Create `logbook.md` if it does not exist. |
| `logbook add <title> [options]` | Append a decision with `--why`, `--rejected`, `--risk`, repeatable `--tag`, optional `--stage`, and optional `--print`. |
| `logbook list [--tag X] [--since DATE] [--until DATE] [--limit N]` | Print matching entries newest first. |
| `logbook last` | Print the newest entry. |
| `logbook show <DATE>` | Print entries recorded on a date. |
| `logbook search <term>` | Search all entry text, case-insensitively. |
| `logbook supersede <DATE> <new-title> [options]` | Append a decision that replaces an earlier one. |
| `logbook export [--format json] [--limit N]` | Emit stable JSON, optionally limited to recent entries. |
| `logbook tags` | List tags with counts. |
| `logbook stats` | Show entry, date-range, monthly, and tag counts. |
| `logbook where` | Print the resolved logbook path. |

Use `logbook <command> --help` for the full flags. Human-facing reads honor
`--color auto|always|never` and `NO_COLOR`; piped output and JSON never gain
automatic ANSI escapes.

Set `LOGBOOK_FILE` to use a different relative or absolute path, such as
`docs/decisions.md` in a monorepo.

## Superseding a decision

Supersession appends; it never edits the old entry:

```bash
logbook supersede 2026-08-01 "move state to Postgres" \
  --why "write contention is now measurable" \
  --old-title "use SQLite for local state" \
  --risk "adds an operational dependency"
```

`--old-title` is optional when only one entry exists on the old date. When
several entries share that date, logbook requires the title rather than writing
an ambiguous reference. The new entry records both the date and old title, so
`show` and `search` trace the decision in either direction.

## Agent usage

Agents usually need a bounded recent view, not the whole file:

```bash
logbook list --limit 10
logbook export --limit 10
```

The first is readable Markdown; the second is machine-readable JSON. A useful
repository instruction is:

```markdown
At the start of work, read `logbook list --limit 10`. Treat recorded decisions
as constraints unless a later entry supersedes them. Record only non-obvious
product or architecture choices.
```

No agent integration is required. An agent can also read `logbook.md` directly.

## Format and philosophy

Entries are ordinary Markdown:

```markdown
## 2026-08-10 — use SQLite for local state
**why:** it keeps installation to one binary and one data file
**rejected:** Postgres requires a service for a single-user tool
**risk:** write concurrency is limited
**tags:** storage

## 2026-08-20 — move state to Postgres
**why:** write contention is now measurable
**supersedes:** 2026-08-10 — use SQLite for local state
**risk:** adds an operational dependency
**tags:** storage
```

Only the title and `why` are required. The CLI appends canonical entries and
preserves existing text. Hand edits remain possible because Markdown is the
source of truth, but changed decisions should normally be superseded rather
than rewritten.

The scope is deliberately fixed: one repository, one file, local CLI, git-native.
Logbook will not grow a GUI, hosted service, database, sync layer, plugin system,
semantic search, daemon, telemetry, or automatic decision extraction.

## Development

```bash
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
```

CI runs tests on Linux, macOS, and Windows and separately verifies the Rust
1.85 MSRV. Releases package six prebuilt archives from version tags.

Contributions should preserve the product boundary; see
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)
