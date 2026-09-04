/**
 * life-os portfolio projection — read project cards → day/graph candidates.
 * Vault stays outside ensembly git; this module only projects IR.
 * Pure parse + project; IO isolated in load helpers.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { classifyItem, partitionByVisibility } from '../privacy.js';
import { classifyRealm } from '../realm.js';

/** Default life-os vault root (not inside ensembly tree). */
export function resolveLifeOsRoot(opts = {}) {
  return (
    opts.lifeOsRoot ||
    process.env.LIFE_OS_ROOT ||
    path.join(opts.home || process.env.HOME || os.homedir(), 'life-os')
  );
}

/**
 * Parse YAML-ish frontmatter block from an Obsidian project README.
 * Supports simple key: value and key: "quoted value" lines (no nested YAML).
 * @param {string} markdown
 * @returns {{ frontmatter: Record<string, string|number|boolean>, body: string }}
 */
export function parseProjectFrontmatter(markdown = '') {
  const text = String(markdown || '');
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
  if (!m) return { frontmatter: {}, body: text };
  const frontmatter = {};
  for (const line of m[1].split(/\r?\n/)) {
    const lm = line.match(/^([A-Za-z0-9_]+):\s*(.*)$/);
    if (!lm) continue;
    const key = lm[1];
    let raw = lm[2].trim();
    // strip inline comments
    if (!raw.startsWith('[') && !raw.startsWith('{')) {
      const hash = raw.indexOf(' #');
      if (hash >= 0) raw = raw.slice(0, hash).trim();
    }
    if (
      (raw.startsWith('"') && raw.endsWith('"')) ||
      (raw.startsWith("'") && raw.endsWith("'"))
    ) {
      raw = raw.slice(1, -1);
    }
    if (raw === 'true') frontmatter[key] = true;
    else if (raw === 'false') frontmatter[key] = false;
    else if (/^-?\d+(\.\d+)?$/.test(raw)) frontmatter[key] = Number(raw);
    else frontmatter[key] = raw;
  }
  return { frontmatter, body: m[2] || '' };
}

/**
 * Normalize Obsidian [[Area]] wiki link → plain area name.
 * @param {string|undefined} area
 */
export function normalizeArea(area) {
  if (!area) return null;
  const s = String(area).trim();
  const m = s.match(/\[\[([^\]|#]+)(?:[|#][^\]]*)?\]\]/);
  if (m) return m[1].trim();
  return s.replace(/^\[\[|\]\]$/g, '').trim() || null;
}

/**
 * Project one portfolio card (slug + frontmatter) into a day/graph candidate.
 * @param {{ slug: string, frontmatter?: Record<string, any>, path?: string }} card
 * @returns {object|null} candidate or null if archived/empty
 */
export function projectCardToCandidate(card = {}) {
  const fm = card.frontmatter || {};
  const slug = String(card.slug || fm.id || '').trim();
  if (!slug) return null;

  const status = String(fm.status || '').toLowerCase();
  if (status.includes('archive') || status === 'done' || status === 'completed') {
    return null;
  }

  const nextAction = String(fm.next_action || '').trim();
  if (!nextAction) return null;

  const area = normalizeArea(fm.area) || 'Systems';
  const importance = Number(fm.importance ?? 2);
  const urgency = Number(fm.urgency ?? 2);
  const title = nextAction.length > 120 ? `${nextAction.slice(0, 117)}…` : nextAction;

  // Portfolio next_actions are digital copilot/duty by default; body-world
  // only when area/tags/kind already imply physical (realm classifier).
  const base = {
    id: `lifeos-${slug}`,
    title,
    area,
    importance: Number.isFinite(importance) ? importance : 2,
    urgency: Number.isFinite(urgency) ? urgency : 2,
    public: area === 'Finance' ? false : Boolean(fm.public),
    kind: area === 'Finance' ? 'finance_transfer' : fm.kind || 'portfolio_next',
    tags: ['life-os', 'portfolio', slug],
    source: 'life-os-portfolio',
    projectSlug: slug,
    projectStatus: fm.status || null,
    progress: fm.progress ?? null,
    cardPath: card.path || null,
    body: `life-os Projects/${slug} next_action`,
  };

  const realmInfo = classifyRealm(base);
  const classification = classifyItem(base);
  return {
    ...base,
    realm: realmInfo.realm,
    realmInfo,
    classification,
  };
}

/**
 * Project many cards → candidates (skips nulls).
 * @param {Array<object>} cards
 */
export function projectPortfolioToCandidates(cards = []) {
  const out = [];
  for (const card of cards) {
    const c = projectCardToCandidate(card);
    if (c) out.push(c);
  }
  return out;
}

/**
 * Partition projected candidates for share vs local-only.
 * Finance / medical / family private content must not land on public/share IR.
 * @param {Array<object>} candidates
 */
export function partitionPortfolioForShare(candidates = []) {
  return partitionByVisibility(candidates);
}

/**
 * Load project cards from a life-os vault (IO).
 * Does not require the vault inside ensembly git.
 * @param {{ lifeOsRoot?: string, home?: string, fs?: typeof fs }} [opts]
 * @returns {{ cards: Array<object>, candidates: Array<object>, lifeOsRoot: string, projectsDir: string|null }}
 */
export function loadPortfolioProjection(opts = {}) {
  const fsys = opts.fs || fs;
  const lifeOsRoot = resolveLifeOsRoot(opts);
  const projectsDir = path.join(lifeOsRoot, 'Projects');
  if (!fsys.existsSync(projectsDir)) {
    return { cards: [], candidates: [], lifeOsRoot, projectsDir: null };
  }

  const cards = [];
  let entries = [];
  try {
    entries = fsys.readdirSync(projectsDir, { withFileTypes: true });
  } catch {
    return { cards: [], candidates: [], lifeOsRoot, projectsDir };
  }

  for (const ent of entries) {
    if (!ent.isDirectory()) continue;
    const slug = ent.name;
    if (slug.startsWith('.') || slug === 'node_modules') continue;
    const readme = path.join(projectsDir, slug, 'README.md');
    if (!fsys.existsSync(readme)) continue;
    let raw = '';
    try {
      raw = fsys.readFileSync(readme, 'utf8');
    } catch {
      continue;
    }
    const { frontmatter } = parseProjectFrontmatter(raw);
    cards.push({ slug, frontmatter, path: readme });
  }

  const candidates = projectPortfolioToCandidates(cards);
  return { cards, candidates, lifeOsRoot, projectsDir };
}

/**
 * Merge life-os candidates into local state extra_candidates (pure).
 * Existing ids win; life-os fills gaps / adds new.
 * @param {object} state
 * @param {Array<object>} lifeOsCandidates
 */
export function mergeLifeOsIntoState(state = {}, lifeOsCandidates = []) {
  const existing = Array.isArray(state.extra_candidates) ? [...state.extra_candidates] : [];
  const byId = new Map(existing.map((c) => [c.id, c]));
  for (const c of lifeOsCandidates) {
    if (!c?.id) continue;
    if (!byId.has(c.id)) byId.set(c.id, c);
  }
  return {
    ...state,
    extra_candidates: [...byId.values()],
    lifeOsMerged: true,
    lifeOsCount: lifeOsCandidates.length,
  };
}
