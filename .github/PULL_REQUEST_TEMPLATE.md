## What

<!-- One sentence describing the change. -->

## Why

<!-- The motivation. Link to an issue if there is one. -->

## How

<!-- Brief notes on the implementation choices that aren't obvious from the diff. -->

## Test plan

- [ ] `cargo test` passes locally (56+ tests)
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] If snapshot output changed, ran `cargo insta review` and accepted intentionally
- [ ] If user-visible behavior changed, README updated
- [ ] If a new public lib item was added, rustdoc + doctest added
