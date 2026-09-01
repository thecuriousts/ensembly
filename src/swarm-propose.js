/**
 * Swarm propose IR — outer worker deposits a pending gate into the hub inbox.
 * Grok Bot proposes. Operator emits via approve/deny on the wait snapshot.
 */

export const SWARM_PROPOSE_KIND = 'swarm_emit';

/**
 * @param {{
 *   id?: string,
 *   title?: string,
 *   summary?: string,
 *   source?: string,
 *   area?: string,
 *   now?: string,
 * }} [spec]
 */
export function createSwarmPropose(spec = {}) {
	const now = spec.now || new Date().toISOString();
	const title = String(spec.title || 'Swarm chore').trim();
	const slug = title
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-|-$/g, '')
		.slice(0, 28);
	const stamp = now.slice(11, 19).replace(/:/g, '');
	const id = spec.id || `propose-${slug || 'chore'}-${stamp}`;
	return {
		version: 1,
		id,
		title,
		summary: spec.summary ? String(spec.summary).trim() : null,
		source: spec.source || 'grok-bot',
		kind: SWARM_PROPOSE_KIND,
		area: spec.area || 'Systems',
		realm: 'digital',
		status: 'pending',
		createdAt: now,
		updatedAt: now,
	};
}

/**
 * @param {object} propose
 */
export function swarmProposeToActionCandidate(propose) {
	return {
		id: propose.id,
		title: propose.title,
		kind: propose.kind,
		area: propose.area,
		realm: propose.realm,
		source: propose.source,
		summary: propose.summary,
		classification: {
			hitl: true,
			visibility: 'private',
			reason: `swarm propose from ${propose.source}`,
			pushable: false,
		},
	};
}

/**
 * @param {object} propose
 */
export function swarmProposeToApprovalRecord(propose) {
	return {
		id: `auth-${propose.id}`,
		actionId: propose.id,
		title: propose.title,
		kind: propose.kind,
		area: propose.area,
		realm: propose.realm,
		status: 'pending',
		reason: propose.summary
			? `${propose.source}: ${propose.summary}`
			: `emit required from ${propose.source}`,
		source: propose.source,
		proposeId: propose.id,
		createdAt: propose.createdAt,
		updatedAt: propose.updatedAt,
	};
}
