# Operator playbook — kernel + pulse sync

**Status:** Binding dogfood guide  
**Product:** ensembly operator kernel  
**Last updated:** 2026-09-05 (channel-pulse fixture dogfood)

Companion: [MAP.md](MAP.md) · [PRODUCT-CHARTER.md](PRODUCT-CHARTER.md) · [MUSK-CUT-2026-09-04.md](MUSK-CUT-2026-09-04.md) · [PRIVACY.md](PRIVACY.md)

Parked game/Node surfaces: [../prototype/README.md](../prototype/README.md) — not covered here.

---

## 0. What you are optimizing

| Scarce resource | What kernel does | What only you do |
|-----------------|------------------|------------------|
| **Attention** | Surfaces pending auth + physical beacons via runtime | Approve, deny, claim, complete |
| **Risk** | HITL gates in T1 SQLite; no unattended bank/email | Judgment |
| **Continuity** | Episodic memory + pulse-pack across harness sessions | Export/import discipline |

**North star:** Grok/Cursor capture proposals. Kernel records **outcomes**. You pair for body-world work.

---

## 1. Surfaces (live)

| Surface | When | Command |
|---------|------|---------|
| **Runtime HITL/HOOTL (SoT)** | Daily control plane | `cargo run -p peram-kernel -- runtime …` |
| **Reflect** | After ticks — coherence, skills | `cargo run -p peram-kernel -- runtime reflect` |
| **Turn / FocusPlan** | Coached next acts from CP | `cargo run -p peram-kernel -- turn` |
| **Pulse export/import** | Bot ↔ laptop memory sync | `pulse-pack export\|import\|status` |
| **peram-mcp** | Grok/Cursor read memory | `cargo build -p peram-agents --bin peram-mcp` |
| **Backup** | Sealed durability | `cargo run -p peram-kernel -- backup …` |

---

## 2. Runtime recipe (laptop or canonical host)

```bash
cargo test -p peram-kernel

cargo run -p peram-kernel -- runtime load --fixture fixtures/issue-1-runtime.json
cargo run -p peram-kernel -- runtime status

# HOOTL: one step per tick (claim OR complete)
cargo run -p peram-kernel -- runtime tick   # claim triage-inbox
cargo run -p peram-kernel -- runtime tick   # complete triage-inbox
cargo run -p peram-kernel -- runtime tick   # claim draft-transfer
cargo run -p peram-kernel -- runtime tick   # complete draft-transfer

# HITL: action id (pay-rent); auth- prefix accepted then stripped
cargo run -p peram-kernel -- runtime approve pay-rent

cargo run -p peram-kernel -- runtime claim grocery-errand
cargo run -p peram-kernel -- runtime complete grocery-errand

cargo run -p peram-kernel -- runtime status   # expect Hootl when gates cleared
cargo run -p peram-kernel -- runtime reflect
```

Durable store: `data/local/peram-ops.sqlite` (gitignored).  
`--json` prints JSON then trailing `RUNTIME_OK …` — strip last line before parsing.

Memory flags: `--memory <path>` (explicit; open failure fatal) · `--no-memory`.

---

## 3. Pulse + memory sync (bot ↔ laptop)

**Two pack layers — do not conflate:**

| Layer | Format | CLI | Laptop may import? |
|-------|--------|-----|-------------------|
| **T1 ops** | `peram-backup-pack-v1` / `peram-ops-bundle-v1` | `backup` · `restore-*` · `ops-bundle` | **No** — canonical host only |
| **Pulse** | `peram-pulse-pack-v1` | `pulse-pack export\|import\|status` | **Yes** — memory CRDT merge only |

**Topology:** Grok Bot = **canonical kernel host** (single writer on ops DB). Laptop = client — imports pulse packs, never dual-writes ops.

### Bot exports after session

```bash
cargo run -p peram-kernel -- pulse-pack export \
  --out ~/sync/pulse/bot-$(date +%Y%m%d).pulse.json \
  --include-archive

cargo run -p peram-kernel -- pulse-pack status --pack ~/sync/pulse/bot-*.pulse.json
```

### Laptop imports (idempotent CRDT merge)

```bash
cp /path/from/bot/bot-*.pulse.json ~/sync/pulse/

cargo run -p peram-kernel -- pulse-pack import --pack ~/sync/pulse/bot-*.pulse.json
cargo run -p peram-kernel -- runtime reflect
```

