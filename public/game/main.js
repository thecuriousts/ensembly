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

/** Host chrome — off by default so the world is the product */
const chrome = {
  bars: false,
  board: false,
  menu: false,
};

const CHROME_ACTIONS = new Set([
  'TOGGLE_BOARD',
  'TOGGLE_BARS',
  'TOGGLE_MENU',
  'CLOSE_CHROME',
  'CLOSE_HELP',
]);

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
  const biome = $('biome');
  if (biome) {
    biome.textContent = useWasmWorld
      ? 'Night Courtyard · growth run'
      : 'Fallback — run npm run build:wasm';
  }

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
  wirePanelCloses();
  applyChrome();
  scheduleHud();
  fadeEdgeHint();
  loop();
  paintHud();

  setText('status', useWasmWorld
    ? `wasm · ${worldEntityCount()} ents · B board · I bars · M menu`
    : `js-only · ${eng.error || 'no wasm'}`);
  setText('backend', renderer.backend);
  setText('engine', eng.mode);

  window.__PERAM__ = {
    getView: () => store.view,
    getSession: () => store.session,
    chrome: () => ({ ...chrome }),
    wasm: () => useWasmWorld,
    worldFocusSlot: () => (useWasmWorld ? worldFocusSlot() : store.session.focusIndex),
    injectKey: (ev) => {
      handleKey(ev);
      return store.view;
    },
    engineMode,
  };
}

function setText(id, text) {
  const el = $(id);
  if (el) el.textContent = text;
}

function fadeEdgeHint() {
  const hint = $('edge-hint');
  if (!hint) return;
  setTimeout(() => hint.classList.add('faded'), 4500);
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

  if (CHROME_ACTIONS.has(action.type) || action.type.startsWith('TOGGLE_')) {
    handleChrome(action);
    return;
  }

  if (action.type === 'UNDO') store.undo();
  else if (action.type === 'REDO') store.redo();
  else store.dispatch(action);

  if (action.type === 'VOICE_START') startSpeech();
  if (action.type === 'VOICE_STOP') stopSpeech();
}

function handleChrome(action) {
  switch (action.type) {
    case 'TOGGLE_BOARD':
      chrome.board = !chrome.board;
      break;
    case 'TOGGLE_BARS':
      chrome.bars = !chrome.bars;
      break;
    case 'TOGGLE_MENU':
      chrome.menu = !chrome.menu;
      if (chrome.menu) {
        queueMicrotask(() => $('command')?.focus());
      }
      break;
    case 'CLOSE_CHROME':
    case 'CLOSE_HELP':
      chrome.bars = false;
      chrome.board = false;
      chrome.menu = false;
      if (store.session.helpOpen) {
        store.dispatch({ type: 'CLOSE_HELP' });
      }
      if (store.session.voiceListening) {
        stopSpeech();
        store.dispatch({ type: 'VOICE_STOP' });
      }
      break;
    case 'TOGGLE_HELP':
      store.dispatch({ type: 'TOGGLE_HELP' });
      break;
    default:
      return;
  }
  applyChrome();
  hudDirty = true;
}

function applyChrome() {
  const body = document.body;
  body.dataset.bars = chrome.bars ? '1' : '0';
  body.dataset.board = chrome.board ? '1' : '0';
  body.dataset.menu = chrome.menu ? '1' : '0';

  const top = $('topbar');
  const board = $('board');
  const menu = $('menubar');
  const hud = $('hud');
  if (top) top.hidden = !chrome.bars;
  if (board) board.hidden = !chrome.board;
  if (menu) menu.hidden = !chrome.menu;
  if (hud) {
    const any = chrome.bars || chrome.board || chrome.menu;
    hud.setAttribute('aria-hidden', any ? 'false' : 'true');
  }
}

function handleKey(ev) {
  if (!store) return;
  if (ev.target?.id === 'command' || ev.target?.tagName === 'INPUT') {
    if (ev.key === 'Escape') {
      ev.preventDefault();
      ev.target.blur();
      runAction({ type: 'CLOSE_CHROME' });
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

function wirePanelCloses() {
  document.querySelectorAll('[data-close]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const which = btn.getAttribute('data-close');
      if (which === 'bars') chrome.bars = false;
      else if (which === 'board') chrome.board = false;
      else if (which === 'menu') chrome.menu = false;
      else runAction({ type: 'CLOSE_CHROME' });
      applyChrome();
      hudDirty = true;
    });
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
  const msgEl = $('msg');
  if (msgEl) {
    msgEl.textContent = isFocusEcho ? '' : msg;
    msgEl.hidden = isFocusEcho || !msg;
  }

  setText('focus', v.focusLabel || '—');
  setText('pending', String(v.pendingOpen));
  $('btn-voice')?.classList.toggle('active', v.voiceListening);

  setText('g-level', `Lv ${g.level || 1} ${g.title || 'Ember'}`);
  setText('g-xp', `${g.xp || 0} XP`);
  setText('g-streak', `streak ${g.streak || 0}`);
  $('g-streak')?.classList.toggle('hot', (g.streak || 0) >= 2);
  setText('g-quests', `${g.questsDone || 0}/${g.questsTotal || 0}`);
  setText('g-meter', `${Math.round((g.growthMeter01 || 0) * 100)}%`);

  const fill = $('xp-fill');
  if (fill) fill.style.width = `${Math.round((g.progress01 || 0) * 100)}%`;

  const chipXp = $('chip-xp');
  if (chipXp) {
    if (g.lastGain && performance.now() < toastUntil + 400) {
      chipXp.hidden = false;
      chipXp.textContent = `+${g.lastGain.amount}`;
    } else if ((g.xp || 0) > 0) {
      chipXp.hidden = false;
      chipXp.textContent = `${g.xp} XP`;
    } else {
      chipXp.hidden = true;
    }
  }

  const coach = $('coach');
  if (coach) coach.textContent = growthCoachLine(g);

  if (chrome.board) {
    paintBalance(g.balance || {});
    paintQuests(g.quests || [], session);
    paintRadar(session);
  }

  const live = $('focus-live');
  if (live) {
    live.textContent = v.focusLabel
      ? `Focused: ${v.focusLabel}${v.focusRole ? ` (${v.focusRole})` : ''}`
      : '';
  }

  const dlg = $('help');
  if (dlg) {
    if (session.helpOpen) {
      setText('help-body', [...helpLines(), '', ...voiceVocabulary()].join('\n'));
      if (!dlg.open) dlg.showModal();
    } else if (dlg.open) dlg.close();
  }
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
  if (store?.session.helpOpen) store.dispatch({ type: 'CLOSE_HELP' });
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
