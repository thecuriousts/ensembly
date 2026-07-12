import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  classifyItem,
  partitionByVisibility,
  privatePathPatterns,
} from '../src/privacy.js';

describe('privacy classification (shipped)', () => {
  it('marks Finance area private and non-pushable', () => {
    const c = classifyItem({
      id: 'runway',
      title: 'Runway admin',
      area: 'Finance',
      public: true,
    });
    // area wins over naive public flag for finance
    assert.equal(c.visibility, 'private');
    assert.equal(c.pushable, false);
    assert.equal(c.hitl, true);
  });

  it('detects private keyword patterns in title/body', () => {
    const c = classifyItem({
      id: 'med',
      title: 'Follow up medical appointment notes',
      area: 'Health',
    });
    assert.equal(c.visibility, 'private');
    assert.equal(c.pushable, false);
  });

  it('classifies explicit public OSS work as public pushable', () => {
    const c = classifyItem({
      id: 'oss',
      title: 'Ship ensembly swarm README (OSS)',
      area: 'Systems',
      public: true,
    });
    assert.equal(c.visibility, 'public');
    assert.equal(c.pushable, true);
    assert.equal(c.hitl, false);
  });

  it('default-deny when not marked public', () => {
    const c = classifyItem({ id: 'x', title: 'Vague task', area: 'Learning' });
    assert.equal(c.visibility, 'private');
    assert.equal(c.pushable, false);
  });

  it('partitionByVisibility splits lists with classification attached', () => {
    const { publicItems, privateItems } = partitionByVisibility([
      { id: 'p', title: 'Publish skill to GitHub', area: 'Systems', public: true },
      { id: 'f', title: 'Debt avalanche review', area: 'Finance' },
    ]);
    assert.equal(publicItems.length, 1);
    assert.equal(privateItems.length, 1);
    assert.equal(publicItems[0].classification.visibility, 'public');
    assert.equal(privateItems[0].classification.visibility, 'private');
  });

  it('privatePathPatterns includes private/ and data/local/', () => {
    const patterns = privatePathPatterns();
    assert.ok(patterns.includes('private/'));
    assert.ok(patterns.includes('data/local/'));
  });
});
