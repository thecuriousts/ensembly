# Privacy contract

Ensembly is a **life swarm control plane**. It may *read* private operator context on the local machine. It must **never** push private data to git remotes or treat private actions as public events.

## Split

| Layer | Location | Git | Contents |
|-------|----------|-----|----------|
| **Full persona** | `private/persona/` | **ignored** | Detailed operator model (family/health/finance-adjacent themes from local sources) |
| **Public projection** | `public/persona/` | tracked | Scrubbed principles, rhythm skeleton, career focus, life areas |
| **Private actions / plans** | `private/actions/`, `private/state/` | **ignored** | Local-only action logs, generated daily plans |
| **Local data copies** | `data/local/`, `data/private/` | **ignored** | Optional mirrors of life-os / collab inputs; **activity/log SQLite** at `data/local/activity.sqlite` (SoT for durable events — never push) |
| **Public code + events** | `src/`, `bin/`, `public/events/` | tracked | Swarm logic, schemas, example public events |

## Rules

1. **Default deny** — actions are private/non-pushable until classified public.
2. **Finance area** is always local-only (details never in public plans beyond placeholders).
3. **Keyword scrub** — medical, debt/bank, identity-document patterns force private.
4. **HITL** — external email, job submit, calendar mutate, finance transfer, shared git push, publish-private require human pause (listed in daily plan). Optional cloud bridge ([EVE-FIT.md](EVE-FIT.md)): Eve channels may surface **redacted** turn digests and park tool approvals; **never** host full `private/persona` or finance/medical vaults on Eve by default.
5. **Sources** — `~/life-os/private/llm.txt` and collab-finder `data/` may be read/copied into ignored paths only; never committed.
6. **Activity store** — durable activities/logs live in local SQLite under `data/local/` (see [DECISIONS.md](DECISIONS.md) storage foundation). Browser IndexedDB / Eve / remote DBs are **not** sources of truth for private activity; export **redacted** IR only at channel boundaries.
7. **Shared micro-capture** (`~/.premflow/`, life-os `Projects/premflow/capture` symlink) — may contain finance, health, family, and other PII. **Local views OK.** Never commit capture contents; never paste raw todos/logs into `public/`, Eve digests, or shareable IR. Derived stats/insights use **redacted projection** (`projectCaptureForShare` / `flow path --json`). Law: [PREMFLOW-FIT.md](PREMFLOW-FIT.md).

## Enforcement

- `.gitignore` lists `private/`, `data/local/`, `data/private/`, `*.private.*`, `.env*`
- Runtime classifier: `src/privacy.js` (`classifyItem`, `partitionByVisibility`)
- Daily plan sections **Privacy split** + **HITL pause points**
- Verify: `git check-ignore -v private/persona/full.json`

## What may be pushed

- Swarm source, tests, docs, public persona projection, public event examples, fixtures that contain **no secrets**.
- Public sovereignty **gist** only (`public/thinking/`, `src/sovereignty-gist.js`) — not full private thinking packs under `private/thinking/`.
