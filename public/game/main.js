/**
 * Thin host shell — input + paint only.
 * Focus SoT: JS session store. WASM world only mirrors focusIndex.
 * Growth HUD: XP, quests, balance — gamifies real progress.
 */
import { mapKeyEvent, helpLines } from '/src/game/input.js';
import {
  parseVoiceCommand,
  voiceVocabulary,
  speechRecognitionAvailable,
} from '/src/game/voice.js';
import { createGameStore } from '/src/game/store.js';
import { growthCoachLine } from '/src/game/growth.js';
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
let lastToastTick = -1;
let toastUntil = 0;

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
  $('biome').textContent = useWasmWorld
    ? 'Night Courtyard · growth run'
    : 'Fallback — run npm run build:wasm';

  store.subscribe(() => {
    hudDirty = true;
    if (useWasmWorld) syncWorldFocusFromSession(store.session);
    maybeToast(store.view);
  });

  wireInput();
  wireVoice();
  wireCommand();
  wireRadar();
  wireQuests();
  scheduleHud();
  loop();
  paintHud();

  $('status').textContent = useWasmWorld
    ? `wasm · ${worldEntityCount()} ents · claim beacons for XP · clear HITL gates`
    : `js-only · ${eng.error || 'no wasm'}`;

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
        { id: 'a', type: 'action', label: 'Ship craft', area: 'craft' },
        { id: 'b', type: 'physical', realm: 'physical', label: 'Garden' },
        { id: 'c', type: 'schedule', label: 'Family walk', area: 'family' },
        { id: 'd', type: 'hitl', hitl: true, label: 'HITL gate' },
      ],
      pending: [{ id: 'auth-x', title: 'Gate', status: 'pending' }],
    };
  }
}

