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

## Usage

From inside any repo:

```bash
logbook add "switched ticker-tape from polling to websocket" \
  --why "polling at 1s was hammering the upstream API and getting throttled" \
  --rejected "redis pub/sub (overkill for 1 user), SSE (no bidirectional need)" \
  --risk "websocket drops need reconnect logic — added exp backoff"
```

This appends a block to `./logbook.md`:

```markdown
## 2026-05-15 — switched ticker-tape from polling to websocket
**why:** polling at 1s was hammering the upstream API and getting throttled
**rejected:** redis pub/sub (overkill for 1 user), SSE (no bidirectional need)
**risk:** websocket drops need reconnect logic — added exp backoff
```

Pass `--stage` to also `git add logbook.md` so the entry lands in your next commit.

### Other commands

- `logbook init` — create `logbook.md` with a header if it doesn't exist
- `logbook list` — print all entries, newest first
- `logbook search <term>` — case-insensitive grep of entries
- `logbook last` — show the most recent entry

## Format

Single markdown file, `logbook.md`, at the project root, append-only. Each entry:

```markdown
## YYYY-MM-DD — <title>
**why:** <reason this was chosen>
**rejected:** <alternatives considered and why not, comma-separated>
**risk:** <what could go wrong>
```

Only the title and `--why` are required. `--rejected` and `--risk` are optional but recommended.

## Philosophy

- **One file, in the repo.** No external dependencies, no service to run.
- **Append-only.** Never edit old entries. If a decision is reversed, write a new entry that supersedes it.
- **Three fields max.** Why, rejected, risk. If you need more structure, you need a design doc.
- **45 seconds per entry.** If it takes longer, the tool is wrong.

## Status

`0.0.1` — minimum viable. `init`, `add`, `list`, `search`, `last` commands work. No tests yet, no error-message polish, no `cargo install logbook` until published to crates.io.

## License

MIT
