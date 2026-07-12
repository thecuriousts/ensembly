/**
 * WebGPU primary + Canvas2D fallback — richer cockpit graph.
 */

export async function createRenderer(canvas) {
  if (navigator.gpu) {
    try {
      const adapter = await navigator.gpu.requestAdapter();
      if (adapter) {
        const device = await adapter.requestDevice();
        const context = canvas.getContext('webgpu');
        if (context) {
          const format = navigator.gpu.getPreferredCanvasFormat();
          context.configure({ device, format, alphaMode: 'premultiplied' });
          return {
            backend: 'webgpu',
            draw: (session) => drawWebGpu(device, context, format, canvas, session),
          };
        }
      }
    } catch {
      /* fall through */
    }
  }
  const ctx = canvas.getContext('2d');
  return {
    backend: 'canvas2d',
    draw: (session) => drawCanvas2d(ctx, canvas, session),
  };
}

function drawCanvas2d(ctx, canvas, session) {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = canvas.clientWidth || canvas.parentElement?.clientWidth || 800;
  const h = canvas.clientHeight || canvas.parentElement?.clientHeight || 500;
  if (canvas.width !== Math.floor(w * dpr) || canvas.height !== Math.floor(h * dpr)) {
    canvas.width = Math.floor(w * dpr);
    canvas.height = Math.floor(h * dpr);
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  // deep stage
  const g = ctx.createLinearGradient(0, 0, w, h);
  g.addColorStop(0, '#0a0e18');
  g.addColorStop(1, '#0d1424');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);

  // instrument grid
  ctx.strokeStyle = 'rgba(90, 140, 200, 0.06)';
  ctx.lineWidth = 1;
  for (let x = 0; x < w; x += 48) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, h);
    ctx.stroke();
  }
  for (let y = 0; y < h; y += 48) {
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(w, y);
    ctx.stroke();
  }

  const pad = 48;
  const nodes = session.nodes || [];
  const edges = session.edges || [];
  // Map for O(1) edge endpoint lookup (Patterns.dev performance)
  const byId =
    session.nodeById instanceof Map
      ? session.nodeById
      : new Map(nodes.map((n) => [n.id, n]));

  for (let ei = 0, elen = edges.length; ei < elen; ei++) {
    const e = edges[ei];
    const a = byId.get(e.source);
    const b = byId.get(e.target);
    if (!a || !b) continue;
    const x1 = pad + a.position.x + 70;
    const y1 = pad + a.position.y + 24;
    const x2 = pad + b.position.x + 70;
    const y2 = pad + b.position.y + 24;
    ctx.strokeStyle = 'rgba(110, 180, 255, 0.28)';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    const mx = (x1 + x2) / 2;
    ctx.bezierCurveTo(mx, y1, mx, y2, x2, y2);
    ctx.stroke();
  }

  const focusIndex = session.focusIndex;
  const selectedId = session.selectedId;
  for (let i = 0, nlen = nodes.length; i < nlen; i++) {
    const n = nodes[i];
    const x = pad + (n.position?.x || 0);
    const y = pad + (n.position?.y || 0);
    const focused = i === focusIndex;
    const selected = n.id === selectedId;
    const colors = colorForType(n.type, n.realm);

    if (focused) {
      ctx.shadowColor = 'rgba(110, 230, 255, 0.55)';
      ctx.shadowBlur = 18;
    } else {
      ctx.shadowBlur = 0;
    }

    ctx.fillStyle = colors.bg;
    ctx.strokeStyle = focused ? '#7cf0ff' : selected ? '#ff8a4a' : colors.bd;
    ctx.lineWidth = focused ? 2.5 : 1.25;
    roundRect(ctx, x, y, 168, 52, 12);
    ctx.fill();
    ctx.stroke();
    ctx.shadowBlur = 0;

    // type chip
    ctx.fillStyle = colors.chip;
    roundRect(ctx, x + 10, y + 8, 36, 14, 4);
    ctx.fill();
    ctx.fillStyle = '#0a0e14';
    ctx.font = '600 9px ui-monospace, monospace';
    ctx.fillText(colors.chipLabel, x + 14, y + 18);

    ctx.fillStyle = '#eef3ff';
    ctx.font = '600 12px system-ui, sans-serif';
    ctx.fillText(truncate(n.label || n.id, 20), x + 12, y + 38);
  }
}

