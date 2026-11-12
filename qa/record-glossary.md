# QA: record glossary

```text
drift     Status and EventKind from src/record.rs
          equal the glossary tables
red       drop one status row, the test fails and names it
green     restore the row, the test passes
hosts     laptop-1, laptop-2, mac-mini
compare   exit 0 match, exit 1 mismatch, exit 2 error
          read-only, CLI only
```

Run the drift test from the crate that contains `tests/glossary_drift.rs`.

1. `cargo test --locked --test glossary_drift`
   Expect: the record status set and the event kind set match the glossary.
2. Delete one backticked status row under Record status. Run the same test.
   Expect: failure, and the message names that status as missing from the glossary.
3. Restore the row. Run the same test. Expect pass.
4. `cargo fmt --all --check`
5. `cargo clippy --locked --workspace --all-targets -- -D warnings`
6. Privacy scan and host-alias scan on the new files. Expect zero hits.
   No email, home path, or hostname outside laptop-1, laptop-2, and mac-mini.
