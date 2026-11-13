# Local-check fixtures

Synthetic statements only. No personal data, host names, or home paths.

CI replays the recorded JSON in `classify/` and `conflict/`. It does not
download or run models. Live runs on mac-mini are opt-in:

```
ENSEMBLY_LOCAL_MODELS=1 cargo run -p ensembly-agents --bin ensembly-local-check -- fixtures
```

After editing a fixture, refresh the digest:

```
# printed by: cargo test -p ensembly-agents recorded_fixture_digest_matches_manifest -- --nocapture
```

and replace the hash in `MANIFEST.sha256`.
