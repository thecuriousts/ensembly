import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  parseProjectFrontmatter,
  normalizeArea,
  projectCardToCandidate,
  projectPortfolioToCandidates,
  partitionPortfolioForShare,
  loadPortfolioProjection,
  mergeLifeOsIntoState,
  resolveLifeOsRoot,
} from '../src/lifeos/portfolio.js';
import { privatePathPatterns } from '../src/privacy.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

describe('life-os portfolio projection (shipped)', () => {
  it('parses project README frontmatter next_action', () => {
    const sample = fs.readFileSync(
      path.join(root, 'fixtures', 'lifeos-portfolio-sample.md'),
      'utf8',
    );
    const { frontmatter } = parseProjectFrontmatter(sample);
    assert.equal(frontmatter.importance, 4);
    assert.equal(frontmatter.urgency, 4);
    assert.match(String(frontmatter.next_action), /Dogfood turn/);
    assert.equal(normalizeArea(frontmatter.area), 'Systems');
  });

  it('projects card → day/graph candidate with real next_action title', () => {
    const sample = fs.readFileSync(
      path.join(root, 'fixtures', 'lifeos-portfolio-sample.md'),
      'utf8',
    );
    const { frontmatter } = parseProjectFrontmatter(sample);
    const cand = projectCardToCandidate({ slug: 'ensembly', frontmatter });
    assert.ok(cand);
    assert.equal(cand.id, 'lifeos-ensembly');
    assert.match(cand.title, /Dogfood turn/);
    assert.equal(cand.source, 'life-os-portfolio');
    assert.equal(cand.area, 'Systems');
    assert.ok(cand.realm);
  });

  it('finance card stays private on share partition (no public leak)', () => {
    const sample = fs.readFileSync(
      path.join(root, 'fixtures', 'lifeos-finance-card.md'),
      'utf8',
    );
    const { frontmatter } = parseProjectFrontmatter(sample);
    const cand = projectCardToCandidate({ slug: 'wealth-due-diligence', frontmatter });
    assert.ok(cand);
    assert.equal(cand.area, 'Finance');
    assert.equal(cand.classification.visibility, 'private');
    assert.equal(cand.classification.hitl, true);
    assert.equal(cand.classification.pushable, false);

    const { publicItems, privateItems } = partitionPortfolioForShare([cand]);
    assert.equal(publicItems.length, 0);
    assert.equal(privateItems.length, 1);
    assert.ok(privateItems[0].id.includes('wealth'));
  });

  it('mergeLifeOsIntoState adds candidates without dropping local ones', () => {
    const state = {
      extra_candidates: [{ id: 'grocery-errand', title: 'Grocery', realm: 'physical' }],
    };
    const life = projectPortfolioToCandidates([
      {
        slug: 'ensembly',
        frontmatter: {
          status: 'In Progress',
          importance: 4,
          urgency: 4,
          area: '[[Systems]]',
          next_action: 'Export life-graph',
        },
      },
    ]);
    const merged = mergeLifeOsIntoState(state, life);
    assert.equal(merged.lifeOsMerged, true);
    assert.ok(merged.extra_candidates.some((c) => c.id === 'grocery-errand'));
    assert.ok(merged.extra_candidates.some((c) => c.id === 'lifeos-ensembly'));
  });

  it('loadPortfolioProjection reads real ~/life-os when present (no vault in git tree)', () => {
    const lifeOs = resolveLifeOsRoot();
    const proj = loadPortfolioProjection({ lifeOsRoot: lifeOs });
    // Product must not require vault path inside ensembly repo
    assert.ok(!String(proj.lifeOsRoot || '').includes(`${root}/life-os`));
    assert.ok(privatePathPatterns().some((p) => p.includes('private/') || p.includes('data/local')));

    if (fs.existsSync(path.join(lifeOs, 'Projects'))) {
      assert.ok(proj.cards.length >= 1, 'expected at least one portfolio card');
      assert.ok(proj.candidates.length >= 1, 'expected projected next_action candidates');
      // ensembly card should project
      const ens = proj.candidates.find((c) => c.projectSlug === 'ensembly' || c.id === 'lifeos-ensembly');
      if (ens) {
        assert.match(ens.title, /./);
        assert.equal(ens.source, 'life-os-portfolio');
      }
      // finance stays private
      const fin = proj.candidates.find((c) => c.area === 'Finance');
      if (fin) {
        assert.equal(fin.classification.visibility, 'private');
      }
    }
  });

  it('skips archived cards and empty next_action', () => {
    const archived = projectCardToCandidate({
      slug: 'old',
      frontmatter: { status: 'Archived', next_action: 'noop', area: 'Systems' },
    });
    assert.equal(archived, null);
    const empty = projectCardToCandidate({
      slug: 'x',
      frontmatter: { status: 'In Progress', next_action: '', area: 'Systems' },
    });
    assert.equal(empty, null);
  });
});
