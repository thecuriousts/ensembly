---
name: run-ledger
description: Use when a task run starts or ends, when a claim needs a status, or when checking whether the router picked the wrong skills
owner: flowingstack
source: master-workflow G6 G12
last-reviewed: 2026-10-30
---

# run-ledger

## Flow

```stateDiagram-v2
  [*] --> Open: ledger open
  Open --> Claimed: ledger claim
  Claimed --> Claimed: status change
  Open --> Closed: ledger close
  Claimed --> Closed: ledger close
  Closed --> Compacted: ledger compact
  Compacted --> [*]
```

## Contract

```text
// Signature
L        ledger TSV, FLOWINGSTACK_LEDGER or .flowingstack/ledger.tsv
row(t)   open row of run t
c        contract id
S        verified | unverified | doubted

// Axioms
A1  row(t) written ≺ run(t) ∧ row.contract ≠ ∅
A2  |loads(t)| ≤ 4
A3  close(t) ∨ claim(t) → row(t) ∈ L
A4  claim.status ∈ S ∧ claim.source ≠ ∅
A5  compact(L) keeps first open, last close, last claim per text
A6  value ∌ tab, newline
```

| Command | Pre | Post | Exit |
|---|---|---|---|
| `ledger open --contract c --route r --loads a,b --pstack installed` | c ≠ ∅, A2 | row appended, run id printed | 0 or 2 |
| `ledger close t --opened a,b,c --switches n --failed-fixes n --override 0/1 --unrun x` | A3 | close row | 0 or 2 |
| `ledger claim t --text x --status s --source y` | A3, A4 | claim row | 0 or 2 |
| `ledger compact` | — | A5 | 0 |
| `ledger query q` | q ∈ Q | run ids | 0 or 2 |

## States

| Query q | Run t matches when |
|---|---|
| missed-load | opened ∖ loads ≠ ∅ |
| unused-load | loads ∖ opened ≠ ∅ |
| playbook-switch | switches > 0 |
| premise | failed fixes ≥ 2 |
| override | override = 1 |
| unrun-check | unrun ≠ ∅ |

## Checks

| Command | Expect |
|---|---|
| `python3 -m unittest discover -s test -p 'test_ledger*'` | green |
| `scripts/ledger --ledger test/fixtures/ledger/misroute.tsv query premise` | r3 |

## Notes

Each misroute run becomes a frozen eval case through eval-freeze.
