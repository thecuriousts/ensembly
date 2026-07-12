# Privacy contract

Ensembly is a **life swarm control plane**. It may *read* private operator context on the local machine. It must **never** push private data to git remotes or treat private actions as public events.

## Split

| Layer | Location | Git | Contents |
|-------|----------|-----|----------|
| **Full persona** | `private/persona/` | **ignored** | Detailed operator model (family/health/finance-adjacent themes from local sources) |
| **Public projection** | `public/persona/` | tracked | Scrubbed principles, rhythm skeleton, career focus, life areas |
| **Private actions / plans** | `private/actions/`, `private/state/` | **ignored** | Local-only action logs, generated daily plans |
| **Local data copies** | `data/local/`, `data/private/` | **ignored** | Optional mirrors of life-os / collab inputs |
| **Public code + events** | `src/`, `bin/`, `public/events/` | tracked | Swarm logic, schemas, example public events |

## Rules

1. **Default deny** — actions are private/non-pushable until classified public.
2. **Finance area** is always local-only (details never in public plans beyond placeholders).
3. **Keyword scrub** — medical, debt/bank, identity-document patterns force private.
4. **HITL** — external email, job submit, calendar mutate, finance transfer, shared git push, publish-private require human pause (listed in daily plan).
5. **Sources** — `~/life-os/private/llm.txt` and collab-finder `data/` may be read/copied into ignored paths only; never committed.

## Enforcement

- `.gitignore` lists `private/`, `data/local/`, `data/private/`, `*.private.*`, `.env*`
- Runtime classifier: `src/privacy.js` (`classifyItem`, `partitionByVisibility`)
- Daily plan sections **Privacy split** + **HITL pause points**
- Verify: `git check-ignore -v private/persona/full.json`

## What may be pushed

- Swarm source, tests, docs, public persona projection, public event examples, fixtures that contain **no secrets**.