function runAction(action) {
  if (!action || !store) return;
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
    ? 'Voice ready · say claim / approve / next'
    : 'Type: claim · approve · next';
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

function wireQuests() {
  $('quests')?.addEventListener('click', (ev) => {
    const li = ev.target.closest('li[data-node-id]');
    if (!li || !store) return;
    const id = li.dataset.nodeId;
    const idx = store.session.nodes.findIndex((n) => n.id === id);
    if (idx < 0) return;
    runAction({ type: 'FOCUS_INDEX', payload: { index: idx } });
    const role = li.dataset.role;
    if (role === 'hitl') {
      /* focus only — A/D to resolve */
    } else {
      runAction({ type: 'COMPLETE' });
    }
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

function maybeToast(view) {
  const gain = view?.growth?.lastGain;
  if (!gain || gain.tick === lastToastTick) return;
  lastToastTick = gain.tick;
  const el = $('toast');
  if (!el) return;
  el.hidden = false;
  el.textContent = `+${gain.amount} XP · ${gain.reason.replace(/^(claim|hitl):/, '')}`;
  el.classList.add('show');
  toastUntil = performance.now() + 2200;
  const ember = $('ember-pulse');
  ember?.classList.add('burst');
  setTimeout(() => ember?.classList.remove('burst'), 600);
}

function scheduleHud() {
  const tick = () => {
    if (hudDirty && store) {
      hudDirty = false;
      paintHud();
    }
    const toast = $('toast');
    if (toast && !toast.hidden && performance.now() > toastUntil) {
      toast.hidden = true;
      toast.classList.remove('show');
    }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

function paintHud() {
  const v = store.view;
  const session = store.session;
  const g = v.growth || {};

  const msg = v.message || '';
  const isFocusEcho = /^Focus:/i.test(msg);
  $('msg').textContent = isFocusEcho ? '' : msg;
  $('msg').hidden = isFocusEcho || !msg;

  $('focus').textContent = v.focusLabel || '—';
  $('pending').textContent = String(v.pendingOpen);
  $('btn-voice').classList.toggle('active', v.voiceListening);

  $('g-level').textContent = `Lv ${g.level || 1} ${g.title || 'Ember'}`;
  $('g-xp').textContent = `${g.xp || 0} XP`;
  $('g-streak').textContent = `streak ${g.streak || 0}`;
  $('g-streak').classList.toggle('hot', (g.streak || 0) >= 2);
  $('g-quests').textContent = `${g.questsDone || 0}/${g.questsTotal || 0}`;
  $('g-meter').textContent = `${Math.round((g.growthMeter01 || 0) * 100)}%`;

  const fill = $('xp-fill');
  if (fill) fill.style.width = `${Math.round((g.progress01 || 0) * 100)}%`;

  const coach = $('coach');
  if (coach) coach.textContent = growthCoachLine(g);

  paintBalance(g.balance || {});
  paintQuests(g.quests || [], session);
  paintRadar(session);

  const live = $('focus-live');
  if (live) {
    live.textContent = v.focusLabel
      ? `Focused: ${v.focusLabel}${v.focusRole ? ` (${v.focusRole})` : ''}`
      : '';
  }

  const dlg = $('help');
  if (session.helpOpen) {
    $('help-body').textContent = [...helpLines(), '', ...voiceVocabulary()].join('\n');
    if (!dlg.open) dlg.showModal();
  } else if (dlg.open) dlg.close();
}

function paintBalance(balance) {
  const row = $('balance-row');
  if (!row) return;
  const axes = [
    { key: 'physical', label: 'Body' },
    { key: 'presence', label: 'Presence' },
    { key: 'craft', label: 'Craft' },
    { key: 'hitl', label: 'Gates' },
  ];
  const frag = document.createDocumentFragment();
  for (const a of axes) {
    const n = balance[a.key] || 0;
    const chip = document.createElement('span');
    chip.className = `bal-chip${n > 0 ? ' on' : ''}`;
    chip.dataset.axis = a.key;
    chip.textContent = `${a.label} ${n}`;
    frag.appendChild(chip);
  }
  row.replaceChildren(frag);
}

function paintQuests(quests, session) {
  const list = $('quests');
  if (!list) return;
  const frag = document.createDocumentFragment();
  for (const q of quests) {
    const li = document.createElement('li');
    li.dataset.nodeId = q.id;
    li.dataset.role = q.role;
    li.dataset.status = q.status;
    const focused = session.nodes[session.focusIndex]?.id === q.id;
    if (focused) li.dataset.focus = '1';
    li.innerHTML =
      '<span class="q-mark"></span><span class="q-name"></span><span class="q-xp"></span>';
    li.querySelector('.q-mark').textContent = q.status === 'done' ? '✓' : '○';
    li.querySelector('.q-name').textContent = q.label;
    li.querySelector('.q-xp').textContent = q.status === 'done' ? 'done' : `+${q.xp}`;
    frag.appendChild(li);
  }
  list.replaceChildren(frag);
}

function paintRadar(session) {
  const radar = $('radar');
  if (!radar) return;
  const completed = new Set(session.growth?.completedIds || []);
  const frag = document.createDocumentFragment();
  session.nodes.forEach((n, i) => {
    const li = document.createElement('li');
    li.dataset.index = String(i);
    li.dataset.focus = i === session.focusIndex ? '1' : '0';
    if (completed.has(n.id) || n.status === 'done' || n.status === 'approved' || n.status === 'denied') {
      li.dataset.done = '1';
    }
    const tag =
      n.type === 'physical' || n.realm === 'physical'
        ? 'PHYS'
        : n.type === 'hitl' || n.hitl
          ? 'HITL'
          : n.type === 'schedule'
            ? 'PRES'
            : n.type === 'phase' || n.type === 'game'
              ? 'META'
              : 'CRAFT';
    li.innerHTML =
      '<span class="idx"></span><span class="name"></span><span class="tag"></span>';
    li.querySelector('.idx').textContent = String(i + 1).padStart(2, '0');
    li.querySelector('.name').textContent = n.label || n.id;
    li.querySelector('.tag').textContent = tag;
    frag.appendChild(li);
  });
  radar.replaceChildren(frag);
}

$('help')?.addEventListener('close', () => {
  if (store?.session.helpOpen) runAction({ type: 'CLOSE_HELP' });
});

function loop() {
  const frame = () => {
    if (useWasmWorld) {
      worldTick(5);
      const buf = worldDrawBuffer();
      const meter = store?.view?.growth?.growthMeter01 || 0;
      const completed = new Set(store?.session?.growth?.completedIds || []);
      renderer?.drawBuffer(buf, labels, { growthMeter01: meter, completedSlots: completedSlotSet(completed) });
    }
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

function completedSlotSet(completedIds) {
  if (!store) return new Set();
  const slots = new Set();
  store.session.nodes.forEach((n, i) => {
    if (completedIds.has(n.id) || n.status === 'done' || n.status === 'approved' || n.status === 'denied') {
      slots.add(i);
    }
  });
  return slots;
}

boot().catch((err) => {
  console.error(err);
  const s = $('status');
  if (s) s.textContent = `boot: ${err.message || err}`;
});
