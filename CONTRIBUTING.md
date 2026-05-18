# Contributing to logbook

Thanks for considering a contribution. This project is intentionally small — the philosophy is in the [README](README.md#philosophy) — so contributions land easier when they're aligned with that scope.

## Before you start

- **Read the "Not on the roadmap" section in the README.** Editing past entries, server mode, GUI, plugins, multi-user sync are explicit no-goes. PRs that add these will be politely declined.
- **Open an issue first** for anything bigger than a bug fix or doc tweak. Two paragraphs of "what + why" in an issue saves you from writing code that gets bounced.
- **One logical change per PR.** Bundling unrelated changes makes review slow and rollbacks painful.

## Workflow

```bash
git clone https://github.com/jeffbai996/logbook.git
cd logbook

cargo build              # debug build
cargo test               # full suite — 56+ tests across 4 categories
cargo fmt --all          # apply formatting
cargo clippy --all-targets -- -D warnings   # lint
cargo doc --open         # browse rustdoc locally
```

CI runs the same commands across Linux + macOS + Windows on every push and PR. Get it green locally first and you won't have to do the round-trip.

## Test discipline

The suite is meaningful — it's the spec. New behavior needs a test:

- **Pure functions** → unit test in the same module's `#[cfg(test)] mod tests`.
- **CLI surface** → integration test in `tests/cli.rs` using `assert_cmd` against a tempdir.
- **Format-stable output** → snapshot test in `tests/property.rs` via `insta`.
- **Round-trip invariants** → property test via `proptest`.
- **Public lib item** → rustdoc with executable `# Example` block.

Snapshot review when output intentionally changes: `cargo install cargo-insta && cargo insta review`.

## Commit messages

Conventional commits, one line under ~70 chars:

- `feat: …` new user-visible behavior
- `fix: …` bug fix
- `refactor: …` no behavior change
- `docs: …` documentation only
- `test: …` tests only
- `chore: …` build / deps / CI / housekeeping
- `release: …` version bumps

Body in the imperative; explain the *why* not the *what*. Keep one logical change per commit so `git bisect` stays useful.

## Releasing (maintainer notes)

1. Bump `version` in `Cargo.toml`.
2. Update the README's "Status" section.
3. Commit + tag `v<version>` + push tags. The release workflow auto-builds and uploads 5 prebuilt binary archives to GitHub Releases.
4. Run `cargo publish` for crates.io.
5. From `~/repos/homebrew-tap`, run `./update_logbook.sh` to backfill formula SHA256s, then commit + push the tap.

## Code of conduct

Be decent. Don't ship things you wouldn't be willing to maintain.
