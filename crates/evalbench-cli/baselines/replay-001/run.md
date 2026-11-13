# replay-001

- What: first-pass score over the 12 frozen synthetic tasks, from recorded replies.
- Model: `synthetic-replay-model`, a hand-written synthetic reply set (the frozen
  `fixtures/replay/replies.jsonl`). Not a live model. No API keys, no network.
- Dataset hash: `55faad2e303c22039226da1295312218c6f3b51af4d643d00ab5af8364b4b3df` (36 files, 12 tasks)
- Prompt: `gaps-v1`
- n: 12 tasks, one first attempt each
- Result: first_pass 7, score 58 (floor(100*7/12)), median tokens per passing task 421, missing 0
- Generated: 13 Nov 2026 at commit `a0b5c4d16f377fd5ad05a357d213b1bb7cbe4e81` (branch `e1/evalbench`);
  the squash-merge SHA will differ, so re-run the command at any later commit.
- Command: `scripts/evalbench-baseline` (exit 0 same numbers, 1 drift; `--write` rewrites
  `traces.jsonl` and `score.json`)
- Not a CV number. Live runs and the spread over reruns are E5.
