export interface OpenCodeGoWindow {
  status?: string;
  percentUsed?: number;
  resetAt?: number;
}

export interface OpenCodeGoUsageSummary {
  rolling?: OpenCodeGoWindow;
  weekly?: OpenCodeGoWindow;
  monthly?: OpenCodeGoWindow;
}

const WINDOW_KEYS = ['rolling', 'weekly', 'monthly'] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parseWindow(raw: unknown): OpenCodeGoWindow | undefined {
  if (!isRecord(raw)) return undefined;
  const percent = typeof raw.percent === 'number' && Number.isFinite(raw.percent)
    ? Math.min(100, Math.max(0, raw.percent))
    : undefined;
  const resetAt = typeof raw.resetsAt === 'string' ? Date.parse(raw.resetsAt) : Number.NaN;
  return {
    status: typeof raw.status === 'string' && raw.status.trim() ? raw.status.trim() : undefined,
    percentUsed: percent,
    resetAt: Number.isFinite(resetAt) ? resetAt : undefined,
  };
}

/**
 * Parse `GET https://opencode.ai/zen/go/v1/usage`. The endpoint is undocumented,
 * so every field is optional, and a body with none of the three windows throws
 * instead of quietly reading as 0% used.
 */
export function parseOpenCodeGoUsage(raw: unknown): OpenCodeGoUsageSummary {
  const usage = isRecord(raw) ? raw.usage : undefined;
  if (!isRecord(usage)) throw new Error('OpenCode Go usage response did not include usage windows.');

  const summary: OpenCodeGoUsageSummary = {};
  for (const key of WINDOW_KEYS) {
    const window = parseWindow(usage[key]);
    if (window) summary[key] = window;
  }

  if (!summary.rolling && !summary.weekly && !summary.monthly) {
    throw new Error('OpenCode Go usage response did not include usage windows.');
  }
  return summary;
}

/** A user-readable message for a failed usage request. Never includes the body. */
export function openCodeGoErrorMessage(status: number): string {
  if (status === 401) return 'OpenCode Go rejected this API key. Check the key in the OpenCode console.';
  if (status === 403) return 'This API key has no OpenCode Go subscription.';
  if (status === 429) return 'OpenCode Go is rate limiting usage requests. Try again in a minute.';
  return `OpenCode Go usage returned ${status}.`;
}

/** Show only the last four characters, for labels and pickers. */
export function maskOpenCodeGoKey(apiKey: string): string {
  const trimmed = apiKey.trim();
  return `Go key ••••${trimmed.slice(-4)}`;
}
