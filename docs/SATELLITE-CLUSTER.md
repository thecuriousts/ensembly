# Satellite cluster — friction removers (binding)

**Status:** Product law  
**Date:** 2026-08-17  
**Complements:** [LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md)

Personal friction removers are a **cluster of apps**, not one mega-repo. They deploy side by side, **read** each other’s SoT freely, and **write only through the owner** (gated). Any app may **open** any other app or ask the owner to run a named action.

## Members (now)

| App | SoT it **writes** | Others may |
|-----|-------------------|------------|
| **life-os** | Public-safe wiki (`UI/Mission.md` generic) | Read |
| **mission-map** (`mm-lifeos-graph`) | `~/.grok/mission-maps/*`, `UI/_private.Mission.md` | Read |
| **collab-finder** | SQLite + `application_packs/` | Read; **Heading** screen is the career cockpit |
| **focus-now** | `~/.config/focus-now/live.json` | Read |
| **premflow** | `~/.premflow/` | Read |
| **ensembly** | Turn / day / HITL / clone ledger | Read; **delegates** (`open`, `run`) — does not grow a second hiring UI |
| **waybar / mako** | none | Signal only; click **opens** a member |

## Rules

1. **No merge.** Cluster ≠ one process. Same as life-os ≠ ensembly.
2. **Read is free** on local SoT paths. No copy-into-every-app caches that drift.
3. **Write is gated.** Only the owner mutates its SoT. Cross-app update = invoke the owner (`mm-lifeos-graph`, CF `update_opportunity_status`, …), never silent file smash from a sibling.
4. **Open is free.** From any app: focus or launch another (`mm-waybar open` → CF Heading). Route file: `~/.grok/mission-maps/open-route` (one line, consumed once).
5. **Background invoke is an owner CLI**, not a new bus crate. Nightly timer already runs `mm-lifeos-graph`. Ensembly later: `exec` the same binaries.
6. **No new GTK overlay.** CF is the existing Tauri desktop. CF sidebar **Mission** = career-board hunt. Map view = **Heading**.
7. **Process-safety** on anything that hits git: life-os `AGENTS.md`. Names, mail, job ids stay in local SoT.

## First slice (P1)

Notify / waybar **click** → write `open-route=heading` → launch or focus collab-finder → **Heading** screen (map + contacts + open URL / copy mail).

Disprove: if Heading is unused for two weeks, do not grow the cluster bus.