Paths (gitignored): `data/local/peram-memory.json`, `data/local/pulse-archive.jsonl`.

**Next (not automated yet):** Drive/shared-folder staging for `*.pulse.json` — file copy only, no live sync.

---

## 4. Harness fit (Grok / Cursor)

1. **Canonical host** runs `runtime load` + `tick` + gate commands during bot session.
2. **Export pulse** at session end.
3. **Register peram-mcp** for read-only memory queries during coding:
   ```bash
   cargo build -p peram-agents --bin peram-mcp
   grok mcp add --scope project peram -- cargo run -p peram-agents --bin peram-mcp
   ```
4. **Laptop** imports pulse; runs `reflect` — does not rewrite ops from chat.

Refuse: treating chat history as pending-ledger SoT.

---

## 5. Channel pulse (Issue #8)

**Law:** Pulse is admissions-filtered **observation** — it never writes **G**, gates, or priorities. One writer on `data/local/peram-ops.sqlite`.

| Surface | Command | Output |
|---------|---------|--------|
| **Channel IR** (stdout) | `cargo run -p peram-kernel -- turn --channel [--fixture …] [--location home\|travel\|office]` | Versioned JSON: `next_body`, `next_gate`, optional `where`/`when`, `snapshot_fingerprint`. No `TURN_OK` stderr banner. |
| **Weekday reconcile** | `cargo run -p peram-kernel -- --db <ops.sqlite> channel-pulse reconcile --fixture … --out <pulse.json>` | Diff wait-snapshot vs last pulse file. Empty DB + fixture → in-memory projection only. **Unchanged → exit 0, silent.** Changed → write `--out` (default gitignored `data/local/channel-pulse.json`). Never writes **G**. |

```bash
# Emit channel IR for a harness (parse stdout only)
cargo run -p peram-kernel -- turn --channel --fixture fixtures/issue-1-runtime.json

# Agent / CI fixture dogfood — isolated temp DB + pulse; no Eve, no channel bot, no live ops
cargo run -p peram-kernel -- --db /tmp/peram-ops-smoke.sqlite channel-pulse reconcile \
  --fixture fixtures/issue-1-runtime.json --out /tmp/channel-pulse.json --json
cargo run -p peram-kernel -- --db /tmp/peram-ops-smoke.sqlite channel-pulse reconcile \
  --fixture fixtures/issue-1-runtime.json --out /tmp/channel-pulse.json
  # unchanged → exit 0, silent

# Operator weekday path (canonical host only — default DB + gitignored pulse)
cargo run -p peram-kernel -- channel-pulse reconcile --fixture fixtures/issue-1-runtime.json
```

**Channel pulse JSON shape (v1):** `{ version, generated_at, next_body?, next_gate?, where?, when?, snapshot_fingerprint }`. Private/finance titles are redacted via the kernel classifier; gate ids remain for HITL approve/deny on the canonical host.

**Not this:** `pulse-pack` (episodic memory CRDT) · prototype `turn-status.json` · MCP write tools.

---

## 6. Remote / channels (trajectory)

Eve or Slack digests would call kernel with **redacted** JSON only. Not shipped at root. See [EVE-FIT.md](EVE-FIT.md).

---

## 7. Privacy checkpoint

Before any export or pulse copy:

- No `private/` contents in packs unless explicitly designed and classified.
- Pulse pack = memory traces + archive events — verify with `pulse-pack status`.
- Full rules: [PRIVACY.md](PRIVACY.md).

---

## 8. Quick reference

```bash
cargo test -p peram-kernel && cargo test -p peram-memory

cargo run -p peram-kernel -- runtime status
cargo run -p peram-kernel -- runtime approve <action-id>
cargo run -p peram-kernel -- runtime deny <action-id>
cargo run -p peram-kernel -- runtime claim <beacon-id>
cargo run -p peram-kernel -- runtime complete <beacon-id>

cargo run -p peram-kernel -- turn --channel --fixture fixtures/issue-1-runtime.json
cargo run -p peram-kernel -- --db /tmp/peram-ops-smoke.sqlite channel-pulse reconcile \
  --fixture fixtures/issue-1-runtime.json --out /tmp/channel-pulse.json --json

cargo run -p peram-kernel -- pulse-pack export --out /tmp/x.pulse.json
cargo run -p peram-kernel -- pulse-pack import --pack /tmp/x.pulse.json
```

**Footer:** One writer on ops. Pulse for memory. Harness for capture.
