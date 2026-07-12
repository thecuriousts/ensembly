/**
 * Canvas painter for WASM draw buffer — no world logic here.
 * Buffer ABI: see crates/peram-core/src/world.rs draw_buffer
 */

const KIND_AVATAR = 1;
const KIND_BEACON_DIGITAL = 2;
const KIND_BEACON_PHYSICAL = 3;
const KIND_BEACON_HITL = 4;
const KIND_BEACON_PHASE = 5;
const KIND_AGENT = 6;

const PROP_TREE = 1;
const PROP_LANTERN = 2;
const PROP_BENCH = 3;
const PROP_ARCH = 4;
const PROP_WELL = 5;
const PROP_BANNER = 6;
const PROP_STONE = 7;

const FLAG_FOCUSED = 1;

export function createWorldRenderer(canvas) {
  const ctx = canvas.getContext('2d');
  return {
    backend: 'canvas2d-wasm-buffer',
    /**
     * @param {number[]|Float32Array} buf
     * @param {string[]} labels
     * @param {{ growthMeter01?: number, completedSlots?: Set<number> }} [opts]
     */
    drawBuffer(buf, labels = [], opts = {}) {
      paint(ctx, canvas, buf, labels, opts);
    },
  };
}

function paint(ctx, canvas, buf, labels, opts = {}) {
  if (!buf || buf.length < 6) return;
  const growthMeter = Math.max(0, Math.min(1, opts.growthMeter01 || 0));
  const completedSlots = opts.completedSlots || new Set();
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const cssW = canvas.clientWidth || 960;
  const cssH = canvas.clientHeight || 540;
  if (canvas.width !== (cssW * dpr | 0) || canvas.height !== (cssH * dpr | 0)) {
    canvas.width = cssW * dpr | 0;
    canvas.height = cssH * dpr | 0;
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  const W = cssW;
  const H = cssH;

  const time = buf[0];
  const bw = buf[1] || 1600;
  const bh = buf[2] || 900;
  const entCount = buf[4] | 0;
  const propCount = buf[5] | 0;

  // environment (fixed night courtyard — palette could move to buffer later)
  const horizon = H * 0.42;
  const sky = ctx.createLinearGradient(0, 0, 0, horizon);
  sky.addColorStop(0, '#070b18');
  sky.addColorStop(1, '#1a1430');
  ctx.fillStyle = sky;
  ctx.fillRect(0, 0, W, horizon);

  ctx.fillStyle = 'rgba(220,230,255,0.8)';
  for (let i = 0; i < 40; i++) {
    const sx = ((i * 97) % 1000) / 1000 * W;
    const sy = ((i * 53) % 1000) / 1000 * horizon * 0.85;
    ctx.globalAlpha = 0.35 + (i % 5) * 0.1;
    ctx.fillRect(sx, sy, i % 3 === 0 ? 2 : 1, i % 3 === 0 ? 2 : 1);
  }
  ctx.globalAlpha = 1;

  // moon
  ctx.fillStyle = 'rgba(255,245,220,0.95)';
  ctx.shadowColor = 'rgba(255,240,200,0.45)';
  ctx.shadowBlur = 18;
  ctx.beginPath();
  ctx.arc(W * 0.82, H * 0.14, 20, 0, Math.PI * 2);
  ctx.fill();
  ctx.shadowBlur = 0;
  ctx.fillStyle = '#070b18';
  ctx.beginPath();
  ctx.arc(W * 0.82 + 7, H * 0.14 - 2, 16, 0, Math.PI * 2);
  ctx.fill();

  // hills
  ctx.fillStyle = '#0e1520';
  ctx.beginPath();
  ctx.moveTo(0, horizon);
  for (let x = 0; x <= W; x += 40) {
    ctx.lineTo(x, horizon - 28 - Math.sin(x * 0.01) * 16);
  }
  ctx.lineTo(W, horizon);
  ctx.closePath();
  ctx.fill();

  // ground
  const g = ctx.createLinearGradient(0, horizon, 0, H);
  g.addColorStop(0, '#1c2820');
  g.addColorStop(1, '#12181a');
  ctx.fillStyle = g;
  ctx.fillRect(0, horizon, W, H - horizon);

  // path — growth embers along the road
  ctx.strokeStyle = '#2a2418';
  ctx.lineWidth = 34;
  ctx.lineCap = 'round';
  ctx.beginPath();
  ctx.moveTo(W * 0.5, H);
  ctx.quadraticCurveTo(W * 0.48, H * 0.75, W * 0.5, horizon + 28);
  ctx.stroke();
  if (growthMeter > 0.02) {
    ctx.strokeStyle = `rgba(255, 120, 60, ${0.25 + growthMeter * 0.55})`;
    ctx.lineWidth = 6 + growthMeter * 10;
    ctx.shadowColor = '#ff6a30';
    ctx.shadowBlur = 12 + growthMeter * 20;
    ctx.beginPath();
    ctx.moveTo(W * 0.5, H);
    ctx.quadraticCurveTo(W * 0.48, H * 0.75, W * 0.5, horizon + 28);
    ctx.stroke();
    ctx.shadowBlur = 0;
  }

  const scale = Math.min(W / bw, H / bh) * 1.05;
  const ox = (W - bw * scale) / 2;
  const oy = (H - bh * scale) / 2 + H * 0.02;
  const wx = (x) => ox + x * scale;
  const wy = (y) => oy + y * scale;

  let o = 6;
  // entities first pass — props after in buffer order; props are after entities
  const ents = [];
  for (let i = 0; i < entCount; i++) {
    ents.push({
      kind: buf[o++] | 0,
      flags: buf[o++] | 0,
      x: buf[o++],
      y: buf[o++],
      slot: buf[o++] | 0,
      hash: buf[o++] | 0,
    });
  }
  const props = [];
  for (let i = 0; i < propCount; i++) {
    props.push({
      kind: buf[o++] | 0,
      x: buf[o++],
      y: buf[o++],
      scale: buf[o++],
    });
  }

  for (const p of props) drawProp(ctx, p, wx, wy, scale);
  for (const e of ents) {
    const label = e.slot >= 0 && labels[e.slot] ? labels[e.slot] : '';
    const done = e.slot >= 0 && completedSlots.has(e.slot);
    drawEntity(ctx, e, wx, wy, scale, time, label, done);
  }

  // growth ring (top-right of world, not HUD)
  drawGrowthRing(ctx, W, H, growthMeter);
}

function drawGrowthRing(ctx, W, H, meter) {
  const cx = W - 52;
  const cy = H - 52;
  const r = 28;
  ctx.beginPath();
  ctx.arc(cx, cy, r, 0, Math.PI * 2);
  ctx.strokeStyle = 'rgba(40,50,70,0.85)';
  ctx.lineWidth = 6;
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(cx, cy, r, -Math.PI / 2, -Math.PI / 2 + meter * Math.PI * 2);
  ctx.strokeStyle = meter > 0.66 ? '#3dff9a' : meter > 0.33 ? '#ffb060' : '#ff7a45';
  ctx.lineWidth = 6;
  ctx.lineCap = 'round';
  ctx.stroke();
  ctx.fillStyle = 'rgba(230,236,250,0.9)';
  ctx.font = '600 11px system-ui,sans-serif';
  ctx.textAlign = 'center';
  ctx.fillText(`${Math.round(meter * 100)}%`, cx, cy + 4);
  ctx.textAlign = 'left';
}

function drawProp(ctx, p, wx, wy, worldScale) {
  const x = wx(p.x);
  const y = wy(p.y);
  const s = (p.scale || 1) * worldScale;
  switch (p.kind) {
    case PROP_TREE:
      ctx.fillStyle = '#3a2818';
      ctx.fillRect(x - 4 * s, y - 20 * s, 8 * s, 24 * s);
      ctx.fillStyle = '#1a4030';
      ctx.beginPath();
      ctx.ellipse(x, y - 36 * s, 22 * s, 28 * s, 0, 0, Math.PI * 2);
      ctx.fill();
      break;
    case PROP_LANTERN:
      ctx.fillStyle = '#2a2a30';
      ctx.fillRect(x - 2 * s, y - 28 * s, 4 * s, 28 * s);
      ctx.fillStyle = '#ffb060';
      ctx.shadowColor = '#ff8a40';
      ctx.shadowBlur = 14 * s;
      ctx.beginPath();
      ctx.arc(x, y - 33 * s, 5 * s, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;
      break;
    case PROP_BENCH:
      ctx.fillStyle = '#4a3828';
      ctx.fillRect(x - 22 * s, y - 10 * s, 44 * s, 6 * s);
      ctx.fillRect(x - 20 * s, y - 4 * s, 5 * s, 10 * s);
      ctx.fillRect(x + 15 * s, y - 4 * s, 5 * s, 10 * s);
      break;
    case PROP_ARCH:
      ctx.strokeStyle = '#5a4a38';
      ctx.lineWidth = 8 * s;
      ctx.beginPath();
      ctx.moveTo(x - 50 * s, y);
      ctx.lineTo(x - 50 * s, y - 70 * s);
      ctx.quadraticCurveTo(x, y - 110 * s, x + 50 * s, y - 70 * s);
      ctx.lineTo(x + 50 * s, y);
      ctx.stroke();
      break;
    case PROP_WELL:
      ctx.fillStyle = '#3a3a42';
      ctx.beginPath();
      ctx.ellipse(x, y, 18 * s, 8 * s, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = '#2a2a32';
      ctx.fillRect(x - 16 * s, y - 14 * s, 32 * s, 14 * s);
      break;
    case PROP_BANNER:
      ctx.fillStyle = '#3a2818';
      ctx.fillRect(x - 2 * s, y - 50 * s, 4 * s, 50 * s);
      ctx.fillStyle = '#c04030';
      ctx.beginPath();
      ctx.moveTo(x, y - 48 * s);
      ctx.lineTo(x + 28 * s, y - 40 * s);
      ctx.lineTo(x, y - 32 * s);
      ctx.fill();
      break;
    case PROP_STONE:
      ctx.fillStyle = '#4a4840';
      ctx.beginPath();
      ctx.ellipse(x, y, 10 * s, 5 * s, 0, 0, Math.PI * 2);
      ctx.fill();
      break;
    default:
      break;
  }
}

function drawEntity(ctx, e, wx, wy, worldScale, time, label, done = false) {
  const x = wx(e.x);
  const y = wy(e.y);
  const focused = (e.flags & FLAG_FOCUSED) !== 0;
  const s = worldScale * (focused ? 1.12 : 1);
  const bob = Math.sin(time * 0.08 + e.x * 0.01) * 2;

  if (done && e.kind !== KIND_AVATAR) {
    ctx.strokeStyle = 'rgba(61,255,154,0.55)';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.ellipse(x, y + 4, 18 * s, 7 * s, 0, 0, Math.PI * 2);
    ctx.stroke();
  }

  if (focused) {
    ctx.strokeStyle = 'rgba(124,240,255,0.7)';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.ellipse(x, y + 4, 22 * s, 8 * s, 0, 0, Math.PI * 2);
    ctx.stroke();
  }

  if (e.kind === KIND_AVATAR) {
    // walker
    ctx.fillStyle = '#1a3a50';
    ctx.beginPath();
    ctx.moveTo(x, y - 28 * s + bob);
    ctx.lineTo(x - 12 * s, y + bob);
    ctx.lineTo(x + 12 * s, y + bob);
    ctx.fill();
    ctx.fillStyle = '#6ef0ff';
    ctx.fillRect(x - 5 * s, y - 24 * s + bob, 10 * s, 16 * s);
    ctx.fillStyle = '#e8c4a8';
    ctx.beginPath();
    ctx.arc(x, y - 30 * s + bob, 7 * s, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#ff8a4a';
    ctx.fillRect(x - 5 * s, y - 18 * s + bob, 10 * s, 3 * s);
  } else if (e.kind === KIND_AGENT) {
    ctx.fillStyle = '#d070ff';
    ctx.shadowBlur = 10;
    ctx.shadowColor = '#d070ff';
    ctx.beginPath();
    ctx.arc(x, y - 10 * s + bob, 10 * s, 0, Math.PI * 2);
    ctx.fill();
    ctx.shadowBlur = 0;
  } else {
    // beacon
    const colors =
      e.kind === KIND_BEACON_PHYSICAL
        ? { core: '#3dff9a', glow: '#6dffa8', base: '#0f3a2a' }
        : e.kind === KIND_BEACON_HITL
          ? { core: '#ff7a45', glow: '#ffb080', base: '#4a1c12' }
          : e.kind === KIND_BEACON_PHASE
            ? { core: '#8aa4ff', glow: '#b0c4ff', base: '#1a2860' }
            : { core: '#5a8cff', glow: '#6ef0ff', base: '#1a2744' };
    ctx.fillStyle = colors.base;
    ctx.beginPath();
    ctx.ellipse(x, y, 14 * s, 5 * s, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillRect(x - 4 * s, y - 36 * s, 8 * s, 36 * s);
    ctx.fillStyle = colors.glow;
    ctx.globalAlpha = focused ? 0.55 : 0.3;
    ctx.shadowColor = colors.glow;
    ctx.shadowBlur = focused ? 22 : 12;
    ctx.beginPath();
    ctx.arc(x, y - 40 * s + bob, 12 * s, 0, Math.PI * 2);
    ctx.fill();
    ctx.globalAlpha = 1;
    ctx.fillStyle = colors.core;
    ctx.beginPath();
    ctx.arc(x, y - 40 * s + bob, 7 * s, 0, Math.PI * 2);
    ctx.fill();
    ctx.shadowBlur = 0;
  }

  if (label) {
    ctx.fillStyle = done ? '#3dff9a' : focused ? '#7cf0ff' : 'rgba(230,236,250,0.9)';
    ctx.font = `600 ${11 * Math.max(worldScale, 0.75)}px system-ui,sans-serif`;
    ctx.textAlign = 'center';
    const mark = done ? '✓ ' : '';
    const raw = `${mark}${label}`;
    const t = raw.length > 20 ? `${raw.slice(0, 19)}…` : raw;
    ctx.fillText(t, x, y - 52 * s);
    ctx.textAlign = 'left';
  }
}
