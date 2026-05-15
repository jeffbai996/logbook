# dev-log

A tiny CLI for keeping a per-repo decision log. Drops a single `dev-log.md` file at the root of any project and gives you a one-line command to append a structured entry to it.

Built for solo-operator-plus-LLM workflows where heavyweight process (Agile, Jira, ADR templates) is overkill but "wait, why did I do this last month" is a real recurring cost.

## Why

In agentic coding, the human is the only thread of continuity across sessions. The agent loses state every time you `/clear`. Architecture decisions, library choices, and rejected approaches live in your head — until they don't, and you spend an hour re-deriving why the database is SQLite instead of Postgres.

A `dev-log.md` committed in the repo solves this for free:

- Travels with the code (clone the repo → you have the context)
- Searchable with `grep` or `git log`
- Agents can `cat dev-log.md` at session start to bootstrap context
- No external service, no DB, no ceremony — just markdown in git

This CLI just makes appending entries frictionless enough that you'll actually do it.

## Not a substitute for

- **README** — explains what the project does, for users
- **Commit messages** — explain what changed, per change
- **squad-store** — durable cross-project facts (people, ongoing initiatives, references)

dev-log fills the gap between those: *why* you made the structural choices that the code itself can't justify.

## Install

```bash
git clone git@github.com:jeffbai996/dev-log.git
cd dev-log
pip install -e .
```

Once installed, `dev-log` is available on `$PATH`.

## Usage

From inside any repo:

```bash
dev-log add \
  --why "polling at 1s was hammering yfinance and getting throttled" \
  --rejected "redis pub/sub (overkill for 1 user), SSE (no bidirectional need)" \
  --risk "websocket connection drops need reconnect logic — added exp backoff" \
  "switched ticker-tape from polling to websocket"
```

This appends a block to `./dev-log.md`:

```markdown
## 2026-05-15 — switched ticker-tape from polling to websocket
**why:** polling at 1s was hammering yfinance and getting throttled
**rejected:** redis pub/sub (overkill for 1 user), SSE (no bidirectional need)
**risk:** websocket connection drops need reconnect logic — added exp backoff
```

Optionally auto-stages the file (`--stage`) so it lands in the next commit.

### Other commands

- `dev-log init` — creates `dev-log.md` with header if it doesn't exist
- `dev-log list` — prints all entries newest-first
- `dev-log search <term>` — greps entries
- `dev-log last` — shows the most recent entry

## Format

Single markdown file, `dev-log.md`, append-only. Each entry is:

```markdown
## YYYY-MM-DD — <title>
**why:** <reason this was chosen>
**rejected:** <alternatives considered and rejected, with brief reason>
**risk:** <what could go wrong, if anything>
```

Only `--why` is required. `rejected` and `risk` are optional but encouraged for non-obvious choices.

## Philosophy

- **One file, in the repo.** No external dependencies, no service to run.
- **Append-only.** Never edit old entries — superseded decisions get a new entry.
- **Three fields max.** Why, rejected, risk. If you need more structure than that, write a real design doc.
- **45 seconds per entry.** If it takes longer, the tool is wrong.
- **Solo-operator scope.** This is not for teams. Teams have other tools.

## Status

Draft / pre-alpha. README first, code second. Planning to implement after the spec settles.

## License

TBD (probably MIT — placeholder).
