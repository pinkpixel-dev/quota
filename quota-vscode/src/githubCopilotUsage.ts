export interface GitHubCopilotUsageSummary {
  inlineSuggestionsUsedPercent?: number;
  chatMessagesUsedPercent?: number;
  premiumRequestsUsedPercent?: number;
  inlineIncluded: boolean;
  chatIncluded: boolean;
  premiumIncluded: boolean;
  remainingCompletions?: number;
  remainingChat?: number;
  remainingPremiumRequests?: number;
  totalCompletions?: number;
  totalChat?: number;
  totalPremiumRequests?: number;
  usedPremiumRequests?: number;
  allowanceResetAt?: number;
}

interface BuildUsageInput {
  copilotToken: string;
  quotaSnapshots?: unknown;
  quotaResetDate?: string | null;
  limitedUserQuotas?: unknown;
  limitedUserResetDate?: number | null;
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
}

function getNumber(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value)) return Math.round(value);
  if (typeof value === 'string') {
    const parsed = Number.parseFloat(value);
    if (Number.isFinite(parsed)) return Math.round(parsed);
  }
  return undefined;
}

function clampPercent(value: number | undefined): number | undefined {
  if (value == null || !Number.isFinite(value)) return undefined;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function snapshot(input: unknown, key: string): Record<string, unknown> | undefined {
  return asRecord(asRecord(input)?.[key]);
}

function limitedQuota(input: unknown, key: string): number | undefined {
  return getNumber(asRecord(input)?.[key]);
}

function tokenValue(token: string, key: string): string | undefined {
  const prefix = token.split(':')[0] ?? token;
  for (const part of prefix.split(';')) {
    const [partKey, partValue] = part.split('=');
    if (partKey?.trim() === key) return partValue?.trim();
  }
  return undefined;
}

function resetFromToken(token: string): number | undefined {
  const value = tokenValue(token, 'rd');
  const head = value?.split(':')[0]?.trim();
  const parsed = head == null ? undefined : Number.parseInt(head, 10);
  return parsed != null && Number.isFinite(parsed) ? parsed * 1000 : undefined;
}

export function parseCopilotResetDate(date: string): number | undefined {
  const trimmed = date.trim();
  if (!trimmed) return undefined;
  if (/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) {
    const parsed = Date.parse(`${trimmed}T00:00:00Z`);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  const parsed = Date.parse(trimmed);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function percentFromSnapshot(item: Record<string, unknown> | undefined): number | undefined {
  if (!item) return undefined;
  if (item.unlimited === true) return 0;
  const entitlement = getNumber(item.entitlement);
  if (entitlement != null && entitlement < 0) return 0;
  const percentRemaining = getNumber(item.percent_remaining);
  return percentRemaining == null ? undefined : Math.min(100, Math.max(0, 100 - percentRemaining));
}

function includedFromSnapshot(item: Record<string, unknown> | undefined): boolean {
  if (!item) return false;
  if (item.unlimited === true) return true;
  const entitlement = getNumber(item.entitlement);
  return entitlement != null && entitlement < 0;
}

function remainingFromSnapshot(item: Record<string, unknown> | undefined): number | undefined {
  if (!item) return undefined;
  const remaining = getNumber(item.remaining);
  if (remaining != null) return remaining;
  const entitlement = getNumber(item.entitlement);
  const percentRemaining = getNumber(item.percent_remaining);
  if (entitlement == null || percentRemaining == null || entitlement <= 0) return undefined;
  return Math.round((entitlement * percentRemaining) / 100);
}

function usedPercent(total: number | undefined, remaining: number | undefined): number | undefined {
  if (total == null || remaining == null || total <= 0) return undefined;
  return clampPercent(((total - remaining) / total) * 100);
}

export function buildCopilotUsageSummary(input: BuildUsageInput): GitHubCopilotUsageSummary {
  const completionsSnapshot = snapshot(input.quotaSnapshots, 'completions');
  const chatSnapshot = snapshot(input.quotaSnapshots, 'chat');
  const premiumSnapshot = snapshot(input.quotaSnapshots, 'premium_interactions') ?? snapshot(input.quotaSnapshots, 'premium_models');

  const remainingCompletions = remainingFromSnapshot(completionsSnapshot) ?? limitedQuota(input.limitedUserQuotas, 'completions');
  const remainingChat = remainingFromSnapshot(chatSnapshot) ?? limitedQuota(input.limitedUserQuotas, 'chat');
  const remainingPremium = remainingFromSnapshot(premiumSnapshot);
  const totalCompletions = getNumber(completionsSnapshot?.entitlement) ?? remainingCompletions;
  const totalChat = getNumber(chatSnapshot?.entitlement) ?? remainingChat;
  const totalPremium = getNumber(premiumSnapshot?.entitlement) ?? remainingPremium;
  const exactRemainingPremium = getNumber(premiumSnapshot?.remaining);
  const resetFromDate = input.quotaResetDate ? parseCopilotResetDate(input.quotaResetDate) : undefined;
  const resetFromLimited = input.limitedUserResetDate != null ? input.limitedUserResetDate * 1000 : undefined;

  return {
    inlineSuggestionsUsedPercent: percentFromSnapshot(completionsSnapshot) ?? usedPercent(totalCompletions, remainingCompletions),
    chatMessagesUsedPercent: percentFromSnapshot(chatSnapshot) ?? usedPercent(totalChat, remainingChat),
    premiumRequestsUsedPercent: percentFromSnapshot(premiumSnapshot) ?? usedPercent(totalPremium, remainingPremium),
    inlineIncluded: includedFromSnapshot(completionsSnapshot),
    chatIncluded: includedFromSnapshot(chatSnapshot),
    premiumIncluded: includedFromSnapshot(premiumSnapshot),
    remainingCompletions,
    remainingChat,
    remainingPremiumRequests: exactRemainingPremium ?? remainingPremium,
    totalCompletions,
    totalChat,
    totalPremiumRequests: totalPremium,
    usedPremiumRequests: totalPremium != null && exactRemainingPremium != null ? Math.max(0, totalPremium - exactRemainingPremium) : undefined,
    allowanceResetAt: resetFromLimited ?? resetFromDate ?? resetFromToken(input.copilotToken),
  };
}
