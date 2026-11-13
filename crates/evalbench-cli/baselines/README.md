# Baselines

Each run lands as `<run_id>/{traces.jsonl,replies.jsonl,score.json,run.md}`;
`run.md` holds the commit SHA, the command, the date and n. Synthetic tasks only.
`tests/baselines.rs` replays and re-scores every committed baseline and fails on any drift.

- `replay-001`: recorded synthetic replies (`synthetic-replay-model`), offline, no API keys.
  It proves the pipeline is deterministic. It is not a model result and supports no CV claim.

A live baseline waits for the operator's yes on model id, wrapper and cost cap.
