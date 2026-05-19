# Changelog

All notable changes to `logbook` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-05-19

### Added
- `CHANGELOG.md` at project root, following Keep a Changelog format.
- crates.io and docs.rs badges in `README.md`.
- `x86_64-unknown-linux-musl` target in the release workflow — static binary for Alpine, scratch containers, and ancient glibc systems.

## [0.2.0] - 2026-05-19

### Added
- Published to [crates.io](https://crates.io/crates/logbook) — install with `cargo install logbook`.
- Prebuilt release binaries on GitHub Releases for five targets: `x86_64-linux`, `aarch64-linux`, `x86_64-mac`, `aarch64-mac`, `x86_64-windows`.
- Homebrew tap: `brew install jeffbai996/tap/logbook`.
- CI release workflow that builds and uploads binaries on every tag push.

### Changed
- `Cargo.toml` polished for crates.io publication (description, keywords, categories, repository, documentation links).

## [0.1.1] - 2026-05-17

### Added
- Full rustdoc coverage on every public library item.
- Property tests and snapshot tests for the parser/renderer.

### Changed
- README rewritten for public onboarding.

### Added
- MIT `LICENSE` file at project root.

## [0.1.0] - 2026-05-16

### Added
- First version intended to be depended on by other projects.
- Library + binary split: `src/lib.rs` exposes the parser/renderer, `src/main.rs` is a slim CLI wrapper.
- Custom `Error` type replacing ad-hoc string errors.
- End-to-end CLI integration suite (19 tests).
- Unit tests for parser, date validator, and renderer.
- GitHub Actions CI: build + test on Linux/macOS/Windows, `cargo fmt` and `cargo clippy` lint.

## [0.0.3] - earlier

### Added
- Initial prototype releases (0.0.1 → 0.0.3) — pre-public iterations of the CLI surface and `logbook.md` format.

[Unreleased]: https://github.com/jeffbai996/logbook/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/jeffbai996/logbook/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/jeffbai996/logbook/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/jeffbai996/logbook/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jeffbai996/logbook/compare/v0.0.3...v0.1.0
[0.0.3]: https://github.com/jeffbai996/logbook/releases/tag/v0.0.3
