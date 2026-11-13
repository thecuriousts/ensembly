# E5: pulse on vs off, synthetic treatment run

**Synthetic.** The replies are written by `scripts/e5/synthetic_replies.py`, not returned
by a model. It answers each task right or makes one mistake, with a set chance of a
mistake per arm (`P_WRONG`: off 0.40, on 0.25, fixed before the run). So the gap between
the arms comes from that parameter, not from pulse. This run checks the pipeline: same
tasks, same model id, same prompt plus the pulse pack, three reruns, the spread, and the
hashes. It measures nothing about pulse or any model. **Not a CV number.**

## Design

- **Tasks:** the 12 frozen E1 tasks (`crates/evalbench-core/fixtures/tasks`), same hash.
- **Model id:** `synthetic-replay-model` in both arms, as in the E1 baseline.
- **Prompt:**
  - Off arm: the E1 prompt `gaps-v1`, byte for byte.
  - On arm (`gaps-v1+pulse`): the same bytes, then `PULSE_HEADER`, then the pulse pack.
  - That is the only change, and a test checks it on every task.
- **Pulse pack:** `crates/evalbench-core/fixtures/pulse/e5-pulse.json`. It holds 4
  synthetic traces about the checking rules, as canonical `pulse-pack-v1` bytes, and both
  pack checkers return ok on it.
- **Reruns:** 3 per arm (`off-r1..3`, `on-r1..3`). Each rerun is a new seed of the
  synthetic generator. Each run is scored on first attempts only, as in E1.
- **Tokens:** `tokens_in` is the prompt bytes divided by 4, rounded up, so the on arm
  carries the pack's size in tokens. Cost uses a made-up price.

## Result

<!-- e5-table:begin -->
| Arm | Prompt id | Score per rerun | Min to max | Mean score | Pooled first pass | Median tokens per passing task |
|---|---|---|---|---|---|---|
| pulse off | `gaps-v1` | 58, 50, 66 | 50 to 66 | 58.0 | 21/36 (58.3%) | 246, 245, 243 |
| pulse on | `gaps-v1+pulse` | 66, 83, 75 | 66 to 83 | 74.7 | 27/36 (75.0%) | 777, 776, 776 |

n = 12 tasks per run, 3 reruns per arm. Dataset hash `55faad2e303c22039226da1295312218c6f3b51af4d643d00ab5af8364b4b3df`. Model id `synthetic-replay-model`. Synthetic replies (see `scripts/e5/synthetic_replies.py`): not a model result, not a CV number.
<!-- e5-table:end -->

Generated at commit `5599c3057c8dc38367f33cb1e7a296ccd3c2239a`.

Pulse costs about 530 more input tokens per task here (the pack is 2005 bytes). That is
a property of this fixture, not a measurement.

## Regenerate

```sh
scripts/evalbench-treatment           # replay all six runs, score, compare; exit 0 same, 1 drift
scripts/evalbench-treatment --write   # also rewrite traces, scores, summary.json and the table
python3 -I scripts/e5/synthetic_replies.py --check   # the committed replies match the generator
python3 scripts/e5/build_chart.py     # rebuild chart/index.html from summary.json
```

The chart (`chart/index.html`) is drawn with Minicharts (MIT, April Zhu, 2026). Its
licence and notice are in `chart/vendor/minicharts/`. The fonts are bundled in `chart/fonts/`.
