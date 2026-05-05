import { writable } from 'svelte/store';

// ── Current Page ──────────────────────────────────────────
export const currentPage = writable<string>('dashboard');

// ── System Info ───────────────────────────────────────────
export const systemInfo = writable({
  os: '',
  arch: '',
  hostname: '',
  drives: [] as any[],
});

// ── Scan Progress ─────────────────────────────────────────
export const scanProgress = writable({
  inProgress: false,
  filesScanned: 0,
  dirsScanned: 0,
  totalSize: 0,
  percentage: 0,
  message: 'Ready',
});

// ── Analysis Results ──────────────────────────────────────
export const analysisResults = writable({
  totalFiles: 0,
  safeToDelete: 0,
  caution: 0,
  doNotDelete: 0,
  reviewManually: 0,
  reclaimableSpace: 0,
});

// ── Cleanup Status ────────────────────────────────────────
export const cleanupStatus = writable({
  inProgress: false,
  itemsProcessed: 0,
  itemsSucceeded: 0,
  itemsFailed: 0,
  spaceFreed: 0,
});

// ── Risk Tolerance ────────────────────────────────────────
export const riskTolerance = writable<string>('balanced');

// ── Quarantine ────────────────────────────────────────────
export const quarantineEntries = writable<any[]>([]);
