/**
 * Thin host shell — input + paint only.
 * Focus SoT: JS session store. WASM world only mirrors focusIndex.
 */
import { mapKeyEvent, helpLines } from '/src/game/input.js';
import {
  parseVoiceCommand,
  voiceVocabulary,
  speechRecognitionAvailable,
} from '/src/game/voice.js';
import { createGameStore } from '/src/game/store.js';
import {
  initEngine,
  engineMode,
  worldBindGraph,
  worldSetFocus,
  worldFocusSlot,
  worldTick,
  worldDrawBuffer,
  worldEntityCount,
  worldPropCount,
  worldReset,
} from '/game/engine.js';
import { createWorldRenderer } from '/game/world-render.js';

const $ = (id) => document.getElementById(id);

let store = null;
let labels = [];
let renderer = null;
let recognition = null;
let hudDirty = true;
let useWasmWorld = false;

/** Mirror session focus → WASM only (never advance WASM independently). */
export function syncWorldFocusFromSession(session, setFocus = worldSetFocus) {
  if (!session || typeof setFocus !== 'function') return;
  const idx = Number(session.focusIndex);
  if (!Number.isFinite(idx) || idx < 0) return;
  setFocus(idx | 0);
}

async function boot() {
  const graph = await loadGraph();
  labels = (graph.nodes || []).map((n) => n.label || n.id);
  store = createGameStore(graph, { pending: graph.pending || [] });

  const eng = await initEngine();
  useWasmWorld = eng.mode === 'wasm';
  if (useWasmWorld) {
    worldReset(1600, 900);
    const bound = worldBindGraph(graph.nodes || []);
    useWasmWorld = bound;
    if (useWasmWorld) {
      syncWorldFocusFromSession(store.session);
    }
  }

  renderer = createWorldRenderer($('stage'));
  $('backend').textContent = renderer.backend;
  $('engine').textContent = eng.mode;
  $('biome').textContent = useWasmWorld ? 'Night Courtyard (WASM)' : 'Fallback — run npm run build:wasm';

  store.subscribe(() => {
    hudDirty = true;
    if (useWasmWorld) syncWorldFocusFromSession(store.session);
  });

  wireInput();
  wireVoice();
  wireCommand();
  wireRadar();
  scheduleHud();
  loop();
  paintHud(); // first paint

  $('status').textContent = useWasmWorld
    ? `wasm world · ${worldEntityCount()} ents · ${worldPropCount()} props · focus synced`
    : `js-only fallback · ${eng.error || 'no wasm'}`;

  window.__PERAM__ = {
    getView: () => store.view,
    getSession: () => store.session,
    wasm: () => useWasmWorld,
    worldFocusSlot: () => (useWasmWorld ? worldFocusSlot() : store.session.focusIndex),
    injectKey: (ev) => {
      handleKey(ev);
      return store.view;
    },
    engineMode,
  };
}

async function loadGraph() {
  try {
    const res = await fetch('/game/sample-graph.json', { cache: 'no-store' });
    return await res.json();
  } catch {
    return {
      nodes: [
        { id: 'a', type: 'action', label: 'Ship' },
        { id: 'b', type: 'physical', realm: 'physical', label: 'Garden' },
        { id: 'c', type: 'hitl', hitl: true, label: 'HITL gate' },
      ],
      pending: [{ id: 'auth-x', title: 'Gate', status: 'pending' }],
    };
  }
}

function runAction(action) {
  if (!action || !store) return;
  // Session is sole navigator. WASM follows via store.subscribe → syncWorldFocusFromSession.
  if (action.type === 'UNDO') store.undo();
  else if (action.type === 'REDO') store.redo();
  else store.dispatch(action);

  if (action.type === 'VOICE_START') startSpeech();
  if (action.type === 'VOICE_STOP') stopSpeech();
}

function handleKey(ev) {
  if (!store) return;
  if (ev.target?.id === 'command' || ev.target?.tagName === 'INPUT') {
    if (ev.key === 'Escape') {
      ev.target.blur();
      runAction({ type: 'CLOSE_HELP' });
    }
    return;
  }
  if (ev.key === '?' || (ev.key === '/' && ev.shiftKey)) {
    ev.preventDefault();
    runAction({ type: 'TOGGLE_HELP' });
    return;
  }
  if ((ev.key === 'u' || ev.key === 'U') && !ev.ctrlKey && !ev.metaKey) {
    ev.preventDefault();
    runAction({ type: 'UNDO' });
    return;
  }
  if ((ev.key === 'r' || ev.key === 'R') && !ev.ctrlKey && !ev.metaKey) {
    ev.preventDefault();
    runAction({ type: 'REDO' });
    return;
  }
  const action = mapKeyEvent(ev, { voiceListening: store.session.voiceListening });
  if (!action) return;
  ev.preventDefault();
  if (action.type === 'FOCUS_RIGHT' || action.type === 'FOCUS_DOWN') {
    runAction({ type: 'FOCUS_NEXT' });
  } else if (action.type === 'FOCUS_LEFT' || action.type === 'FOCUS_UP') {
    runAction({ type: 'FOCUS_PREV' });
  } else {
    runAction(action);
  }
}

