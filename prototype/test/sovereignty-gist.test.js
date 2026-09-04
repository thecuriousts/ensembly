import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  SOVEREIGNTY_STEPS,
  SOVEREIGNTY_LAYERS,
  SOVEREIGNTY_ALPHA_LINE,
  assuranceTierFor,
  isExtractionPronePosture,
  stepsForLayer,
} from '../src/sovereignty-gist.js';

describe('sovereignty-gist (public tracked)', () => {
  it('exposes exactly 15 steps across four layers', () => {
    assert.equal(SOVEREIGNTY_STEPS.length, 15);
    assert.equal(SOVEREIGNTY_STEPS[0].id, 'I');
    assert.equal(SOVEREIGNTY_STEPS[14].id, 'XV');
    assert.ok(SOVEREIGNTY_LAYERS.control.own);
    assert.equal(SOVEREIGNTY_LAYERS.models.own, false);
    assert.match(SOVEREIGNTY_ALPHA_LINE, /Sovereignty is the alpha/i);
  });

  it('assuranceTierFor maps high to local no-egress', () => {
    const t = assuranceTierFor('high');
    assert.equal(t.tier, 'local_no_egress');
    assert.equal(t.epmAllowed, false);
  });

  it('isExtractionPronePosture treats non-ZDR third-party as EPM', () => {
    assert.equal(isExtractionPronePosture({ thirdParty: true, zdr: false }), true);
    assert.equal(isExtractionPronePosture({ thirdParty: true, zdr: true }), false);
  });

  it('stepsForLayer returns control steps including context flywheel', () => {
    const control = stepsForLayer('control');
    assert.ok(control.length >= 5);
    assert.ok(control.some((s) => s.id === 'XV'));
  });
});
