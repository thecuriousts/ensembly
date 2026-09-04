/**
 * Playable life-mirror graph — pure transform from operator turn IR.
 * Beacons: next physical + next authorization (not sample-only labels).
 * Does not import turn.js (avoids cycle: turn → play → turn).
 */
import { buildGameGraph } from './graph.js';
import { flowToPlaceNode, flowToActionCandidate } from './digital-flow.js';

/**
 * Build a playable graph from an existing turn surface (pure).
 * Stamps nextPhysical / nextAuth as beacons for session + WASM focus.
 *
 * @param {{
 *   turn: object,
 *   digitalFlows?: Array<object>,
 *   trail?: Array<{ phase: string }>,
 * }} input
 */
export function buildPlayableGraphFromTurn({ turn, digitalFlows = [], trail } = {}) {
  if (!turn || typeof turn !== 'object') {
    throw new Error('turn required');
  }

  const actions = [...(turn.plan?.actions || turn.actions || [])];
  const projects = [...(turn.plan?.projects || turn.projects || [])];
  const schedule = turn.plan?.schedule || turn.schedule || [];
  const snapshot = turn.snapshot || null;

  // Ensure digital-flow actions exist on the graph
  for (const flow of digitalFlows) {
    const cand = flowToActionCandidate(flow);
    if (!actions.some((a) => a.id === cand.id)) actions.push(cand);
  }

  const graph = buildGameGraph({
    date: turn.date,
    card: {
      phase: turn.summary?.phase || turn.snapshot?.phase || 'CLEAR',
      goal: 'Game of Peram life-mirror',
    },
    trail: trail || [
      { phase: 'ORIENT' },
      { phase: 'PLAN' },
      { phase: turn.summary?.phase || turn.snapshot?.phase || 'CLEAR' },
    ],
    actions,
    projects,
    schedule,
    snapshot,
  });

  // World places (Bank, …)
  for (const flow of digitalFlows) {
    const place = flowToPlaceNode(flow);
    if (!graph.nodes.some((n) => n.id === place.id)) {
      graph.nodes.push({
        ...place,
        position: { x: 0, y: (graph.nodes.length % 6) * 80 },
      });
      graph.edges.push({
        id: `e-game-peram-${place.id}-place`,
        source: 'game-peram',
        target: place.id,
        kind: 'world_place',
      });
      const actionNodeId = `action-${flow.actionId || flow.id}`;
      if (graph.nodes.some((n) => n.id === actionNodeId)) {
        graph.edges.push({
          id: `e-${place.id}-${actionNodeId}-hosts`,
          source: place.id,
          target: actionNodeId,
          kind: 'hosts_flow',
        });
      }
    }
  }

  const nextPhysical = turn.nextPhysical || turn.status?.next?.physical || null;
  const nextAuth = turn.nextAuth || turn.status?.next?.authorization || null;

  stampBeacons(graph, nextPhysical, nextAuth);

  graph.meta = {
    ...graph.meta,
    generator: 'ensembly/src/play.js',
    source: 'life-operator-ir',
    nextPhysicalId: nextPhysical?.id || null,
    nextAuthId: nextAuth?.id || null,
    nextPhysicalLabel: nextPhysical?.title || null,
    nextAuthLabel: nextAuth?.title || null,
    digitalFlowCount: digitalFlows.length,
    turnStatusAt: turn.snapshot?.updatedAt || null,
    snapshotStatus: turn.summary?.snapshotStatus ?? snapshot?.status ?? null,
  };
  graph.meta.nodeCount = graph.nodes.length;
  graph.meta.edgeCount = graph.edges.length;

  // Session-friendly pending list
  graph.pending = (turn.pendingAuthorizations || snapshot?.pending || [])
    .filter((p) => (p.status || 'pending') === 'pending')
    .map((p) => ({
      id: p.id,
      title: p.title || p.id,
      actionId: p.actionId || null,
      status: p.status || 'pending',
      kind: p.kind || null,
    }));

  return graph;
}

/**
 * Mark next physical / next auth nodes as beacons for game focus.
 */
function stampBeacons(graph, nextPhysical, nextAuth) {
  const physId = nextPhysical?.id;
  const authId = nextAuth?.id;
  const authActionId = nextAuth?.actionId;

  for (const n of graph.nodes) {
    n.beacon = false;
    n.beaconRole = null;

    if (physId) {
      if (n.id === `action-${physId}` || n.id === physId) {
        n.beacon = true;
        n.beaconRole = 'next_physical';
        n.kind = n.kind || 'action';
      }
    }
    if (authId) {
      if (
        n.id === authId ||
        n.id === `auth-${authActionId || ''}` ||
        n.id === `hitl-${authActionId || ''}` ||
        (authActionId && n.id === `action-${authActionId}`)
      ) {
        n.beacon = true;
        n.beaconRole = n.beaconRole || 'next_auth';
      }
    }
  }

  // Ensure beacon nodes exist even if ranking pointed at ids outside node list
  if (physId && !graph.nodes.some((n) => n.beaconRole === 'next_physical')) {
    graph.nodes.push({
      id: `action-${physId}`,
      type: 'physical',
      label: nextPhysical.title || physId,
      realm: 'physical',
      kind: 'action',
      beacon: true,
      beaconRole: 'next_physical',
      area: nextPhysical.area || null,
      position: { x: 220, y: 0 },
    });
    graph.edges.push({
      id: `e-game-peram-action-${physId}-physical_pickup`,
      source: 'game-peram',
      target: `action-${physId}`,
      kind: 'physical_pickup',
    });
  }
  if (authId && !graph.nodes.some((n) => n.beaconRole === 'next_auth')) {
    graph.nodes.push({
      id: authId,
      type: 'hitl',
      label: nextAuth.title || authId,
      kind: 'authorization',
      status: 'pending',
      beacon: true,
      beaconRole: 'next_auth',
      position: { x: 440, y: 0 },
    });
    graph.edges.push({
      id: `e-game-peram-${authId}-authorization`,
      source: 'game-peram',
      target: authId,
      kind: 'authorization',
    });
  }
}

/**
 * List beacon nodes (for tests / session boot focus).
 * @param {object} graph
 */
export function listBeacons(graph) {
  return (graph?.nodes || []).filter((n) => n.beacon);
}
