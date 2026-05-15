# tests/

Reserved for the test suite — empty for now.

Tests land in **0.1.0**. Plan:

- Unit tests for entry parsing (`make_entry`), tag filtering, date validation
- Integration tests using `tempfile` + `assert_cmd` to drive the CLI end-to-end against a throwaway `logbook.md`
- Property tests via `proptest` for the markdown parser (round-trip: write entry → read entries → first matches input)

Run with `cargo test` once populated.
