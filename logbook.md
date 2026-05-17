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

## 2026-05-15 — 0.0.3 — atomic writes, LOGBOOK_FILE env var, --since/--until, --print, where
**why:** first robustness pass before 0.1.0 tests. atomic write via tmp+rename means a crashed CLI run can never leave a half-written entry; LOGBOOK_FILE unblocks monorepos and personal-log use cases; --since/--until generalize the show command for time-window queries; --print supports piping into other tools; 'where' makes the env-var resolution debuggable
**rejected:** fsync on every write (overkill for human-pace writes — sync_all on the tmp file is enough); --message-file/editor mode (defer to 0.3.0, needs  spawn logic); file locking via fcntl (overkill — atomic rename solves the race we actually have)
**risk:** atomic_append reads the whole file into memory before writing — fine for <10MB logbooks, becomes a problem in the millions-of-entries case but that's not the target user. tmp file lives next to logbook.md briefly during write — if the dir is read-only this fails loudly which is the correct behavior
**tags:** features, robustness

## 2026-05-16 — 0.1.0 — tests, typed errors, lib/bin split, CI
**why:** first version meant to be depended on: 22 unit + 19 integration tests pin every visible behavior; typed Error enum lets future callers match on failure modes instead of grepping anyhow messages; lib/bin split lets the suite test the parser + store directly without shelling out; CI on three OSes catches windows/macos regressions early. semver-spirit: 0.0.x = 'i'm exploring', 0.1.0 = 'i trust this enough to recommend'
**rejected:** 0.0.4 (would have been feature-creep without proof the existing surface is solid — wrong order); proptest round-trip generators (overkill for the 200-line parser, deferred to maybe-someday); separate crate for the library (no second binary planned, single-crate keeps install simple)
**risk:** no published crates.io release means cargo install logbook still doesn't work for users — that's 0.2.0. README docs the source build workflow as the official path until then. CI burns ~3 minutes per push but parallelizes well so it's not blocking
**tags:** release, tests, ci

## 2026-05-16 — 0.1.1 — proof and polish patch: property/snapshot tests, full rustdoc, public-first README
**why:** patch release discipline: no new user-visible features (those are 0.2.x), only proof that what's built actually works and docs that a stranger can use. proptest round-trip catches whole classes of bugs unit tests miss (renderer/parser drift); insta snapshots pin canonical output so silent format changes fail CI; rustdoc on every public item means crates.io and IDE hover-help aren't empty; README rewrite assumes zero prior context — leads with a working example, includes ToC, FAQ, comparison-to-alternatives table
**rejected:** new commands (would be feature creep, breaks semver-patch contract); colored output (deferred to 0.3.0 as planned); changing the on-disk format (would break round-trip property tests by definition)
**risk:** the proptest 'parser never panics' guarantee is only as strong as the input regex — we don't yet feed it true random bytes via arbtest or AFL; that's fine for a 200-line parser but worth knowing if the format grows. snapshots are reviewed by humans so a careless 'cargo insta accept' could mask a real regression
**tags:** release, tests, docs

## 2026-05-16 — 0.2.0 — distribution: crates.io, prebuilt binaries, homebrew tap
**why:** 0.1.x proved the tool works. 0.2.x makes it installable without a rust toolchain. three install paths added: cargo install logbook (from crates.io), brew install jeffbai996/tap/logbook (homebrew tap that points at prebuilt-binary archives), and direct binary download from github releases. release.yml builds 5 targets on every tag push (x86_64+arm64 linux, x86_64+arm64 macOS, x86_64 windows) and attaches to the GitHub Release. semver-wise this is a minor bump because no user-visible CLI behavior changes — only how you obtain the binary
**rejected:** scoop/chocolatey for windows (homebrew covers the majority of unix users; windows users tend to grab binaries directly and the install path is documented); cargo-binstall as a separate path (it works automatically off the cargo metadata + github releases — no extra config needed from us); musl static linux builds (gnu binaries work on every modern distro and have better debug symbols; revisit if alpine users complain); signing/notarization for macOS binaries (cost benefit doesn't justify — users can override gatekeeper on first run, common for unsigned CLI tools)
**risk:** homebrew formula has placeholder SHA256s until the release.yml run completes and update_logbook.sh patches them in — first install attempt before that runs will fail loudly. crates.io publish is irreversible: version numbers can never be re-used even on yank. release.yml cross-compiles aarch64-linux via gcc-aarch64-linux-gnu — most binaries work but anything that links openssl-sys would fail (we don't, but future deps need to be checked)
**tags:** release, distribution, crates-io, homebrew

