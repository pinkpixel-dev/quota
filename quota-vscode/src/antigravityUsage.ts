import { PROVIDER_LABELS } from './constants';
import type { QuotaTrack } from './types';

export interface AntigravityCreditInfo {
  creditType: string;
  creditAmount?: string;
  minimumCreditAmountForUsage?: string;
}

export interface AntigravityLoadStatus {
  tierId?: string;
  tierName?: string;
  projectId?: string;
  credits: AntigravityCreditInfo[];
}

export interface AntigravityCreditsAccount {
  id: string;
  email: string;
  name?: string;
  credits?: AntigravityCreditInfo[];
  quotaQueryLastError?: string | null;
  usageUpdatedAt?: number | null;
}

function normalize(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}

function readPath(root: unknown, path: string[]): unknown {
  let current = root;
  for (const key of path) {
    if (current == null || typeof current !== 'object') return undefined;
    current = (current as Record<string, unknown>)[key];
  }
  return current;
}

function readString(root: unknown, path: string[]): string | undefined {
  return normalize(readPath(root, path));
}

function readArray(root: unknown, path: string[]): unknown[] {
  const value = readPath(root, path);
  return Array.isArray(value) ? value : [];
}

function firstString(values: Array<string | undefined>): string | undefined {
  return values.find((value) => value != null);
}

function creditNumber(value: string | undefined): number | undefined {
  if (value == null) return undefined;
  const parsed = Number.parseFloat(value.replace(/,/g, '').trim());
  return Number.isFinite(parsed) ? parsed : undefined;
}

export function parseAntigravityLoadStatus(raw: unknown): AntigravityLoadStatus {
  const paidTier = readPath(raw, ['paidTier']);
  const currentTier = readPath(raw, ['currentTier']);
  const projectValue = readPath(raw, ['cloudaicompanionProject']);
  const credits = readArray(paidTier, ['availableCredits'])
    .map((item): AntigravityCreditInfo | undefined => {
      const creditType = readString(item, ['creditType']);
      const creditAmount = readString(item, ['creditAmount']);
      if (!creditType || !creditAmount) return undefined;
      return {
        creditType,
        creditAmount,
        minimumCreditAmountForUsage: readString(item, ['minimumCreditAmountForUsage']),
      };
    })
    .filter((item): item is AntigravityCreditInfo => item != null);

  return {
    tierId: firstString([
      readString(paidTier, ['id']),
      readString(currentTier, ['id']),
      readString(readArray(raw, ['allowedTiers'])[0], ['id']),
    ]),
    tierName: firstString([
      readString(paidTier, ['name']),
      readString(currentTier, ['name']),
    ]),
    projectId: normalize(projectValue)
      ?? readString(projectValue, ['id'])
      ?? readString(projectValue, ['projectId']),
    credits,
  };
}

export function formatAntigravityCredits(credits: AntigravityCreditInfo[] | undefined): string | undefined {
  let total = 0;
  let hasValidAmount = false;

  for (const credit of credits ?? []) {
    const parsed = creditNumber(credit.creditAmount);
    if (parsed == null) continue;
    total += parsed;
    hasValidAmount = true;
  }

  return hasValidAmount
    ? total.toLocaleString(undefined, { maximumFractionDigits: 2 })
    : undefined;
}

export function buildAntigravityCreditsTrack(account: AntigravityCreditsAccount): QuotaTrack | undefined {
  const valueLabel = formatAntigravityCredits(account.credits);
  if (!valueLabel) return undefined;

  return {
    id: 'antigravity.credits',
    providerId: 'antigravity',
    providerLabel: PROVIDER_LABELS.antigravity,
    label: 'Available AI Credits',
    accountLabel: account.name ? `${account.name} (${account.email})` : account.email,
    valueLabel,
    updatedAt: account.usageUpdatedAt,
    error: account.quotaQueryLastError ?? null,
  };
}
