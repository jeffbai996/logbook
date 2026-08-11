# Changelog

Notable changes to `logbook` are recorded here.

## [Unreleased]

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

[Unreleased]: https://github.com/jeffbai996/logbook/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/jeffbai996/logbook/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jeffbai996/logbook/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/jeffbai996/logbook/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/jeffbai996/logbook/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/jeffbai996/logbook/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jeffbai996/logbook/compare/v0.0.3...v0.1.0
[0.0.3]: https://github.com/jeffbai996/logbook/releases/tag/v0.0.3
