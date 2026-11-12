# QA: applications ledger record adapter

Run from the repo root. Fixtures in the tests are invented. Hosts are laptop-1 and mac-mini.

1. `cargo test --locked -p ensembly-schema --test apps_record`
   Expect: every ledger status maps; `offered` is refused; a second plan is empty; a changed payload conflicts; compare lists matches and one injected mismatch.
2. `cargo test --locked -p ensembly-kernel --test apps_record_cli`
   Expect: import leaves the ledger bytes unchanged; a second import leaves the log bytes unchanged; compare on a read-only directory exits 0; a mismatched log exits non-zero and does not change either file.
3. `cargo fmt --all --check`
4. `cargo clippy --locked --workspace --all-targets -- -D warnings`
5. `cargo test --locked -p ensembly-schema -p ensembly-kernel -p ensembly-agents`
6. Privacy and host-alias scans on the diff. Expect zero hits. No email, home path, or hostname outside laptop-1 and mac-mini in the new files.
