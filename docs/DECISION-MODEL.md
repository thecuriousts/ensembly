# Decision model (E3)

Local eval and propose-and-wait only. The decider is not live. It does
not submit an application. Silence never becomes an action.

The triage question is fixed: **Should this posting be labelled apply?**
Labels are `apply` and `skip`. The model may abstain and defer to the
operator.

## What ships now

- Schema and types in `ensembly-schema` (`LabelRow`, `DecisionEvent`, frozen
  `assign_split`).
- A scam-gate that runs first and is never learned. Fixture flags use
  `*.example.test` only. Listed domains are matched on the posting URL
  host and on hosts extracted from the posting text (case-insensitive,
  trailing dots stripped, subdomains included). Cue matching and host
  extraction share one normaliser and run on several views; a hit on
  any view is a hit. Views cover (a) format/zero-width stripped, (b)
  format/zero-width replaced by a space, (c) NFKC then combining marks
  removed and Cyrillic/Greek confusables folded (also applied to
  domains), (d) up to three rounds of percent and HTML-entity decoding
  on URL authorities and the body, and (e) de-obfuscation of `[.]`,
  `(.)`, `{.}`, ` dot `, ` [dot] `, ` at `, and `[at]`. Soft cues
  (`Uncertain`) abstain: they are not a hard fail and do not skip the
  posting; the model defers to the operator.
- A transparent feature model and an untuned keyword baseline. Weights and
  the abstain band are fit on train only.
- Propose-and-wait through `KernelWriter`. Raw event-log writes are
  crate-private.
- A synthetic fixture set for tests. Those labels are invented and are
  not a claim about real accuracy.
- An empty labelling template for the operator. Real labels are not in
  this repository.

Held-out precision and accuracy, with a Wilson interval, are reported
only after the operator's held-out rows are labelled. Until then the
report says so and makes no accuracy claim. Eval JSON on a synthetic
fixture includes a `fixture_disclaimer` so those metrics cannot be
mistaken for operator-held-out claims.

`fit` and `pick_band` are public for tests and tooling, but they fit on
whatever rows they are given. Passing held-out rows into either is a
misuse; `decision eval` already restricts fitting to labelled train
rows.

## Statechart (written for this crate)

```mermaid
stateDiagram-v2
  [*] --> Gate
  Gate --> Skipped: listed domain or hard cue
  Gate --> Propose: clear
  Propose --> Waiting: card and event written
  Waiting --> Confirmed: operator confirm
  Waiting --> Overridden: operator override
  Waiting --> Expired: timeout with no signal
  Confirmed --> Record: append confirm event
  Overridden --> Record: append override event
  Expired --> NoAction: append expired, no apply
  Skipped --> Record
  Record --> [*]
  NoAction --> [*]
```

Rules:

- `rid` is `hash(link_key, "propose", model_version, snapshot_hash)`.
- Each step is idempotent on `(rid, event_type)`.
- The model never writes a posting status. Only an operator signal or a
  gate skip changes the proposal phase.
- Timeout (default 72 hours) writes `expired` and takes no action.
- A late confirm after expiry is refused. A new proposal needs a new rid.
- Writer identity is the configured `ENSEMBLY_WRITER` value.

## CLI

```bash
ENSEMBLY_WRITER=laptop-1 cargo run -p ensembly-kernel -- decision eval \
  --labels fixtures/decision/synthetic/labels.jsonl --json

ENSEMBLY_WRITER=laptop-1 cargo run -p ensembly-kernel -- decision template \
  --from postings.jsonl --out sheet.csv
```

`decision propose`, `confirm`, `override` and `expire` append to a sealed
decision event log through `KernelWriter`.

## What this is not

This milestone does not score the operator's real labelled set. It does
not turn silence into apply. It does not put the decider in the live
submit path.
