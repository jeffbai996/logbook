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

