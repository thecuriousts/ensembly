# hub

```agentflow-beta LR
  a["read"] --> b["route"]
```

| Fn | Pre | Post |
|---|---|---|
| route(t) | t ∈ Tasks | lane(t) ∈ Lanes |

```text
// Axioms
A1  contract.check = ∅ → route = empty
```

Routes each task to one lane.
