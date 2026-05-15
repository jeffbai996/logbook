# logbook

Append-only record of architectural decisions for this project.
Newest entries at the bottom. Generated and maintained by `logbook` — https://github.com/jeffbai996/logbook

## 2026-05-15 — rust + clap for the implementation
**why:** single static binary via cargo install, no python/node runtime, clap gives derive-macro CLI ergonomics with help/version/parsing for free
**rejected:** python (needs venv, slower install for users); go (more boilerplate for a 200-line tool); shell script (no help text, fragile arg parsing)
**risk:** rust toolchain required for cargo install; mitigated when published to crates.io (single binary download) and once GitHub Releases ship prebuilt binaries

## 2026-05-15 — single file logbook.md at repo root, append-only
**why:** max simplicity — file lives with code, travels through clones, no service to run, grep-friendly, agent-friendly (cat at session start)
**rejected:** SQLite (overkill for append-only text); JSON/YAML (less human-friendly than markdown); external service (defeats the in-repo locality benefit)
**risk:** no atomic-write protection — concurrent calls could interleave; acceptable for solo-CLI use case

## 2026-05-15 — 3 fields max: why, rejected, risk
**why:** constraint forces conciseness — 45 seconds per entry. More fields = drift toward design-doc territory which has its own home
**rejected:** free-form body (loses structure); ADR full template (too heavy); single 'notes' field (loses the rejected/risk discipline)

## 2026-05-15 — 0.0.2 — tags, show, stats, tags-list
**why:** feature-creep within the philosophy: tags enable cross-cutting concerns without breaking append-only; show <date> answers 'what did i decide tuesday'; stats gives a habit-tracker signal for dogfood feedback
**rejected:** color output (defer to 0.3.0 — needs TTY detection); --message-file/editor mode (defer to 0.3.0); supersede syntax (needs more spec time)
**risk:** now reading + parsing the whole file on read commands — fine until a logbook has thousands of entries, then we want streaming/index
**tags:** features, scope

