/**
 * life-os bridge surface (portfolio projection only — vault never merges into git).
 */
export {
  resolveLifeOsRoot,
  parseProjectFrontmatter,
  normalizeArea,
  projectCardToCandidate,
  projectPortfolioToCandidates,
  partitionPortfolioForShare,
  loadPortfolioProjection,
  mergeLifeOsIntoState,
} from './portfolio.js';
