# Patterns.dev skills in ensembly

**Catalog:** [patterns.dev/ai/skills](https://www.patterns.dev/ai/skills/)  
**Install source:** [PatternsDev/skills](https://github.com/PatternsDev/skills)

## Installed here

JavaScript skill pack under `.agents/skills/` (local to this repo), including:

| Skill | Use in Game of Peram |
|-------|----------------------|
| **command-pattern** | `src/game/commands.js` — execute + undo/redo history |
| **observer-pattern** | `src/game/observable.js` + `store.subscribe` |
| **mediator-pattern** | `src/game/store.js` — single bus for session/commands/UI |
| **module-pattern** | `src/game/*` ESM modules |
| **js-performance-patterns** | Map lookups, rAF HUD batching, loop caches, DocumentFragment radar |
| **loading-sequence** | `modulepreload` / `preload` in `public/game/index.html` |

## Re-install

```bash
cd /path/to/ensembly
npx skills add PatternsDev/skills/javascript
# or one skill:
npx skills add PatternsDev/skills --skill command-pattern
```

## Code map

```text
src/game/session.js      pure state machine (immutable dispatch)
src/game/commands.js     Command pattern + history
src/game/observable.js   Observer
src/game/store.js        Mediator store
public/game/main.js      shell: subscribe → rAF paint
```

Not affiliated with Patterns.dev — skills used under their MIT skill licenses.
