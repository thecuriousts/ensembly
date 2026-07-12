import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { classifyRealm, physicalPickups, digitalActions, enrichWithRealm } from '../src/realm.js';

describe('realm (shipped physical vs digital)', () => {
  it('classifies explicit physical realm and errand kinds', () => {
    assert.equal(classifyRealm({ realm: 'physical' }).realm, 'physical');
    assert.equal(
      classifyRealm({ id: 'g', title: 'Grocery errand', kind: 'physical_errand' }).realm,
      'physical',
    );
  });

  it('classifies career OSS as digital', () => {
    const r = classifyRealm({
      title: 'Ship ensembly swarm README (OSS)',
      area: 'Systems',
      public: true,
    });
    assert.equal(r.realm, 'digital');
  });

  it('physicalPickups filters only physical pending', () => {
    const list = physicalPickups([
      { id: 'a', title: 'Walk outdoor', realm: 'physical' },
      { id: 'b', title: 'Ship PR', realm: 'digital' },
      { id: 'c', title: 'Done walk', realm: 'physical', status: 'done' },
    ]);
    assert.equal(list.length, 1);
    assert.equal(list[0].id, 'a');
  });

  it('enrichWithRealm attaches realmInfo', () => {
    const [x] = enrichWithRealm([{ id: '1', title: 'Evening outdoor family walk' }]);
    assert.equal(x.realm, 'physical');
    assert.ok(x.realmInfo.reason);
  });

  it('digitalActions returns digital only', () => {
    const d = digitalActions([
      { id: '1', title: 'Apply job', area: 'Career' },
      { id: '2', title: 'Family support time', area: 'Relationships', non_negotiable: true },
    ]);
    assert.ok(d.every((a) => a.realm === 'digital'));
  });
});
