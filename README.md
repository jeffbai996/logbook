# logbook

[![crates.io](https://img.shields.io/crates/v/logbook.svg)](https://crates.io/crates/logbook)
[![docs.rs](https://docs.rs/logbook/badge.svg)](https://docs.rs/logbook)
[![CI](https://github.com/jeffbai996/logbook/actions/workflows/ci.yml/badge.svg)](https://github.com/jeffbai996/logbook/actions/workflows/ci.yml)

`logbook` is a small CLI for keeping a decision log in each repository.

Code tells you what it does. Git tells you what changed. Logbook tells you why.

Each repository gets one append-only `logbook.md`. It records why a product or
architecture decision was made, which alternatives lost, what risk was
accepted, and when a later decision superseded it. There is no service,
database, account, generated project tree, or integration framework.

## Install

With Rust 1.85 or newer:

```bash
cargo install logbook --locked
```

Six prebuilt Linux, macOS, and Windows archives plus `SHA256SUMS` are attached
to the [latest GitHub release](https://github.com/jeffbai996/logbook/releases/latest).

From a checkout:

```bash
git clone https://github.com/jeffbai996/logbook.git
cd logbook
cargo install --path . --locked
```

## 60-second example

Run inside a repository:

```bash
logbook init --stage

logbook add "use SQLite for local state" \
  --why "it keeps installation to one binary and one data file" \
  --rejected "Postgres requires a service for a single-user tool" \
  --risk "write concurrency is limited" \
  --tag storage \
  --stage

logbook last
logbook search SQLite
logbook check
```

`--stage` runs `git add` for the logbook. Logbook never commits automatically.
`add` creates the file when needed, so `init` is optional.

## Everyday use

Logbook discovers the nearest `logbook.md` from any nested directory, stopping
at the current repository root:

```bash
cd src/deep/in/the/weeds
logbook list --active --limit 10
logbook where
```

Use `--file PATH` for an explicit file; it takes precedence over
`LOGBOOK_FILE`. Relative paths are resolved from the current directory:

```bash
logbook --file docs/decisions.md list
```

Omit `--why` to compose the reason in `$EDITOR` or `$VISUAL`. Use `--why -`
for stdin and `--date` when importing an older decision:

```bash
logbook add "keep the API synchronous"

printf '%s\n' "A daemon adds lifecycle work without improving local writes." |
  logbook add "avoid a background worker" --why - --tag architecture

logbook add "adopt the existing storage format" \
  --why "records a decision made during the migration" \
  --date 2026-08-01
```

Reasons, rejected alternatives, and risks may span multiple lines. Titles and
tags stay single-line so the Markdown remains unambiguous.

## Retrieval

`list`, `search`, and `export` share date, tag, active-state, and limit filters.
Repeated tags are combined with AND:

```bash
logbook list --tag storage --tag local --since 2026-01-01
logbook list --search "write contention" --active --limit 20
logbook search SQLite --tag storage --active

logbook export --active --limit 10
logbook export --format jsonl --tag storage
```

Human output is newest-first. JSON and JSON Lines remain in document order and
include the source fields plus derived `active` and `superseded_by` state.
Machine output is never colorized. Human output honors
`--color auto|always|never` and `NO_COLOR`.

## Supersession

Changing a decision appends a new entry; it never edits the old one. This
concrete example starts the chain with an imported decision:

```bash
logbook add "use SQLite for local state" \
  --why "it keeps installation to one binary and one data file" \
  --date 2026-08-01

logbook supersede 2026-08-01 "move state to Postgres" \
  --old-title "use SQLite for local state" \
  --why "write contention is now measurable" \
  --risk "adds an operational dependency" \
  --stage
```

`--old-title` is optional when only one entry exists on the date. When several
entries share it, the exact title is required. A decision can have only one
successor; supersede the current decision instead of branching an old one.
Inspect a reversal in either direction with:

```bash
logbook trace 2026-08-01 --title "use SQLite for local state"
```

`list --active` excludes decisions that a valid later entry supersedes.
`logbook check` exits nonzero for malformed dates, missing required fields,
broken or ambiguous links, duplicate references, and branched supersessions;
it is suitable for CI.

## Commands

| Command | Purpose |
|---|---|
| `init [--stage]` | Create the resolved logbook without overwriting one. |
| `add <title> [options]` | Append a decision from flags, stdin, or an editor. |
| `list [filters]` | Print matching decisions newest-first. |
| `search <term> [filters]` | Search entry text case-insensitively. |
| `last` | Print the newest decision. |
| `show <date> [--title T]` | Print decisions on a date. |
| `supersede <date> <title> [options]` | Append a replacement decision. |
| `trace <date> [--title T]` | Print a complete supersession chain. |
| `check` | Validate the file and its decision graph. |
| `export [--format json|jsonl] [filters]` | Emit stable machine-readable records. |
| `tags` | List normalized tags and counts. |
| `stats` | Summarize total, active, superseded, date, and tag counts. |
| `where` | Print the resolved path. |

Use `logbook <command> --help` for every option.

## Agent usage

Agents need bounded current context, not a ceremonial document dump:

```bash
logbook list --active --limit 10
logbook export --active --limit 10
```

A useful repository instruction is:

```markdown
Before product, architecture, or operational work, read
`logbook list --active --limit 10`. Treat those decisions as constraints.
Record only new non-obvious decisions, and supersede reversals instead of
rewriting earlier entries.
```

No agent integration is required. Reading `logbook.md` directly also works.

## Format and philosophy

The source of truth is ordinary Markdown:

```markdown
## 2026-08-01 — use SQLite for local state
**why:** it keeps installation to one binary and one data file
**rejected:** Postgres requires a service for a single-user tool
**risk:** write concurrency is limited
**tags:** storage

## 2026-08-20 — move state to Postgres
**why:** write contention is now measurable
**supersedes:** 2026-08-01 — use SQLite for local state
**risk:** adds an operational dependency
**tags:** storage
```

Only the dated title and `why` are required. Hand edits remain possible, but
changed decisions should normally be superseded rather than rewritten. Writes
use an adjacent atomic replacement and a short-lived cross-process lock, so a
crash cannot leave a partial entry and concurrent writers do not lose one.

Version 0.5 is feature-complete for the intended product. The project is in
maintenance mode: bug fixes, portability work, and dependency upkeep remain
welcome; no broader feature roadmap is planned.

The boundary is fixed: one repository, one file, append-only, local CLI,
git-native. Logbook will not grow a GUI, hosted service, database, sync layer,
plugin system, semantic search, daemon, telemetry, LLM integration, or
automatic decision extraction.

## Development

```bash
cargo test --locked
cargo +1.85.0 test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
cargo package --locked
```

CI tests Linux, macOS, Windows, and the Rust 1.85 MSRV. Tagged releases build
six archives and publish one checksum file. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)
