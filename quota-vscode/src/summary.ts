// @env node
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';

import { formatAntigravityCredits, type AntigravityCreditInfo } from './antigravityUsage';
import { PROVIDER_LABELS } from './constants';
import type { ProviderId, QuotaSnapshot, QuotaTrack, TrackId } from './types';

interface SafeSummaryPayload {
  exportedAt?: string;
  providers?: Record<string, unknown>;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function asArray(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value) ? value.filter(isRecord) : [];
}

function asNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

function asString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim().length > 0 ? value : undefined;
}

function clampPercent(value: number | undefined): number | undefined {
  if (value == null) return undefined;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function usedFromRemaining(value: unknown): number | undefined {
  const remaining = clampPercent(asNumber(value));
  return remaining == null ? undefined : 100 - remaining;
}

function normalizeTimestamp(value: unknown): number | null | undefined {
  const timestamp = asNumber(value);
  if (timestamp == null) return undefined;
  return timestamp > 0 && timestamp < 10_000_000_000 ? timestamp * 1000 : timestamp;
}

function accountLabel(account: Record<string, unknown>): string {
  return (
    asString(account.email)
    ?? asString(account.githubLogin)
    ?? asString(account.githubEmail)
    ?? asString(account.displayName)
    ?? asString(account.name)
    ?? 'Connected account'
  );
}

function makeTrack(
  id: TrackId,
  providerId: ProviderId,
  account: Record<string, unknown>,
  label: string,
  percentUsed?: number,
  percentRemaining?: number,
  resetAt?: number | null,
): QuotaTrack {
  return {
    id,
    providerId,
    providerLabel: PROVIDER_LABELS[providerId],
    label,
    accountLabel: accountLabel(account),
    percentUsed: clampPercent(percentUsed),
    percentRemaining: clampPercent(percentRemaining),
    resetAt,
    updatedAt: normalizeTimestamp(account.usageUpdatedAt),
    error: asString(account.quotaQueryLastError) ?? null,
  };
}

function tracksFromCopilot(accounts: Record<string, unknown>[]): QuotaTrack[] {
  return accounts.flatMap((account) => {
    const usage = isRecord(account.usage) ? account.usage : {};
    const resetAt = normalizeTimestamp(usage.allowanceResetAt);

    return [
      makeTrack(
        'githubCopilot.premium',
        'githubCopilot',
        account,
        'Premium requests',
        asNumber(usage.premiumRequestsUsedPercent),
        undefined,
        resetAt,
      ),
      makeTrack(
        'githubCopilot.chat',
        'githubCopilot',
        account,
        'Chat messages',
        asNumber(usage.chatMessagesUsedPercent),
        undefined,
        resetAt,
      ),
      makeTrack(
        'githubCopilot.inline',
        'githubCopilot',
        account,
        'Inline suggestions',
        asNumber(usage.inlineSuggestionsUsedPercent),
        undefined,
        resetAt,
      ),
    ];
  });
}

function tracksFromCodex(accounts: Record<string, unknown>[]): QuotaTrack[] {
  return accounts.flatMap((account) => {
    const quota = isRecord(account.quota) ? account.quota : {};

    return [
      makeTrack(
        'codex.primary',
        'codex',
        account,
        '5h usage',
        usedFromRemaining(quota.hourlyRemainingPercent),
        asNumber(quota.hourlyRemainingPercent),
        normalizeTimestamp(quota.hourlyResetAt),
      ),
      makeTrack(
        'codex.weekly',
        'codex',
        account,
        'Weekly usage',
        usedFromRemaining(quota.weeklyRemainingPercent),
        asNumber(quota.weeklyRemainingPercent),
        normalizeTimestamp(quota.weeklyResetAt),
      ),
    ];
  });
}

function tracksFromClaude(accounts: Record<string, unknown>[]): QuotaTrack[] {
  return accounts.flatMap((account) => {
    const quota = isRecord(account.quota) ? account.quota : {};

    return [
      makeTrack(
        'claude.fiveHour',
        'claude',
        account,
        '5h usage',
        usedFromRemaining(quota.fiveHourRemainingPercent),
        asNumber(quota.fiveHourRemainingPercent),
        normalizeTimestamp(quota.fiveHourResetAt),
      ),
      makeTrack(
        'claude.weekly',
        'claude',
        account,
        'Weekly usage',
        usedFromRemaining(quota.weeklyRemainingPercent),
        asNumber(quota.weeklyRemainingPercent),
        normalizeTimestamp(quota.weeklyResetAt),
      ),
    ];
  });
}

function tracksFromAntigravity(accounts: Record<string, unknown>[]): QuotaTrack[] {
  return accounts.flatMap((account) => {
    const quota = isRecord(account.quota) ? account.quota : {};
    const geminiFiveHour = isRecord(quota.geminiFiveHour) ? quota.geminiFiveHour : {};
    const thirdPartyFiveHour = isRecord(quota.thirdPartyFiveHour) ? quota.thirdPartyFiveHour : {};
    const credits = asArray(account.credits)
      .map((item): AntigravityCreditInfo | undefined => {
        const creditType = asString(item.creditType);
        const creditAmount = asString(item.creditAmount);
        if (!creditType || !creditAmount) return undefined;
        return {
          creditType,
          creditAmount,
          minimumCreditAmountForUsage: asString(item.minimumCreditAmountForUsage),
        };
      })
      .filter((item): item is AntigravityCreditInfo => item != null);
    const creditsValue = formatAntigravityCredits(credits);

    return [
      makeTrack(
        'antigravity.gemini',
        'antigravity',
        account,
        'Gemini Models',
        usedFromRemaining(geminiFiveHour.remainingPercent),
        asNumber(geminiFiveHour.remainingPercent),
        normalizeTimestamp(geminiFiveHour.resetAt),
      ),
      makeTrack(
        'antigravity.claude',
        'antigravity',
        account,
        'Claude/GPT models',
        usedFromRemaining(thirdPartyFiveHour.remainingPercent),
        asNumber(thirdPartyFiveHour.remainingPercent),
        normalizeTimestamp(thirdPartyFiveHour.resetAt),
      ),
      creditsValue
        ? {
            id: 'antigravity.credits',
            providerId: 'antigravity',
            providerLabel: PROVIDER_LABELS.antigravity,
            label: 'Available AI Credits',
            accountLabel: accountLabel(account),
            valueLabel: creditsValue,
            updatedAt: normalizeTimestamp(account.usageUpdatedAt),
            error: asString(account.quotaQueryLastError) ?? null,
          }
        : undefined,
    ].filter((track): track is QuotaTrack => track != null);
  });
}

function tracksFromKiro(accounts: Record<string, unknown>[]): QuotaTrack[] {
  return accounts.map((account) => {
    const used = asNumber(account.creditsUsed);
    const total = asNumber(account.creditsTotal);
    const percentUsed = used != null && total != null && total > 0 ? (used / total) * 100 : undefined;

    return makeTrack(
      'kiro.promptCredits',
      'kiro',
      account,
      'Prompt credits',
      percentUsed,
      percentUsed == null ? undefined : 100 - percentUsed,
      normalizeTimestamp(account.usageResetAt),
    );
  });
}

function payloadToTracks(payload: SafeSummaryPayload): QuotaTrack[] {
  const providers = isRecord(payload.providers) ? payload.providers : {};

  return [
    ...tracksFromCopilot(asArray(providers.githubCopilot)),
    ...tracksFromCodex(asArray(providers.codex)),
    ...tracksFromClaude(asArray(providers.claude)),
    ...tracksFromAntigravity(asArray(providers.antigravity)),
    ...tracksFromKiro(asArray(providers.kiro)),
  ].filter((track) => track.percentUsed != null || track.percentRemaining != null || track.valueLabel || track.error);
}

export function expandHome(inputPath: string): string {
  if (inputPath === '~') return os.homedir();
  if (inputPath.startsWith(`~${path.sep}`)) return path.join(os.homedir(), inputPath.slice(2));
  if (inputPath.startsWith('~/')) return path.join(os.homedir(), inputPath.slice(2));
  return inputPath;
}

export async function loadQuotaSnapshot(inputPath: string): Promise<QuotaSnapshot> {
  const sourcePath = expandHome(inputPath);
  const warnings: string[] = [];

  try {
    const raw = await fs.readFile(sourcePath, 'utf8');
    const parsed: unknown = JSON.parse(raw);

    if (!isRecord(parsed)) {
      return { sourcePath, tracks: [], warnings: ['The summary file is not a JSON object.'] };
    }

    return {
      sourcePath,
      exportedAt: asString(parsed.exportedAt),
      tracks: payloadToTracks(parsed),
      warnings,
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      sourcePath,
      tracks: [],
      warnings: [`Could not read the safe summary file: ${message}`],
    };
  }
}
