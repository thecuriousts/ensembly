import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  createSession,
  dispatch,
  sessionView,
  SPN_SYMBOL,
  SPN_BASE,
  buildSpnQuote,
  eventToPriceDelta,
  seriesToSvgPath,
  formatSpnTicker,
} from '../src/game/index.js';

function growthGraph() {
  return {
    nodes: [
      { id: 'game-root', type: 'game', label: 'Root', kind: 'root' },
      { id: 'phys', type: 'physical', realm: 'physical', label: 'Grocery' },
      { id: 'craft', type: 'action', realm: 'digital', label: 'Ship OSS', area: 'craft' },
      { id: 'family', type: 'schedule', label: 'Family outdoor', area: 'family' },
      { id: 'hitl-x', type: 'hitl', hitl: true, label: 'HITL Apply' },
    ],
    pending: [{ id: 'auth-apply-high-signal', title: 'Apply', status: 'pending' }],
  };
}

describe('$SPN progress ticker (shipped)', () => {
  it('symbol is $SPN and open quote is finite at base', () => {
    const q = buildSpnQuote({ events: [], balance: {} });
    assert.equal(q.symbol, SPN_SYMBOL);
    assert.equal(q.symbol, '$SPN');
    assert.equal(q.base, SPN_BASE);
    assert.ok(Number.isFinite(q.price));
    assert.ok(q.series.length >= 2);
    assert.ok(q.series.every((n) => Number.isFinite(n)));
  });

  it('positive claim progress raises $SPN price above open', () => {
    let s = createSession(growthGraph());
    const open = sessionView(s).growth.spn.price;
    s = dispatch(s, { type: 'FOCUS_INDEX', payload: { index: 1 } });
    s = dispatch(s, { type: 'COMPLETE' });
    const spn = sessionView(s).growth.spn;
    assert.ok(spn, 'growthView must expose spn');
    assert.equal(spn.symbol, '$SPN');
    assert.ok(spn.price > open, `price ${spn.price} should exceed open ${open}`);
    assert.equal(spn.direction, 'up');
    assert.ok(spn.change > 0);
    assert.ok(spn.series.length >= 2);
    assert.ok(spn.series.every((n) => Number.isFinite(n)));
  });

  it('HITL deny moves $SPN down (or non-up last step) vs open after only deny', () => {
    let s = createSession(growthGraph());
    const open = sessionView(s).growth.spn.price;
    s = dispatch(s, { type: 'DENY' });
    const spn = sessionView(s).growth.spn;
    // Deny is a miss path for the ticker even if modest XP is recorded
    assert.ok(
      spn.price < open || spn.lastDelta < 0,
      `deny should sell: price=${spn.price} open=${open} lastDelta=${spn.lastDelta}`,
    );
    assert.ok(spn.series.length >= 2);
    const lastStep = spn.series[spn.series.length - 1] - spn.series[spn.series.length - 2];
    assert.ok(lastStep < 0, `last series step should be down, got ${lastStep}`);
  });

  it('claim then deny: series has both up and down moves', () => {
    let s = createSession(growthGraph());
    s = dispatch(s, { type: 'FOCUS_INDEX', payload: { index: 1 } });
    s = dispatch(s, { type: 'COMPLETE' });
    const afterClaim = sessionView(s).growth.spn.price;
    s = dispatch(s, { type: 'DENY' });
    const spn = sessionView(s).growth.spn;
    assert.ok(spn.series.length >= 3);
    // At least one up step and one down step in the path
    let up = false;
    let down = false;
    for (let i = 1; i < spn.series.length; i++) {
      const d = spn.series[i] - spn.series[i - 1];
      if (d > 0) up = true;
      if (d < 0) down = true;
    }
    assert.ok(up, 'expected an up move after claim');
    assert.ok(down, 'expected a down move after deny');
    assert.ok(spn.price < afterClaim, 'deny should pull price under post-claim level');
  });

  it('eventToPriceDelta is positive for claims and negative for deny', () => {
    assert.ok(eventToPriceDelta({ amount: 40, reason: 'claim:physical', role: 'physical' }) > 0);
    assert.ok(eventToPriceDelta({ amount: 18, reason: 'hitl:denied', role: 'hitl' }) < 0);
    assert.ok(eventToPriceDelta({ amount: 10, reason: 'drag:imbalance' }) < 0);
  });

  it('craft-only imbalance applies drag so series can fall', () => {
    const q = buildSpnQuote({
      events: [{ amount: 22, reason: 'claim:craft', role: 'craft' }],
      balance: { craft: 1, physical: 0, presence: 0, hitl: 0 },
    });
    assert.ok(q.series.length >= 3, 'open + claim + imbalance drag');
    const last = q.series[q.series.length - 1];
    const mid = q.series[q.series.length - 2];
    assert.ok(last < mid, 'imbalance drag should lower price after craft-only');
  });

  it('seriesToSvgPath yields drawable path from real series', () => {
    let s = createSession(growthGraph());
    s = dispatch(s, { type: 'FOCUS_INDEX', payload: { index: 1 } });
    s = dispatch(s, { type: 'COMPLETE' });
    s = dispatch(s, { type: 'FOCUS_INDEX', payload: { index: 2 } });
    s = dispatch(s, { type: 'COMPLETE' });
    const series = sessionView(s).growth.spn.series;
    const path = seriesToSvgPath(series, { width: 120, height: 36 });
    assert.match(path.d, /^M[\d.]+ /);
    assert.match(path.d, /L/);
    assert.ok(path.d.length > 8);
  });

  it('formatSpnTicker includes $SPN and price', () => {
    const q = buildSpnQuote({
      events: [{ amount: 40, reason: 'claim:physical', role: 'physical' }],
      balance: { physical: 1 },
    });
    const line = formatSpnTicker(q);
    assert.match(line, /\$SPN/);
    assert.match(line, /\d+\.\d{2}/);
  });
});
