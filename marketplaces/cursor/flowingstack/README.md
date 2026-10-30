# flowingstack

```agentflow-beta LR
  src["p10ns11y/plugins at pin"] --> sync["sync-from-upstream"] --> skills["skills"]
  skills --> lint["doc-lint"] --> priv["privacy-scan"] --> ok["exit 0"]
```

```text
// Signature
R      plugin root, FLOWINGSTACK_ROOT or --root
P      pins.tsv rows: repo sha40 path dest
B      baseline rows: path rule

// Axioms
A1  ∀ path p: p relative to R ∧ ".." ∉ p
A2  scan(f) → f ∈ R ∨ f = repo/.cursor-plugin/marketplace.json
A3  check: tree(dest) = tree(src@sha) ∧ NOTICE = f(P)
A4  symlink ∨ binary → exit 1
```

| Command | Exit 0 | Exit 1 | Exit 2 |
|---|---|---|---|
| `scripts/doc-lint --all` | clean | finding | absolute path |
| `scripts/privacy-scan --all [--repo-root D]` | clean | hit | — |
| `scripts/sync-from-upstream --src D [--check]` | synced or no drift | drift, binary, symlink | bad pin |
| `scripts/check --src D` | all three clean | any fail | — |
| `python3 -m unittest discover -s test` | green | red | — |
