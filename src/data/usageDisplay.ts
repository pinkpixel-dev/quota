export type UsageDisplayMode = 'remaining' | 'used';

export const USAGE_DISPLAY_MODES: UsageDisplayMode[] = ['remaining', 'used'];
export const DEFAULT_USAGE_DISPLAY_MODE: UsageDisplayMode = 'remaining';
// Rows turn red once this much or less is left, whichever way they're displayed.
export const LOW_REMAINING_PERCENT = 20;

const USAGE_DISPLAY_KEY = 'quota.usageDisplay';

export function readStoredUsageDisplayMode(): UsageDisplayMode {
  try {
    const stored = window.localStorage.getItem(USAGE_DISPLAY_KEY);
    return USAGE_DISPLAY_MODES.includes(stored as UsageDisplayMode) ? (stored as UsageDisplayMode) : DEFAULT_USAGE_DISPLAY_MODE;
  } catch {
    return DEFAULT_USAGE_DISPLAY_MODE;
  }
}

export function storeUsageDisplayMode(value: UsageDisplayMode) {
  try {
    window.localStorage.setItem(USAGE_DISPLAY_KEY, value);
  } catch {
    // Preference persistence is best-effort so private browsing/storage errors do not break the UI.
  }
}

export function clampPercent(value: number): number {
  return Math.round(Math.max(0, Math.min(100, value)));
}

/** The percent to show for a metric whose source value is percent used. */
export function displayPercentFromUsed(usedPercent: number, mode: UsageDisplayMode): number {
  const used = clampPercent(usedPercent);
  return mode === 'used' ? used : 100 - used;
}

export function usageSuffix(mode: UsageDisplayMode): string {
  return mode === 'used' ? 'used' : 'left';
}

export function formatUsageDisplayMode(mode: UsageDisplayMode): string {
  return mode === 'used' ? 'Used' : 'Remaining';
}
