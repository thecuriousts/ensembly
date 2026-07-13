/**
 * Shared micro-capture (premflow) — paths + ensembly wrapper.
 */
export {
  PREMFLOW_DATA_DIRNAME,
  LIFE_OS_CAPTURE_LINK_NAME,
  SHARED_CAPTURE_ENTRIES,
  resolveSharedCaptureRoot,
  resolveLifeOsPremflowCard,
  resolveLifeOsCaptureLinkPath,
  resolveSharedCapturePaths,
  isCaptureLinkHealthy,
  ensureLifeOsCaptureLink,
  readSharedCaptureSnapshot,
} from './paths.js';
export {
  PREMFLOW_FORWARD_SUBS,
  resolvePremflowBin,
  buildPremflowSpawn,
  runPremflow,
  sharedCaptureStatus,
} from './wrapper.js';
export {
  classifyCaptureLine,
  redactCaptureLine,
  projectCaptureForShare,
} from './redact.js';
