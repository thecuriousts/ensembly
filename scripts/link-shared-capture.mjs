#!/usr/bin/env node
/**
 * Ensure life-os Projects/premflow/capture → ~/.premflow (one inode tree).
 * Usage: node scripts/link-shared-capture.mjs
 */
import {
  ensureLifeOsCaptureLink,
  resolveSharedCaptureRoot,
  resolveLifeOsCaptureLinkPath,
} from '../src/premflow/index.js';

const result = ensureLifeOsCaptureLink();
console.log(JSON.stringify({
  ...result,
  hint: result.ok
    ? `Vault capture view: ${result.linkPath} → ${result.sharedRoot}`
    : result.error,
}, null, 2));
process.exit(result.ok ? 0 : 1);
