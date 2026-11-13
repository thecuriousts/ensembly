# QA: applications ledger record adapter

```text
ledger -> record, append only
re-import      ledger fields only; writer and clock ignored; appended 0
after plan     replay; a stage that differs -> non-zero, names the link; log unchanged
compare        stage, submit time, and silent since
               exit 1 mismatch
               exit 2 error, including a missing log or a directory given as the log
no submit time import clock is stored and marked synthetic
writers        configured ENSEMBLY_WRITER identity; fixtures use laptop-1, laptop-2, mac-mini

known limit    submitted -> interview -> submitted is refused
               the ledger stays the truth; the log is never rewritten
               that one link blocks the rest of the import
```

Run from the repo root. Fixtures in the tests are invented. Hosts are laptop-1, laptop-2, and mac-mini.

1. `cargo test --locked -p ensembly-schema --test apps_record`
   Expect: every ledger status maps; `offered` is refused; a second plan is empty for another writer or clock; a changed ledger field conflicts; compare lists a submit-time mismatch; an open status is not mapped as intended.
2. `cargo test --locked -p ensembly-kernel --test apps_record_cli`
   Expect: a second import with no clock flag, and a second import as another writer, append nothing; a return to submitted exits non-zero and leaves the log unchanged; compare mismatch exits 1; a missing log or a directory exits 2.
3. `cargo fmt --all --check`
4. `cargo clippy --locked --workspace --all-targets -- -D warnings`
5. `cargo test --locked -p ensembly-schema -p ensembly-kernel -p ensembly-agents`
6. Privacy and host-alias scans on the diff. Expect zero hits. No email, home path, or hostname outside laptop-1, laptop-2, and mac-mini in the new files.
