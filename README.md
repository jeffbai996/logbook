# logbook

A CLI that records repository decisions in one append-only `logbook.md`.

One repository. One file. Append-only. Local CLI. Git-native. No service,
database, account or project scaffolding. No GUI, daemon, telemetry, LLM
integration or sync layer; those belong in other tools.

Write an entry for a non-obvious choice that affects future work and whose
reason is not clear from the diff. Skip routine changes, status updates and
things the code says better. Do not backfill a project diary: start with the
next decision you would otherwise have to reverse-engineer.

## Install

Rust 1.85 or newer:

```bash
cargo install logbook --locked
```

Prebuilt Linux, macOS and Windows archives and `SHA256SUMS` are attached to the
[latest GitHub release](https://github.com/jeffbai996/logbook/releases/latest).

## Usage

```bash
logbook add "store tokens in the OS keychain" \
  --why "the OS provides encryption and access control" \
  --rejected "an app-specific file needs another key" \
  --risk "headless environments need a separate strategy" \
  --tag security --stage --print
logbook last
logbook search keychain
logbook supersede 2026-08-01 "move state to Postgres" --why "write contention is now measurable"
logbook trace 2026-08-01 --title "use SQLite for local state"
logbook export --active --limit 10
logbook check --format json
```

`add` creates the file when needed. `--stage` runs `git add` on it; logbook
never commits. If staging fails, the decision stays saved and the error prints
the recovery command.

Only the title and the reason are required; rejected alternatives, risks and
tags are optional.

`--why` takes text, `-` for stdin, or is omitted to open `$EDITOR` (`$VISUAL`
when `$EDITOR` is unset). In the editor, lines whose first non-space character
is `#` are omitted. `--date YYYY-MM-DD` is for importing earlier decisions.

| Command | Purpose |
|---|---|
| `init [--stage]` | Create an empty logbook without overwriting one. |
| `add <title> [options]` | Append a decision from flags, stdin, or an editor. |
| `list [filters]` | Print matching decisions newest-first. |
| `search <term> [filters]` | Search all entry text case-insensitively. |
| `last` | Print the newest decision. |
| `show <date> [--title T]` | Print decisions on a date. |
| `supersede <date> <title> [options]` | Append a replacement decision. |
| `trace <date> [--title T] [--format human\|json]` | Print one complete decision chain. |
| `check [--format human\|json]` | Validate the file and supersession graph. |
| `export [--format json\|jsonl] [filters]` | Emit stable machine-readable records. |
| `tags` | List normalized tags and counts. |
| `stats` | Summarize decisions, dates, states, and tags. |
| `where` | Print the resolved logbook path. |
| `completions <shell>` | Generate a shell completion script. |

`logbook <command> --help` lists every option.

`list`, `search` and `export` share date, tag, active/superseded and limit
filters; repeated tags mean AND. `list` and `search` return the newest matches
first; `show` keeps file order; `trace` keeps chain order; `export` keeps
document order.

## Use with coding agents

Give an agent a small current slice, not the whole log:
`logbook list --active --limit 10`, `logbook export --active --limit 10`,
`logbook check --format json`. There is no special agent integration: the CLI
emits bounded JSON and the source of truth is a file any tool can read.

Suggested repository instructions:

```markdown
Before product, architecture, or operational work, run
`logbook list --active --limit 10` and treat those decisions as constraints.
Record only new non-obvious decisions. Supersede reversals instead of rewriting
earlier entries.
```

## Configuration

- Path: the nearest `logbook.md` from the current directory up to the
  repository root. `--file PATH` or `LOGBOOK_FILE` override it.
- Editor: `$EDITOR`, then `$VISUAL`.

## File format

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

A decision has one successor. `supersede` appends the replacement and leaves
the old entry intact. `check` reports malformed entries and broken, ambiguous,
duplicate or branched links.

Hand edits to `logbook.md` are possible; `check` catches malformed ones.

Writes use an atomic replacement and a cross-process lock.

- In multiline fields passed by flag or stdin, indent any line beginning with
  `## ` or a field label such as `**risk:**`; unindented ones are rejected.
- A `logbook.md` symlink is refused; pass `--file` with the target path.
- A stale `<logbook path>.lock` directory left by a crash: confirm no writer is
  running, remove it, retry.

## Development

```bash
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

See [CONTRIBUTING.md](CONTRIBUTING.md). Licence: [MIT](LICENSE).