function drawWebGpu(device, context, format, canvas, session) {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = canvas.clientWidth || 800;
  const h = canvas.clientHeight || 500;
  const bw = Math.max(1, Math.floor(w * dpr));
  const bh = Math.max(1, Math.floor(h * dpr));
  if (canvas.width !== bw || canvas.height !== bh) {
    canvas.width = bw;
    canvas.height = bh;
    context.configure({ device, format, alphaMode: 'premultiplied' });
  }

  const verts = [];
  const pad = 48 * dpr;
  const nodes = session.nodes || [];
  for (let i = 0; i < nodes.length; i++) {
    const n = nodes[i];
    const x = pad + (n.position?.x || 0) * dpr;
    const y = pad + (n.position?.y || 0) * dpr;
    const focused = i === session.focusIndex;
    const c = colorRgb(n.type, focused, n.realm);
    pushQuad(verts, x, y, 168 * dpr, 52 * dpr, bw, bh, c);
  }

  const encoder = device.createCommandEncoder();
  const view = context.getCurrentTexture().createView();
  const pass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view,
        clearValue: { r: 0.04, g: 0.055, b: 0.09, a: 1 },
        loadOp: 'clear',
        storeOp: 'store',
      },
    ],
  });

  if (verts.length > 0) {
    const pipeline = getOrCreatePipeline(device, format);
    const buf = device.createBuffer({
      size: verts.length * 4,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      mappedAtCreation: true,
    });
    new Float32Array(buf.getMappedRange()).set(verts);
    buf.unmap();
    pass.setPipeline(pipeline);
    pass.setVertexBuffer(0, buf);
    pass.draw(verts.length / 6, 1, 0, 0);
  }
  pass.end();
  device.queue.submit([encoder.finish()]);
}

let _pipeline = null;
function getOrCreatePipeline(device, format) {
  if (_pipeline) return _pipeline;
  const shader = device.createShaderModule({
    code: `
struct VOut {
  @builtin(position) pos: vec4f,
  @location(0) color: vec4f,
};
@vertex
fn vs(@location(0) pos: vec2f, @location(1) color: vec4f) -> VOut {
  var o: VOut;
  o.pos = vec4f(pos, 0.0, 1.0);
  o.color = color;
  return o;
}
@fragment
fn fs(i: VOut) -> @location(0) vec4f {
  return i.color;
}`,
  });
  _pipeline = device.createRenderPipeline({
    layout: 'auto',
    vertex: {
      module: shader,
      entryPoint: 'vs',
      buffers: [
        {
          arrayStride: 24,
          attributes: [
            { shaderLocation: 0, offset: 0, format: 'float32x2' },
            { shaderLocation: 1, offset: 8, format: 'float32x4' },
          ],
        },
      ],
    },
    fragment: {
      module: shader,
      entryPoint: 'fs',
      targets: [{ format }],
    },
    primitive: { topology: 'triangle-list' },
  });
  return _pipeline;
}

function pushQuad(verts, x, y, qw, qh, bw, bh, c) {
  const toClip = (px, py) => [(px / bw) * 2 - 1, 1 - (py / bh) * 2];
  const corners = [
    [x, y],
    [x + qw, y],
    [x + qw, y + qh],
    [x, y + qh],
  ].map(([px, py]) => toClip(px, py));
  for (const i of [0, 1, 2, 0, 2, 3]) {
    verts.push(corners[i][0], corners[i][1], c[0], c[1], c[2], c[3]);
  }
}

function colorRgb(type, focused, realm) {
  if (focused) return [0.45, 0.9, 1.0, 1.0];
  if (type === 'physical' || realm === 'physical') return [0.2, 0.65, 0.45, 0.95];
  if (type === 'hitl') return [0.9, 0.4, 0.22, 0.95];
  if (type === 'phase') return [0.3, 0.4, 0.85, 0.95];
  if (type === 'schedule') return [0.5, 0.35, 0.75, 0.95];
  if (type === 'game') return [0.55, 0.28, 0.45, 0.95];
  return [0.22, 0.32, 0.55, 0.95];
}

function colorForType(type, realm) {
  if (type === 'physical' || realm === 'physical') {
    return { bg: '#0f3a2a', bd: '#3dff9a', chip: '#3dff9a', chipLabel: 'PHYS' };
  }
  if (type === 'hitl') {
    return { bg: '#4a1c12', bd: '#ff7a45', chip: '#ff8a4a', chipLabel: 'HITL' };
  }
  if (type === 'phase') {
    return { bg: '#1a2860', bd: '#6a8cff', chip: '#8aa4ff', chipLabel: 'PHAS' };
  }
  if (type === 'schedule') {
    return { bg: '#2a1a48', bd: '#b48cff', chip: '#c4a0ff', chipLabel: 'TIME' };
  }
  if (type === 'game') {
    return { bg: '#3a1830', bd: '#e070a0', chip: '#ff90b8', chipLabel: 'ROOT' };
  }
  return { bg: '#152038', bd: '#5a8cff', chip: '#6ef0ff', chipLabel: 'DIG ' };
}

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

function truncate(s, n) {
  s = String(s);
  return s.length <= n ? s : `${s.slice(0, n - 1)}…`;
}
