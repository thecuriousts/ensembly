/**
 * $SPN — Game of Peram progress ticker (pure).
 * Stock-style quote + series from real growth events / gameplay outcomes.
 * Not market data; not random noise as the sole driver.
 */

/** Display + machine symbol */
export const SPN_SYMBOL = '$SPN';

/** IPO / session open */
export const SPN_BASE = 100;

/**
 * Map a single growth event to a signed price delta.
 * Up on claim/approve/combo progress; down on deny (and similar miss paths).
 * @param {{ amount?: number, reason?: string, role?: string }} event
 * @returns {number}
 */
export function eventToPriceDelta(event) {
  if (!event || typeof event !== 'object') return 0;
  const amount = Math.max(0, Number(event.amount) || 0);
  const reason = String(event.reason || '');
  const role = String(event.role || '');

  // Explicit miss / drag paths — sell the tape
  if (
    reason.includes('denied') ||
    reason.includes('deny') ||
    reason === 'drag:idle' ||
    reason === 'drag:imbalance'
  ) {
    // Deny still may have awarded modest XP; market prices the miss harder.
    const baseDrag = 2.4 + Math.min(6, amount * 0.05);
    return -round2(baseDrag);
  }

  if (amount <= 0) return 0;

  // Positive progress — scale by XP with mild role weights
  let weight = 0.12;
  if (role === 'physical') weight = 0.16;
  else if (role === 'presence') weight = 0.15;
  else if (role === 'hitl') weight = 0.13;
  else if (role === 'craft') weight = 0.1;

  if (reason.includes('approved')) weight += 0.02;
  if (reason.startsWith('claim:')) weight += 0.01;

  return round2(amount * weight);
}

/**
 * Build multi-point series from growth events + optional balance drag.
 * @param {object} growth emptyGrowth-shaped or session.growth
 * @param {{ base?: number, applyImbalanceDrag?: boolean }} [opts]
 * @returns {{
 *   symbol: string,
 *   base: number,
 *   series: number[],
 *   points: Array<{ t: number, price: number, delta: number, reason: string|null }>,
 *   price: number,
 *   change: number,
 *   changePct: number,
 *   direction: 'up' | 'down' | 'flat',
 * }}
 */
export function buildSpnQuote(growth = {}, opts = {}) {
  const base = Number.isFinite(opts.base) ? opts.base : SPN_BASE;
  const events = Array.isArray(growth.events) ? growth.events : [];
  const applyImbalance = opts.applyImbalanceDrag !== false;

  /** @type {Array<{ t: number, price: number, delta: number, reason: string|null }>} */
  const points = [{ t: 0, price: round2(base), delta: 0, reason: 'open' }];
  let price = base;

  events.forEach((ev, i) => {
    const delta = eventToPriceDelta(ev);
    price = clampPrice(price + delta);
    points.push({
      t: i + 1,
      price: round2(price),
      delta: round2(delta),
      reason: ev?.reason || null,
    });
  });

  // Soft drag when craft advanced without any physical/presence (imbalance)
  if (applyImbalance) {
    const b = growth.balance || {};
    const craftHeavy =
      (b.craft || 0) > 0 && (b.physical || 0) === 0 && (b.presence || 0) === 0;
    if (craftHeavy) {
      const delta = eventToPriceDelta({
        amount: (b.craft || 1) * 10,
        reason: 'drag:imbalance',
        role: 'meta',
      });
      price = clampPrice(price + delta);
      points.push({
        t: points.length,
        price: round2(price),
        delta: round2(delta),
        reason: 'drag:imbalance',
      });
    }
  }

  // Ensure at least open + one synthetic mark when empty (flat open)
  const series = points.map((p) => p.price);
  if (series.length < 2) {
    series.push(round2(base));
    points.push({ t: 1, price: round2(base), delta: 0, reason: 'flat' });
  }

  const last = series[series.length - 1];
  const change = round2(last - base);
  const changePct = base !== 0 ? round2((change / base) * 100) : 0;
  let direction = 'flat';
  if (change > 0.005) direction = 'up';
  else if (change < -0.005) direction = 'down';

  // Last step direction (for green/red flash) — prefer last non-zero delta
  let lastDelta = 0;
  for (let i = points.length - 1; i >= 0; i--) {
    if (points[i].delta !== 0) {
      lastDelta = points[i].delta;
      break;
    }
  }

  return {
    symbol: SPN_SYMBOL,
    base: round2(base),
    series,
    points,
    price: round2(last),
    change,
    changePct,
    direction,
    lastDelta: round2(lastDelta),
  };
}

/**
 * Convenience: quote from full session (uses growth.events).
 * @param {object} session
 */
export function spnFromSession(session) {
  return buildSpnQuote(session?.growth || {});
}

/**
 * Format for HUD: "$SPN 104.20 +4.20 (+4.20%)"
 * @param {ReturnType<typeof buildSpnQuote>} quote
 */
export function formatSpnTicker(quote) {
  if (!quote) return `${SPN_SYMBOL} —`;
  const sign = quote.change > 0 ? '+' : '';
  return `${quote.symbol} ${quote.price.toFixed(2)} ${sign}${quote.change.toFixed(2)} (${sign}${quote.changePct.toFixed(2)}%)`;
}

/**
 * Path points for SVG sparkline in viewBox 0 0 W H (y-down).
 * @param {number[]} series
 * @param {{ width?: number, height?: number, pad?: number }} [opts]
 * @returns {{ d: string, min: number, max: number, width: number, height: number }}
 */
export function seriesToSvgPath(series, opts = {}) {
  const width = opts.width || 120;
  const height = opts.height || 36;
  const pad = opts.pad ?? 2;
  const pts = (series || []).filter((n) => Number.isFinite(n));
  if (pts.length === 0) {
    return { d: '', min: 0, max: 0, width, height };
  }
  let min = Math.min(...pts);
  let max = Math.max(...pts);
  if (min === max) {
    min -= 1;
    max += 1;
  }
  const innerW = width - pad * 2;
  const innerH = height - pad * 2;
  const n = pts.length;
  const coords = pts.map((p, i) => {
    const x = pad + (n === 1 ? innerW / 2 : (i / (n - 1)) * innerW);
    const y = pad + (1 - (p - min) / (max - min)) * innerH;
    return [round2(x), round2(y)];
  });
  const d = coords
    .map(([x, y], i) => `${i === 0 ? 'M' : 'L'}${x} ${y}`)
    .join(' ');
  return { d, min, max, width, height };
}

function clampPrice(p) {
  if (!Number.isFinite(p)) return SPN_BASE;
  return Math.max(1, Math.min(9999, p));
}

function round2(n) {
  return Math.round(n * 100) / 100;
}
