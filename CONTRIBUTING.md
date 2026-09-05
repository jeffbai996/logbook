# Contributing to logbook

Logbook's 0.5 series is feature-complete for its intended scope. Bug fixes,
portability improvements, dependency upkeep, and changes that clarify the
existing workflow are welcome. New commands need evidence of a missing core
decision-log workflow; there is no standing feature roadmap. Services,
databases, plugins, automatic extraction, and other platform work remain out of
scope.

For a behavioral change, explain the user friction it removes before writing a
large patch. Keep each pull request to one logical change.

## Checks

```bash
cargo test --locked
cargo +1.85.0 test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
cargo package --locked
```

Keep tests at the contract boundary:

- parser and renderer invariants belong beside the implementation;
- filesystem safety and CLI behavior belong in integration tests;
- cross-platform behavior must not assume a Unix shell unless guarded;
- do not add a test only to restate a private implementation detail.

The minimum supported Rust version is 1.85 and is checked in CI.

## Commits

Use conventional commits with a subject no longer than 72 characters:

```text
fix: preserve quoted editor commands
feat: limit recent decision output
```

Explain why in the body when the reason is not obvious from the diff. Do not
include generated attribution.

## Releasing

1. Choose the version from the user-visible substance of the change.
2. Update `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and versioned examples.
3. Run every check above and dogfood the literal README examples.
4. Commit, tag `v<version>`, and push the tag. The release workflow verifies
   the tag, builds six archives, and publishes `SHA256SUMS` through one release
   job.
5. Publish the same version with `cargo publish --locked`.
