# logbook

[![crates.io](https://img.shields.io/crates/v/logbook.svg)](https://crates.io/crates/logbook)
[![docs.rs](https://docs.rs/logbook/badge.svg)](https://docs.rs/logbook)
[![CI](https://github.com/jeffbai996/logbook/actions/workflows/ci.yml/badge.svg)](https://github.com/jeffbai996/logbook/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

> A tiny CLI that gives every repo a single `logbook.md` for "why I made this decision and what I rejected" — the architectural context that's currently scattered across your head, half-written PR descriptions, and Slack threads.

```bash
$ logbook add "switched ORM to raw SQL" \
    --why "ORM was generating 14-join queries for 3-table lookups" \
    --rejected "ORM query hints (still magic), custom resolver (too much code)" \
    --risk "lose auto-migrations — added manual scripts in db/migrations/" \
    --tag perf --tag db --stage

added: 2026-05-16 — switched ORM to raw SQL
staged logbook.md
```

That command appends a structured entry to `./logbook.md` and stages it for your next commit. Three months later, when you've forgotten why the codebase looks the way it does, `logbook last` or `logbook search orm` brings it back.

---

## Table of contents

- [Why this exists](#why-this-exists)
- [What it isn't](#what-it-isnt)
- [Install](#install)
- [Quickstart](#quickstart)
- [Commands](#commands)
- [Entry format](#entry-format)
- [Using logbook with LLM coding agents](#using-logbook-with-llm-coding-agents)
- [Comparison to alternatives](#comparison-to-alternatives)
- [FAQ](#faq)
- [Development](#development)
- [Roadmap](#roadmap)
- [License](#license)

## Why this exists

The code tells you *what* it does. `git log` tells you *what changed*. Neither tells you **why you picked this design over the alternatives** — and that's the context you lose first when you come back to a project after a month away.

For solo developers this is annoying. For developers working alongside an LLM coding agent (Claude Code, Cursor, Aider, …) it's worse: the agent loses state every session, and you become the human glue carrying architectural decisions in your head. `logbook.md` lives in the repo, so the agent can `cat` it at session start and inherit every decision you've made.

The fix is intentionally dumb: a single markdown file in the repo, committed in git, with a small CLI to append entries to it. No service, no database, no editor plugin, no SaaS.

## What it isn't

- **Not a README.** READMEs explain what the project does, for users.
- **Not a CHANGELOG.** CHANGELOGs are for end-users tracking what shipped.
- **Not a commit message.** Commits say what changed in *this* diff.
- **Not a full ADR (Architecture Decision Record) framework.** ADRs are great for teams with formal review processes. `logbook` is what you reach for when ADRs feel like too much ceremony.
- **Not a design doc.** When you need diagrams, prose, or stakeholder review, write a design doc.

`logbook` fills the gap *between* commit messages and design docs: the architectural choices the code itself can't justify.

## Install

**macOS / Linux (Homebrew):**

```bash
brew install jeffbai996/tap/logbook
```

**Any platform with Rust:**

```bash
cargo install logbook
```

**Prebuilt binary (no toolchain required):**

Grab the archive for your platform from the [latest release](https://github.com/jeffbai996/logbook/releases/latest), extract it, and drop the binary on your `$PATH`. Prebuilt targets: `x86_64-linux` (glibc), `x86_64-linux-musl` (static — Alpine, scratch containers), `aarch64-linux`, `x86_64-macos`, `aarch64-macos` (Apple Silicon), `x86_64-windows`.

**From source:**

```bash
git clone https://github.com/jeffbai996/logbook.git
cd logbook
cargo install --path .
```

Requires [Rust](https://rustup.rs) 1.75 or newer. After installing, `logbook` is on your `$PATH`:

```bash
$ logbook --version
logbook 0.2.1
```

## Quickstart

```bash
cd ~/your-project
logbook init                 # one-time per repo
logbook add "switched to websocket" \
  --why  "polling was getting rate-limited" \
  --tag  perf \
  --stage                    # also runs `git add logbook.md`
```

That's it. Future entries are the same shape. The four common workflows:

| To do this | Run |
|---|---|
| Record a decision while you're making it | `logbook add "title" --why "..." [--rejected ...] [--risk ...] [--tag X] --stage` |
| Look up what you decided recently | `logbook last` or `logbook list \| head -50` |
| Find a specific past decision | `logbook search <term>` |
| Recall what you decided on a date | `logbook show 2026-05-16` |

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

Run `logbook --help` or `logbook <cmd> --help` for the full flag reference.

### Environment

| Variable | Effect |
|---|---|
| `LOGBOOK_FILE` | Override the default `./logbook.md`. Useful for monorepos (`LOGBOOK_FILE=docs/decisions.md`), or for keeping a personal log in a fixed location across projects. |

## Entry format

`logbook.md` is plain markdown. The CLI only ever appends — it never rewrites old content. You can edit the file by hand if you want.

Each entry follows this shape:

```markdown
## YYYY-MM-DD — <title>
**why:** <reason this was chosen>
**rejected:** <alternatives considered and why not>
**risk:** <what could go wrong>
**tags:** <comma-separated tags>
```

Only the title and `--why` are required. `--rejected`, `--risk`, and `--tag` are optional but recommended for non-trivial decisions.

A real entry looks like:

```markdown
## 2026-05-16 — atomic writes via tempfile + rename
**why:** crashes during write could leave logbook.md half-written; rename(2) is atomic on POSIX so write-then-rename guarantees the file is either fully old or fully new, never partial
**rejected:** fsync on every write (overkill for human-pace writes); fcntl file locking (overkill — we don't have multiple writers fighting)
**risk:** reads entire file into memory before rewriting — fine until logbooks have millions of entries
**tags:** robustness, io
```

## Using logbook with LLM coding agents

A common workflow: have your agent (Claude Code, Cursor, Aider, etc.) read `logbook.md` at the start of every session so it inherits accumulated decisions.

For [Claude Code](https://docs.anthropic.com/claude/docs/claude-code), add to your `CLAUDE.md`:

```markdown
At session start, run: `logbook list | head -100`
Treat every entry as an architectural constraint unless explicitly superseded.
When you make a non-obvious choice, suggest a `logbook add` command for the user to run.
```

For [Aider](https://aider.chat), add `logbook.md` to your `.aider.conf.yml` `read`-only file list. For [Cursor](https://www.cursor.com), reference it in `.cursorrules`.

This turns the logbook into the agent's long-term memory for the project, with zero extra infrastructure.

## Comparison to alternatives

| Approach | Strength | Weakness vs `logbook` |
|---|---|---|
| **CHANGELOG.md** | End-user-facing, semver-aligned | Doesn't capture *rejected* alternatives or risks; written for outsiders, not the author |
| **`docs/adr/*.md` (full ADRs)** | Battle-tested by enterprises, lots of tooling | Heavyweight — one file per decision, formal status workflow, real overhead. `logbook` is the lite version. |
| **PR descriptions** | Co-located with the diff, contextual | Lost when PRs get merged and you can't find them again; not greppable from the working tree |
| **Slack/Notion/Confluence** | Searchable, supports rich content | Decoupled from the repo, requires login, the link rots, the agent can't read it |
| **Code comments** | Right next to the code | No place for *rejected* alternatives or cross-file decisions; rot pressure |
| **Mental model + memory** | Free | Lossy, doesn't transfer to teammates or to your future self |

The honest take: if your team already runs full ADRs and likes them, keep doing that. `logbook` exists for the much larger group of developers (and solo developers) for whom ADRs are too much.

## FAQ

**Why markdown over JSON/YAML?**
A logbook is for humans first, machines second. Markdown renders well in `cat`, on GitHub, in `less`, in your editor, and inside an LLM's context window. The parser extracts the few structured bits we need (date, tags); the rest is intentionally free-form.

**Can I edit `logbook.md` by hand?**
Yes — the CLI never rewrites old content. As long as you keep the `## YYYY-MM-DD — title` heading shape, the parser will continue to extract the date and tags correctly.

**What happens if I run `logbook add` from two terminals simultaneously?**
Each `add` reads the file, appends in memory, writes to a sibling tempfile, then renames on top. The rename is atomic on POSIX and Windows. Worst case, one of the two writes is lost (last-writer-wins); the file is never corrupted.

**Why isn't there an `--edit` flag to fix typos?**
Append-only is a deliberate philosophy. If a decision is reversed or refined, write a new entry that supersedes the old one — the history of *how thinking changed* is part of the value. If you really need to fix a typo, edit `logbook.md` directly.

**Does it work without git?**
Yes. The `--stage` flag invokes `git add` for convenience but the rest of the tool doesn't care. You can use `logbook` in a directory that isn't a git repo at all.

**Why a custom file format instead of, say, conventional commits?**
Conventional commits live in git history, which means they're harder to read all-at-once and require git tooling to query. `logbook.md` is one greppable file you can `cat` from anywhere — including from inside an LLM session.

**Is this overengineered?**
Honestly, no — it's the opposite. It's a single binary, three runtime dependencies (`clap`, `chrono`, `thiserror`), and a markdown file. The hard part wasn't writing it; the hard part was deciding *not* to add features (editor mode, server mode, plugins, syncing, etc.). See the roadmap's "Not on the roadmap" section.

**What's the binary size?**
About 1.2 MB on Linux, stripped. Starts in <5 ms. Uses ~1 MB RAM. You can run it from a git pre-commit hook without noticing.

## Development

```bash
git clone https://github.com/jeffbai996/logbook.git
cd logbook
cargo build              # debug build
cargo test               # full test suite (unit + integration + property + doctest)
cargo doc --open         # browse the rustdoc-rendered API docs
cargo fmt --all          # format
cargo clippy --all-targets -- -D warnings
```

Snapshot tests use [`insta`](https://insta.rs). If you intentionally change the rendered entry format, install `cargo-insta` (`cargo install cargo-insta`) and run `cargo insta review` to accept the new snapshots.

### Layout

The codebase is a library plus a binary, so the test suite can exercise the parser and IO directly without shelling out:

| Path | Purpose |
|---|---|
| `src/lib.rs` | Public re-exports, path/date helpers, constants |
| `src/error.rs` | Typed `Error` enum (`NotFound`, `BadDate`, `Io`, `Git`) |
| `src/parse.rs` | Pure markdown → `Vec<Entry>` parser (no I/O) |
| `src/store.rs` | File I/O including `atomic_append` |
| `src/main.rs` | `clap` CLI definitions and dispatch (thin) |
| `tests/cli.rs` | End-to-end tests via `assert_cmd` against tempdir sandboxes |
| `tests/property.rs` | `proptest` round-trip + `insta` snapshot tests |
| `tests/snapshots/` | Frozen output snapshots reviewed via `cargo insta review` |

### CI

GitHub Actions runs on every push to `main` and on every PR:

- **Build + test** on ubuntu-latest, macos-latest, and windows-latest (matrix).
- **Lint**: `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings`.

A push is only green when all three OSes build, all 56 tests pass, the code is rustfmt-clean, and clippy finds zero warnings.

## Roadmap

**0.1.x — testing & polish** ✅
- ~~Test suite~~ ✅ 56 tests across 4 categories
- ~~Better error messages~~ ✅ typed `Error` enum
- ~~Atomic writes~~ ✅ shipped in 0.0.3
- ~~`LOGBOOK_FILE` env var~~ ✅ shipped in 0.0.3
- ~~CI on three OSes~~ ✅
- ~~Property + snapshot tests~~ ✅ shipped in 0.1.1
- ~~Full rustdoc coverage~~ ✅ shipped in 0.1.1

**0.2.x — distribution** ✅ *current*
- ~~Publish to crates.io so `cargo install logbook` works~~ ✅ shipped in 0.2.0
- ~~Prebuilt binaries via GitHub Releases for macOS, Linux, Windows~~ ✅ shipped in 0.2.0 (5 targets)
- ~~Homebrew tap~~ ✅ shipped in 0.2.0 (`brew install jeffbai996/tap/logbook`)
- ~~Static `x86_64-linux-musl` binary for Alpine / scratch containers~~ ✅ shipped in 0.2.1
- ~~`CHANGELOG.md` + crates.io / docs.rs badges~~ ✅ shipped in 0.2.1

**0.3.0 — ergonomics**
- `logbook add` opens `$EDITOR` when `--why` is omitted (git-commit style)
- `logbook supersede <old-date> "new title" --why ...` — formal supersession syntax linking the new entry to the old one
- Colored TTY output (off when piped)
- `logbook export --format json` for tooling integrations

**Maybe-someday**
- Shell completion (`logbook completions bash`)
- A read-only web viewer that renders `logbook.md` as a timeline
- Team mode: aggregate logbooks across multiple repos for retro reviews

**Not on the roadmap.** Editing past entries, deleting entries, server mode, GUI, plugins, multi-user sync. Scope creep is the enemy.

## License

MIT. See [LICENSE](LICENSE).
