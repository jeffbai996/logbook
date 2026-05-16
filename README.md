# logbook

A tiny CLI for keeping a per-repo decision log. Drops a single `logbook.md` file at the root of any project and gives you one command to append a structured entry to it.

The metaphor is a ship's logbook: append-only, single source of truth, lives with the vessel.

## Why

The code shows *what* you did. Commit messages say *what changed*. Neither captures *why you chose this design over the alternatives* — and that's the context you lose first when you come back to a project after a month.

The fix is dumb on purpose: a markdown file in the repo, committed in git, with a small CLI to append entries to it. No service, no database, no editor plugin, no SaaS.

This works whether you code solo, with a team, or alongside an LLM agent. (For agents specifically: the file is plain text in the repo, so `cat logbook.md` at session start gives the agent every architectural decision it needs to bootstrap context.)

## Not a substitute for

- **README** — what the project does, for users
- **Commit messages** — what changed, per change
- **Design docs** — for decisions that need diagrams, prose, or stakeholder review

`logbook` fills the gap *between* commits and design docs: the architectural choices the code itself can't explain.

## Install

```bash
cargo install logbook
```

Or from source:

```bash
git clone https://github.com/jeffbai996/logbook.git
cd logbook
cargo install --path .
```

Requires [Rust](https://rustup.rs) 1.75+.

## Quickstart

```bash
cd ~/your-project
logbook init
logbook add "switched from polling to websocket" \
  --why "polling at 1s was hammering the upstream API and getting throttled" \
  --rejected "redis pub/sub (overkill), SSE (no bidirectional need)" \
  --risk "websocket drops need reconnect logic — added exp backoff" \
  --tag refactor --tag perf \
  --stage
```

This appends a block to `./logbook.md` and stages it for the next commit:

```markdown
## 2026-05-15 — switched from polling to websocket
**why:** polling at 1s was hammering the upstream API and getting throttled
**rejected:** redis pub/sub (overkill), SSE (no bidirectional need)
**risk:** websocket drops need reconnect logic — added exp backoff
**tags:** refactor, perf
```

## Commands

| Command | What it does |
|---|---|
| `logbook init` | Create the logbook file with a header. Idempotent. |
| `logbook add <title> --why <reason> [--rejected …] [--risk …] [--tag X]… [--stage] [--print]` | Append a new entry. `--tag` is repeatable. `--stage` runs `git add`. `--print` echoes the rendered block. |
| `logbook list [--tag X] [--since YYYY-MM-DD] [--until YYYY-MM-DD]` | Print entries newest-first with optional filters (all combinable). |
| `logbook last` | Print the most recent entry only. |
| `logbook show <YYYY-MM-DD>` | Print every entry from a specific date. |
| `logbook search <term>` | Case-insensitive substring search across all entries. |
| `logbook tags` | List all distinct tags with usage counts. |
| `logbook stats` | Total entries, date range, entries-this-month, unique tags. |
| `logbook where` | Print the resolved logbook file path (honors `LOGBOOK_FILE`). |

Run `logbook --help` or `logbook <cmd> --help` for full flag reference.

### Environment

- `LOGBOOK_FILE` — override the default `./logbook.md` with any path. Useful for monorepos (`LOGBOOK_FILE=docs/decisions.md`) or for keeping a personal log outside the working directory.

## Format

Single markdown file, `logbook.md`, at the project root, append-only. Each entry:

```markdown
## YYYY-MM-DD — <title>
**why:** <reason this was chosen>
**rejected:** <alternatives considered and why not>
**risk:** <what could go wrong>
**tags:** <comma-separated tags>
```

Only the title and `--why` are required. `--rejected`, `--risk`, and `--tag` are optional but recommended for non-trivial decisions.

Because the format is plain markdown with stable headings, you can hand-edit `logbook.md` directly if you ever need to. The CLI only ever appends — it won't rewrite what's already there.

## Philosophy

- **One file, in the repo.** No external dependencies, no service to run.
- **Append-only.** Never edit old entries. If a decision is reversed, write a new entry that supersedes it.
- **Few fields.** Why, rejected, risk, tags. If you need more structure, you need a design doc.
- **45 seconds per entry.** If it takes longer, the tool is wrong.
- **Markdown over JSON.** A logbook is for humans first, machines second. Markdown reads well in `cat`, in GitHub, in your editor, in `less`, and in an LLM's context window.

## Use with LLM agents

A common workflow: have your agent (Claude Code, Cursor, Aider, etc.) read `logbook.md` at the start of every session so it inherits your accumulated decisions. Example for Claude Code, in `CLAUDE.md`:

```markdown
At session start, run: `logbook list | head -100`
Treat every entry as an architectural constraint unless explicitly superseded.
When you make a non-obvious choice, suggest a `logbook add` command for the user to run.
```

This makes the logbook the agent's long-term memory for the project, with zero infrastructure beyond the file itself.

## Roadmap

**0.1.0 — testing & polish** ✅ *current*
- ~~Test suite~~ ✅ 22 unit tests + 19 integration tests (`cargo test`)
- ~~Better error messages~~ ✅ typed `Error` enum with `NotFound`, `BadDate`, `Io`, `Git` variants
- ~~Atomic writes to survive concurrent invocations~~ ✅ shipped in 0.0.3
- ~~Honor `LOGBOOK_FILE` env var to allow custom filenames~~ ✅ shipped in 0.0.3
- CI on push and PR: build/test on Linux/macOS/Windows + `cargo fmt --check` + `cargo clippy -D warnings`

**0.2.0 — distribution**
- Publish to crates.io so `cargo install logbook` actually works
- Prebuilt binaries via GitHub Releases for macOS, Linux, Windows (no Rust toolchain required to install)
- Homebrew tap

**0.3.0 — ergonomics**
- `logbook add` opens `$EDITOR` when `--why` is omitted (git-commit style)
- `logbook supersede <date> "new title" --why ...` — formal supersession syntax that links the new entry to the old one
- Colored TTY output (off when piped)
- `logbook export --format json` for tooling integrations

**Maybe-someday**
- Shell completion (`logbook completions bash`)
- A read-only web viewer that renders `logbook.md` as a timeline
- Squad/team mode: aggregate logbooks across multiple repos for retro reviews

Not on the roadmap: editing past entries, deleting entries, server-mode, GUI, plugins. Scope creep is the enemy.

## Development

```bash
git clone https://github.com/jeffbai996/logbook.git
cd logbook
cargo build              # debug build
cargo test               # full test suite (unit + integration)
cargo fmt --all          # format
cargo clippy --all-targets -- -D warnings
```

The codebase is laid out as a library + binary:

- `src/lib.rs` — public re-exports, `logbook_path()`, `today()`, `is_date_shaped()`
- `src/error.rs` — typed `Error` enum
- `src/parse.rs` — pure markdown → `Vec<Entry>` parser
- `src/store.rs` — file I/O, including atomic-append
- `src/main.rs` — `clap` CLI definitions + dispatch (thin)
- `tests/cli.rs` — integration tests that spawn the real binary against a tempdir

## Status

`0.1.0` — first version meant to be depended on. 41 tests, CI on three OSes, typed errors. Not yet published to crates.io (build from source for now — `cargo install --path .` from a clone).

## License

MIT
