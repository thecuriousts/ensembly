/**
 * Public sovereignty gist constants (no private operator detail).
 *
 * Credits — inspired by Palantir Technologies white paper:
 *   "Institutional Sovereignty in the Age of AI" (15 steps)
 *   Landing: https://www.palantir.com/ai-sovereignty-is-your-alpha/
 *   PDF: https://assets.ctfassets.net/xrfr7uokpv1b/yF0AXklHQd7K3SqKICNTM/e9f9167d1b3c7cce56ab3b8c4cc572da/Palantir_-_Institutional_Sovereignty_in_the_Age_of_AI.pdf
 * Not affiliated with Palantir. Full personal mapping: private/thinking/ (gitignored).
 *
 * @see public/thinking/sovereignty-gist.md
 */

/** @typedef {'foundations' | 'model' | 'compute' | 'control'} SovereigntyLayer */

/**
 * Fifteen public step ids + titles (shareable).
 */
export const SOVEREIGNTY_STEPS = Object.freeze([
  { id: 'I', title: 'Ensure zero data retention (ZDR)', layer: 'foundations' },
  { id: 'II', title: 'Determine your AI decision tree', layer: 'foundations' },
  { id: 'III', title: 'Identify opportunities in your architecture', layer: 'foundations' },
  { id: 'IV', title: 'Guard against misaligned incentives', layer: 'model' },
  { id: 'V', title: 'Maximize model liquidity', layer: 'model' },
  { id: 'VI', title: 'Own the model flywheel', layer: 'model' },
  { id: 'VII', title: 'Decide hardware based on assurance', layer: 'compute' },
  { id: 'VIII', title: 'Own adaptable hardware for sensitive workflows', layer: 'compute' },
  { id: 'IX', title: 'Verify compute you do not own', layer: 'compute' },
  { id: 'X', title: 'Be model agnostic', layer: 'control' },
  { id: 'XI', title: 'Implement granular permissions', layer: 'control' },
  { id: 'XII', title: 'Audit and log', layer: 'control' },
  { id: 'XIII', title: 'Practice adaptive cybersecurity', layer: 'control' },
  { id: 'XIV', title: 'Build by branching', layer: 'control' },
  { id: 'XV', title: 'Own the context flywheel', layer: 'control' },
]);

/**
 * Layers to own vs rent (public doctrine).
 */
export const SOVEREIGNTY_LAYERS = Object.freeze({
  control: { own: true, note: 'Workflows, ontology, agents, audit, branching' },
  models: { own: false, note: 'Liquid commodities; zero-trust incentives' },
  compute: { own: 'tiered', note: 'Assurance matches sensitivity' },
});

/** One-line public thesis */
export const SOVEREIGNTY_ALPHA_LINE =
  'Sovereignty is the alpha: own value created and freedom to pursue opportunity via an owned context flywheel; keep models liquid.';

/**
 * Map a workload sensitivity label to a public assurance tier.
 * @param {'high' | 'medium' | 'low' | 'physical' | 'external_mutate'} sensitivity
 */
export function assuranceTierFor(sensitivity) {
  switch (sensitivity) {
    case 'high':
      return { tier: 'local_no_egress', zdrRequired: true, epmAllowed: false };
    case 'medium':
      return { tier: 'zdr_or_attested', zdrRequired: true, epmAllowed: false };
    case 'low':
      return { tier: 'public_swappable', zdrRequired: false, epmAllowed: true };
    case 'physical':
      return { tier: 'human_body', zdrRequired: false, epmAllowed: false };
    case 'external_mutate':
      return { tier: 'hitl_gate', zdrRequired: true, epmAllowed: false };
    default:
      return { tier: 'default_deny_high', zdrRequired: true, epmAllowed: false };
  }
}

/**
 * Non-ZDR frontier → extraction-prone posture (public rule).
 * @param {{ zdr?: boolean, thirdParty?: boolean }} opts
 */
export function isExtractionPronePosture(opts = {}) {
  const thirdParty = opts.thirdParty !== false;
  const zdr = opts.zdr === true;
  return thirdParty && !zdr;
}

/**
 * Steps for a layer (pure).
 * @param {SovereigntyLayer} layer
 */
export function stepsForLayer(layer) {
  return SOVEREIGNTY_STEPS.filter((s) => s.layer === layer);
}