function wireInput() {
  window.addEventListener('keydown', handleKey);
}

function wireVoice() {
  $('voice-avail').textContent = speechRecognitionAvailable()
    ? 'Voice ready'
    : 'Type commands';
  $('btn-voice').addEventListener('click', () => {
    if (store.session.voiceListening) {
      stopSpeech();
      runAction({ type: 'VOICE_STOP' });
    } else {
      runAction({ type: 'VOICE_START' });
      startSpeech();
    }
  });
}

function wireCommand() {
  $('voice-form').addEventListener('submit', (ev) => {
    ev.preventDefault();
    const input = $('command');
    const text = input.value;
    input.value = '';
    for (const a of parseVoiceCommand(text).actions) runAction(a);
  });
}

function wireRadar() {
  $('radar')?.addEventListener('click', (ev) => {
    const li = ev.target.closest('li[data-index]');
    if (!li) return;
    const idx = Number(li.dataset.index);
    runAction({ type: 'FOCUS_INDEX', payload: { index: idx } });
    runAction({ type: 'SELECT' });
  });
}

function startSpeech() {
  const SR = window.SpeechRecognition || window.webkitSpeechRecognition;
  if (!SR) return;
  try {
    recognition = new SR();
    recognition.lang = 'en-US';
    recognition.onresult = (ev) => {
      for (const a of parseVoiceCommand(ev.results?.[0]?.[0]?.transcript || '').actions) {
        runAction(a);
      }
      runAction({ type: 'VOICE_STOP' });
    };
    recognition.onerror = () => runAction({ type: 'VOICE_STOP' });
    recognition.start();
  } catch {
    runAction({ type: 'VOICE_STOP' });
  }
}

function stopSpeech() {
  try {
    recognition?.stop();
  } catch {
    /* */
  }
}

function scheduleHud() {
  const tick = () => {
    if (hudDirty && store) {
      hudDirty = false;
      paintHud();
    }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

function paintHud() {
  const v = store.view;
  const session = store.session;
  // Action/result channel only — not a second focus title
  const msg = v.message || '';
  const isFocusEcho = /^Focus:/i.test(msg);
  $('msg').textContent = isFocusEcho ? '' : msg;
  $('msg').hidden = isFocusEcho || !msg;

  $('focus').textContent = v.focusLabel || '—';
  $('pending').textContent = String(v.pendingOpen);
  $('mode').textContent = v.mode;
  $('tick').textContent = String(v.tick);
  $('btn-voice').classList.toggle('active', v.voiceListening);

  // Live region for a11y (not a big side headline)
  const live = $('focus-live');
  if (live) live.textContent = v.focusLabel ? `Focused: ${v.focusLabel}` : '';

  const radar = $('radar');
  if (radar) {
    const frag = document.createDocumentFragment();
    session.nodes.forEach((n, i) => {
      const li = document.createElement('li');
      li.dataset.index = String(i);
      li.dataset.focus = i === session.focusIndex ? '1' : '0';
      const tag =
        n.type === 'physical' || n.realm === 'physical'
          ? 'PHYS'
          : n.type === 'hitl' || n.hitl
            ? 'HITL'
            : n.type === 'phase'
              ? 'PHASE'
              : 'DIG';
      li.innerHTML = `<span class="idx">${String(i + 1).padStart(2, '0')}</span><span class="name"></span><span class="tag"></span>`;
      li.querySelector('.name').textContent = n.label || n.id;
      li.querySelector('.tag').textContent = tag;
      frag.appendChild(li);
    });
    radar.replaceChildren(frag);
  }

  const dlg = $('help');
  if (session.helpOpen) {
    $('help-body').textContent = [...helpLines(), '', ...voiceVocabulary()].join('\n');
    if (!dlg.open) dlg.showModal();
  } else if (dlg.open) dlg.close();
}

$('help')?.addEventListener('close', () => {
  if (store?.session.helpOpen) runAction({ type: 'CLOSE_HELP' });
});

function loop() {
  const frame = () => {
    if (useWasmWorld) {
      worldTick(5);
      const buf = worldDrawBuffer();
      renderer?.drawBuffer(buf, labels);
    }
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

boot().catch((err) => {
  console.error(err);
  const s = $('status');
  if (s) s.textContent = `boot: ${err.message || err}`;
});
