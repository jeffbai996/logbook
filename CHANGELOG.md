# Changelog

Notable changes to `logbook` are recorded here.

## [Unreleased]

## [0.5.1] - 2026-08-29

### Added

- `--superseded` complements `--active` across `list`, `search`, and `export`,
  making either side of a reversed decision directly retrievable.
- `trace --format json` emits a complete supersession chain as stable entry
  objects in decision order.
- `check --format json` emits validation status, decision-state counts, and all
  structural issues while preserving the command's nonzero failure status.
- `completions` generates scripts for Bash, Elvish, Fish, PowerShell, and Zsh.

### Changed

- Added the narrowly scoped `clap_complete` dependency for completion
  generation. Its Rust 1.85 requirement matches the existing MSRV.
- Version 0.5.1 is the frozen feature-complete surface. Future releases are
  limited to fixes, portability, and maintenance unless real use exposes a
  missing core decision-log workflow.

### Fixed

- The lint job no longer runs target-cache cleanup around `cargo package`,
  avoiding false ENOENT error annotations when Cargo removes its transient
  verification tree. Test jobs remain cached.
- Windows appenders retry the brief access-denied state emitted while another
  process removes the write-lock directory instead of failing a safe append.

## [0.5.0] - 2026-08-29

### Added

- Repository-aware discovery finds the nearest `logbook.md` from nested
  directories without crossing the current Git root. Global `--file PATH`
  overrides discovery and `LOGBOOK_FILE`.
- Shared `--tag`, `--since`, `--until`, `--active`, and `--limit` filters for
  `list`, `search`, and `export`; `list` and `export` also accept `--search`.
  Repeated tags match entries containing every supplied tag.
- `trace` prints an entire supersession chain in either direction, while
  `show --title` disambiguates same-day decisions.
- `check` validates required fields, real calendar dates, duplicate decision
  references, and missing, ambiguous, or branched supersession links.
- JSON Lines export via `export --format jsonl`. JSON records now include
  derived `active` and `superseded_by` fields.
- `--why -` reads a reason from stdin, `--date` records imported decisions,
  `init --stage` stages a new logbook, and `supersede --print` echoes its entry.
- GitHub release archives now include a generated `SHA256SUMS` file.

### Changed

- Multiline `why`, `rejected`, and `risk` fields now round-trip through parsing
  and machine export instead of retaining only their first line.
- CLI date arguments reject impossible calendar dates, titles and tags reject
  ambiguous line-breaking characters, duplicate tags are folded, and empty
  stdin reasons abort without writing. New entries reject duplicate dated
  titles, and `supersede` refuses to branch an already-superseded decision.
- `stats` reports active and superseded counts. Bounded machine exports remain
  in document order; human reads remain newest-first.
- Version 0.5 marks the intended feature-complete surface. Maintenance,
  portability fixes, and dependency upkeep continue without a broader roadmap.
- GitHub workflows use the current Node 24 action majors, including strict
  artifact digest verification during release assembly.

### Fixed

- Concurrent appenders now serialize around the full read-copy-rename cycle,
  preventing the last writer from silently discarding another complete entry.
- Atomic appends now report temp-file synchronization failures instead of
  continuing to rename data that was not confirmed durable.
- Parallel editor captures use collision-proof temp names instead of relying
  on wall-clock resolution, eliminating intermittent cross-test and real-use
  contamination.
- Initialization uses create-new semantics and cannot truncate a file created
  by a racing process.
- The release workflow collects all six builds before one job updates the
  GitHub Release, preventing every matrix worker from duplicating release notes.
- Closing a downstream stdout pipe early no longer panics the CLI; commands
  now exit cleanly in pipelines such as `logbook list | head`, and write/stage
  side effects finish before success output can be closed.

## [0.4.0] - 2026-08-10

### Added

- `--limit N` for `list` and `export`, giving humans and coding agents a
  bounded view of recent decisions without a new context command.
- `--old-title` for selecting a supersession target when several entries share
  a date.

### Changed

- Superseding entries now record the old date and title, making references
  traceable instead of date-ambiguous.
- Raised the MSRV from Rust 1.75 to 1.85. Current `clap`, `assert_cmd`, and
  `proptest` releases already require 1.85, and the lockfile is not readable by
  Cargo 1.75. CI now tests the declared MSRV directly.
- Narrowed the library API to the core crate-root re-exports. Internal module
  paths and editor/testing helpers are no longer public; parser, renderer,
  export, error, and filesystem types remain available at the crate root.
- Reduced repetitive README, rustdoc, comments, and tests while retaining the
  parser, append/supersede, filesystem, CLI, output, and cross-platform
  contracts.

### Fixed

- Quoted editor paths and editor commands containing arguments now work.
- Atomic appends use unique sibling temp files, clean them up on failure, and
  preserve existing file permissions. Concurrent writers can no longer
  interfere through one shared `logbook.md.tmp` path.
- Release instructions now match the six shipped archives and no longer
  reference the retired Homebrew tap.

### Removed

- Snapshot-test dependency and snapshots that duplicated exact renderer unit
  tests.
- Speculative viewer/team roadmap items and repository contribution templates
  that added ceremony without serving current users.

## [0.3.0] - 2026-05-29

### Added

- JSON export with stable fields and dependency-free escaping.
- `$EDITOR`/`$VISUAL` flow when `--why` is omitted.
- Append-only supersession syntax.
- TTY-aware colored output honoring `NO_COLOR`.

### Removed

- Homebrew tap; crates.io and prebuilt archives remain the supported install
  paths.

## [0.2.1] - 2026-05-19

### Added

- Changelog and crates.io/docs.rs badges.
- Static `x86_64-unknown-linux-musl` release target.

## [0.2.0] - 2026-05-19

### Added

- crates.io publication and prebuilt GitHub release archives.

## [0.1.1] - 2026-05-17

### Added

- Parser/renderer property tests and public API documentation.
- MIT license.

### Changed

- Reworked the README for first-time users.

## [0.1.0] - 2026-05-16

### Added

- Library/binary split, typed errors, CLI integration tests, and
  Linux/macOS/Windows CI.

## [0.0.3] - earlier

### Added

- Initial CLI, tags, filters, atomic writes, `LOGBOOK_FILE`, and utility
  commands across the 0.0.x prototypes.

[Unreleased]: https://github.com/jeffbai996/logbook/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/jeffbai996/logbook/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/jeffbai996/logbook/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/jeffbai996/logbook/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jeffbai996/logbook/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/jeffbai996/logbook/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/jeffbai996/logbook/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/jeffbai996/logbook/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jeffbai996/logbook/compare/v0.0.3...v0.1.0
[0.0.3]: https://github.com/jeffbai996/logbook/releases/tag/v0.0.3
