// @env node
import { PROVIDER_LABELS } from './constants';
import type { QuotaTrack } from './types';

export interface CodexQuotaSummary {
  hourlyRemainingPercent?: number | null;
  hourlyResetAt?: number | null;
  hourlyWindowMinutes?: number | null;
  weeklyRemainingPercent?: number | null;
  weeklyResetAt?: number | null;
  weeklyWindowMinutes?: number | null;
}

export interface CodexUsageResponse {
  plan_type?: string;
  rate_limit?: {
    primary_window?: WindowInfo;
    secondary_window?: WindowInfo;
  };
}

interface WindowInfo {
  used_percent?: number;
  limit_window_seconds?: number;
  reset_after_seconds?: number;
  reset_at?: number;
}

interface CodexTrackAccount {
  email: string;
  quota: CodexQuotaSummary;
  quotaQueryLastError?: string | null;
  usageUpdatedAt?: number | null;
}

function normalize(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}

function clampPercent(value: number | undefined): number | undefined {
  if (value == null || !Number.isFinite(value)) return undefined;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function remainingPercent(window: WindowInfo | undefined): number | undefined {
  return window == null ? undefined : 100 - Math.min(100, Math.max(0, Math.round(window.used_percent ?? 0)));
}

function windowMinutes(window: WindowInfo | undefined): number | undefined {
  const seconds = window?.limit_window_seconds;
  if (seconds == null || seconds <= 0) return undefined;
  return Math.ceil(seconds / 60);
}

function resetAt(window: WindowInfo | undefined, currentTime: number): number | undefined {
  if (window?.reset_at != null) return window.reset_at * 1000;
  if (window?.reset_after_seconds == null || window.reset_after_seconds < 0) return undefined;
  return currentTime + window.reset_after_seconds * 1000;
}

function trackFromAccount(
  account: CodexTrackAccount,
  id: 'codex.primary' | 'codex.weekly',
  label: string,
): QuotaTrack {
  const isPrimary = id === 'codex.primary';
  const remaining = isPrimary ? account.quota.hourlyRemainingPercent : account.quota.weeklyRemainingPercent;
  const reset = isPrimary ? account.quota.hourlyResetAt : account.quota.weeklyResetAt;
  const clampedRemaining = clampPercent(remaining ?? undefined);

  return {
    id,
    providerId: 'codex',
    providerLabel: PROVIDER_LABELS.codex,
    label,
    accountLabel: account.email,
    percentUsed: clampedRemaining == null ? undefined : 100 - clampedRemaining,
    percentRemaining: remaining ?? undefined,
    resetAt: reset,
    updatedAt: account.usageUpdatedAt,
    error: account.quotaQueryLastError ?? null,
  };
}

export function quotaFromUsage(
  value: CodexUsageResponse,
  currentTime = Date.now(),
): { plan?: string; quota: CodexQuotaSummary } {
  const primary = value.rate_limit?.primary_window;
  const secondary = value.rate_limit?.secondary_window;

  return {
    plan: normalize(value.plan_type),
    quota: {
      hourlyRemainingPercent: remainingPercent(primary),
      hourlyResetAt: resetAt(primary, currentTime),
      hourlyWindowMinutes: windowMinutes(primary),
      weeklyRemainingPercent: remainingPercent(secondary),
      weeklyResetAt: resetAt(secondary, currentTime),
      weeklyWindowMinutes: windowMinutes(secondary),
    },
  };
}

export function tracksFromCodexAccount(account: CodexTrackAccount): QuotaTrack[] {
  return [
    trackFromAccount(account, 'codex.primary', '5h usage'),
    trackFromAccount(account, 'codex.weekly', 'Weekly usage'),
  ].filter((track) => track.percentUsed != null || track.percentRemaining != null || track.error);
}
