# Mesh × ensembly bridge (2026-09-08)

Lab: Cursor Origin [`persathyamz0z/mesh`](https://cursor.com/codebase/persathyamz0z/mesh)  
Public dump: [`thecuriousts/participatory-mess`](https://github.com/thecuriousts/participatory-mess)

Scope: **topology + trust boundary**. No crate merge. Mesh does not become a second life SoT.

## One sentence

**Bot/harness proposes → ensembly authorizes/claims on this node → mesh CommandFabric dispatches → another participant executes.**

Ensembly owns *whether* and *why*. Mesh owns *which verb may run where* on the private network.

## Topology

```
┌─────────────────────────────┐
│ Grok Bot / harness (capture)│
└──────────────┬──────────────┘
               │ proposes / claims work
               ▼
┌─────────────────────────────┐
│ ensembly-kernel (this host) │  HITL/HOOTL · ops SQLite one-writer
│ done / pending / denied     │  pulse-pack · memory aux
└──────────────┬──────────────┘
               │ when act is machine-mutate (not life-ledger)
               ▼
┌─────────────────────────────┐
│ mesh CommandFabric          │  allowlist only (no shell)
│ node A (Bot / ensembly host)│
└──────────────┬──────────────┘
               │ distributed Erlang / private network
               ▼
┌─────────────────────────────┐
│ mesh participant node B/C   │  restart_service · health · …
│ (another laptop / peer)     │
└─────────────────────────────┘
```

Cross-participant work is **normal**: the Bot need not be on the machine that executes. Ensembly on the canonical (or claiming) host still records authorize/claim/complete; mesh dispatches the allowlisted RPC to the peer.

## Layer map

| Concern | Owner |
|---------|--------|
| Life slots, obligations, pulse, preference/audit | **ensembly** |
| Allowlisted machine verbs across Tailscale peers | **mesh** |
| Capture / ADE / coding workers | Grok Bot, Orca, OpenCode, … |
| Portfolio vault | `~/life-os` |

## Keep

- Mesh as a **DelegationBackend-shaped hand**: ensembly claims → fabric executes only allowlisted commands on a target peer.
- Evidence: mesh traces / health → pulse archive; never auto-approve.
- One ensembly ops writer; mesh has its own cluster state — **do not dual-write life SoT into mesh Mnesia/ETS**.

## Refuse

- Folding mesh OTP into `ensembly-kernel`
- Mesh deciding life authorize/deny (or ensembly bypassing CommandFabric for shell)
- Treating Syncthing / Sunshine / VNC as ensembly product surface
- “Harmony across participants” as an excuse for a second chat OS

## Integrate (local slices)

| Slice | Done when |
|-------|-----------|
| A. This note + glossary/MAP cross-links | Docs PR |
| B. Mesh public + Origin docs state the same topology | participatory-mess republish |
| C. Optional later: `DelegationBackend` adapter calling `bin/mesh` / HTTP `:47989` | Allowlisted verbs only; HITL still owns claim/complete |

## Public voice (participatory-mess)

State product truth: bots/harnesses propose; ensembly authorizes/claims; mesh dispatches allowlisted peer acts. Do not publish Steward coach notes or life-slot chat meta on the GitHub dump.
