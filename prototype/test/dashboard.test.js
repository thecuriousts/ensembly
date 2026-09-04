/**
 * Dashboard IR — pure builder + insights + HTML projector (shipped paths).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  DASHBOARD_IR_VERSION,
  buildDashboard,
  deriveInsights,
  dashboardToHtml,
  formatDashboardMarkdown,
} from '../src/dashboard.js';
import { normalizeActivityEntry } from '../src/activity/ir.js';

function sampleStatus() {
  return {
    version: 1,
    date: '2026-07-13',
    snapshotStatus: 'idle_waiting',
    phase: 'HITL_WAIT',
    next: {
      physical: {
        id: 'healthy-self-energy',
        title: 'Healthy Self Energy foundation',
        area: 'Health',
        claimStatus: 'open',
        commands: {
          claim: 'node bin/swarm.js claim healthy-self-energy',
          complete: 'node bin/swarm.js complete healthy-self-energy',
        },
      },
      authorization: {
        id: 'auth-apply-high-signal',
        title: 'Prepare high-signal FT application packet',
        kind: 'job_application_submit',
        commands: {
          approve: 'node bin/swarm.js approve auth-apply-high-signal',
          deny: 'node bin/swarm.js deny auth-apply-high-signal',
        },
      },
    },
    physical: [{ id: 'healthy-self-energy', title: 'Healthy Self Energy foundation', claimStatus: 'open' }],
    pending: [{ id: 'auth-apply-high-signal', title: 'Prepare high-signal FT application packet', status: 'pending' }],
    counts: { physical: 1, pending: 1 },
  };
}

describe('buildDashboard (shipped pure path)', () => {
  it('returns version 1 IR with overview and next from turn status', () => {
    const status = sampleStatus();
    const dash = buildDashboard({
      status,
      snapshot: {
        version: 1,
        status: 'idle_waiting',
        phase: 'HITL_WAIT',
        pending: [{ id: 'auth-apply-high-signal', status: 'pending' }],
        physical: [{ id: 'healthy-self-energy', status: 'open' }],
        history: [{ id: 'x', decision: 'physical_claim' }],
      },
      plan: {
        date: '2026-07-13',
        actions: [
          { id: 'a1', area: 'Career' },
          { id: 'a2', area: 'Systems' },
        ],
        schedule: [{ start: '10:00', end: '13:00' }],
      },
      activities: [],
      now: '2026-07-13T08:00:00.000Z',
    });

    assert.equal(dash.version, DASHBOARD_IR_VERSION);
    assert.equal(dash.overview.date, '2026-07-13');
    assert.equal(dash.overview.phase, 'HITL_WAIT');
    assert.equal(dash.next.physical.id, 'healthy-self-energy');
    assert.equal(dash.next.physical.title, 'Healthy Self Energy foundation');
    assert.equal(dash.next.authorization.id, 'auth-apply-high-signal');
    assert.equal(dash.overview.pendingAuth, 1);
    assert.equal(dash.overview.historyLen, 1);
    assert.equal(dash.stats.scheduleBlocks, 1);
    assert.ok(Array.isArray(dash.insights));
  });

  it('stats count activity kinds from real appended-shaped entries', () => {
    const acts = [
      normalizeActivityEntry({
        id: 'c1',
        kind: 'activity.claim',
        ts: '2026-07-13T07:00:00.000Z',
        payload: { message: 'claimed healthy-self-energy' },
      }),
      normalizeActivityEntry({
        id: 'l1',
        kind: 'log.info',
        ts: '2026-07-13T07:01:00.000Z',
        payload: { message: 'turn ran' },
      }),
    ];
    const dash = buildDashboard({
      status: sampleStatus(),
      activities: acts,
      now: '2026-07-13T08:00:00.000Z',
    });
    assert.equal(dash.stats.activityTotal, 2);
    assert.equal(dash.stats.activityByKind['activity.claim'], 1);
    assert.equal(dash.stats.activityByKind['log.info'], 1);
    assert.equal(dash.activity.recent[0].message, 'turn ran');
    assert.equal(dash.activity.recent[1].message, 'claimed healthy-self-energy');
  });
});

describe('deriveInsights (rule-based with evidence)', () => {
  it('emits clear_auth_gate when pending auth present', () => {
    const dash = buildDashboard({ status: sampleStatus(), activities: [] });
    const ids = dash.insights.map((i) => i.id);
    assert.ok(ids.includes('clear_auth_gate'));
    const ins = dash.insights.find((i) => i.id === 'clear_auth_gate');
    assert.equal(ins.severity, 'steer');
    assert.equal(ins.evidence.nextAuthId, 'auth-apply-high-signal');
    assert.match(ins.steer, /approve auth-apply-high-signal/);
  });

  it('emits body_work_open when physical open and no claim events', () => {
    const dash = buildDashboard({ status: sampleStatus(), activities: [] });
    const ins = dash.insights.find((i) => i.id === 'body_work_open');
    assert.ok(ins);
    assert.equal(ins.evidence.nextPhysicalId, 'healthy-self-energy');
    assert.match(ins.steer, /claim healthy-self-energy/);
  });

  it('emits activity_empty when no activities', () => {
    const dash = buildDashboard({ status: sampleStatus(), activities: [] });
    assert.ok(dash.insights.some((i) => i.id === 'activity_empty'));
  });

  it('emits balance_risk when plan lacks health/relationships', () => {
    const dash = buildDashboard({
      status: sampleStatus(),
      plan: {
        actions: [
          { id: 'c', area: 'Career' },
          { id: 's', area: 'Systems' },
        ],
      },
      activities: [
        normalizeActivityEntry({
          id: 'x',
          kind: 'activity.claim',
          ts: '2026-07-13T07:00:00.000Z',
          payload: {},
        }),
      ],
    });
    const ins = dash.insights.find((i) => i.id === 'balance_risk');
    assert.ok(ins);
    assert.equal(ins.evidence.health, 0);
    assert.equal(ins.evidence.relationships, 0);
  });

  it('emits queues_clear when no open physical and no pending', () => {
    const insights = deriveInsights({
      overview: { physicalOpen: 0, pendingAuth: 0, balanceScore: 1 },
      next: { physical: null, authorization: null },
      stats: { activityByKind: {}, activityTotal: 1 },
      activity: { empty: false },
    }, { activities: [{ kind: 'log.info' }], planActions: [] });
    assert.ok(insights.some((i) => i.id === 'queues_clear'));
  });
});

describe('dashboard projectors', () => {
  it('HTML includes real next physical title and insights evidence', () => {
    const dash = buildDashboard({
      status: sampleStatus(),
      activities: [
        normalizeActivityEntry({
          id: 'a1',
          kind: 'log.info',
          ts: '2026-07-13T07:00:00.000Z',
          payload: { message: 'post-lunch steer' },
        }),
      ],
    });
    const html = dashboardToHtml(dash);
    assert.match(html, /Healthy Self Energy foundation/);
    assert.match(html, /auth-apply-high-signal/);
    assert.match(html, /Intelligent insights/);
    assert.match(html, /clear_auth_gate|Clear the open authorization/);
    assert.match(html, /post-lunch steer/);
    assert.match(html, /dashboard\.json/);
    assert.match(html, /href="http:\/\/127\.0\.0\.1:4173\/game\/"/);
    assert.doesNotMatch(html, /lorem ipsum/i);
  });

  it('markdown includes overview and next ids from input', () => {
    const dash = buildDashboard({ status: sampleStatus(), activities: [] });
    const md = formatDashboardMarkdown(dash);
    assert.match(md, /Life progress dashboard/);
    assert.match(md, /healthy-self-energy/);
    assert.match(md, /auth-apply-high-signal/);
    assert.match(md, /Insights/);
  });
});
