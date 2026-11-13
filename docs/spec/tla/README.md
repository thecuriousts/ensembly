# PulseTrace: a small TLA+ model of the pulse store

`PulseTrace.tla` models two trace rows (`old`, `new`), the supersession phases
(`none -> intent -> applied -> verified`), the evidence pin (`verify_hash`), row
locks, crashes and `SQLITE_BUSY` timeouts between transactions, and the
read-only check (`mode=ro` + `PRAGMA query_only`). It is synthetic and abstract:
no text, times or hosts.

## Run

```sh
java -cp tla2tools.jar tlc2.TLC -deadlock -workers 4 -config PulseTrace.cfg PulseTrace.tla
```

`-deadlock` turns off deadlock reporting: the write bound (`WithinBound`) ends
every behaviour. CI does not run TLC (no JRE in the workflow); the Rust tests
below check the same rules on the code.

## Result (2026-11-13, TLC2 2026.11.06, JRE 21)

| Config | Constants | Result |
| --- | --- | --- |
| `PulseTrace.cfg` (the current code) | `BuggyAdmit = FALSE`, `BuggyCheck = FALSE`, `MaxWrites = 7` | No error, `CheckIsHonest` included. 9737 states generated, 1408 distinct, depth 8. |
| `PulseTraceBeforeE2.cfg` | `BuggyAdmit = TRUE` | `ConfirmedNeedsEvidence` violated at depth 2 (admit left `verify_hash` NULL). Fixed in E2. |
| `PulseTraceCheckBeforeFix.cfg` | `BuggyCheck = TRUE` | `CheckIsHonest` violated at depth 4 (the old `pulse check` said ok on an unpinned tool-verified row or a half-applied supersession). Fixed: `db.evidence` and `db.supersession` warn findings. |

## Rules and the tests that check them

| Rule | Kind | Test |
| --- | --- | --- |
| `TypeOK` | invariant | model only |
| `ConfirmedNeedsEvidence` | invariant | `admit_writes_the_content_hash_python_expects` |
| `CheckIsHonest` | invariant | `check_warns_on_tool_verified_without_evidence`, `check_warns_on_half_finished_supersession`, `db_check_findings_match_goldens_in_rust_and_python` |
| `HalfStateResumable` | invariant | `intent_then_retry_reaches_verified` |
| `PointerOnlyAfterApply` | invariant | `bad_writer_is_refused_before_any_pointer_change` |
| `ReadOnlyNeverWrites` | property | `rust_open_ro_refuses_every_write`, `python_open_ro_refuses_every_write`, `rust_from_db_reproduces_goldens_and_leaves_db_untouched` |
| `NoWriteOnTimeout` | property | model only |
| `LockedNeverOverwritten` | property | `admit_supersedes_links_the_new_row_and_refuses_an_unlinked_fork` |

`tla_rules_map_to_tests` in `crates/ensembly-kernel/tests/pulse_pack_canon.rs`
keeps this table in step with `PulseTrace.cfg` and checks every named test exists.
