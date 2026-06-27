export interface KiroUsageSummary {
  email?: string;
  planName?: string;
  creditsTotal?: number;
  creditsUsed?: number;
  bonusTotal?: number;
  bonusUsed?: number;
  usageResetAt?: number;
  bonusExpireDays?: number;
}

type Path = readonly string[];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function getPath(root: unknown, path: Path): unknown {
  let current = root;
  for (const key of path) {
    if (!isRecord(current)) return undefined;
    current = current[key];
  }
  return current;
}

function pickString(root: unknown, paths: Path[]): string | undefined {
  for (const path of paths) {
    const value = getPath(root, path);
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return undefined;
}

function pickNumber(root: unknown, paths: Path[]): number | undefined {
  for (const path of paths) {
    const value = getPath(root, path);
    if (typeof value === 'number' && Number.isFinite(value)) return value;
    if (typeof value === 'string') {
      const parsed = Number.parseFloat(value.trim());
      if (Number.isFinite(parsed)) return parsed;
    }
  }
  return undefined;
}

export function parseKiroTimestamp(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value)) {
    if (value <= 0) return undefined;
    return value > 10_000_000_000 ? Math.trunc(value) : Math.trunc(value * 1000);
  }

  if (typeof value === 'string' && value.trim()) {
    const trimmed = value.trim();
    const numeric = Number.parseInt(trimmed, 10);
    if (Number.isFinite(numeric) && /^\d+$/.test(trimmed)) return parseKiroTimestamp(numeric);
    const parsed = Date.parse(trimmed);
    return Number.isFinite(parsed) ? parsed : undefined;
  }

  return undefined;
}

function pickTimestamp(root: unknown, paths: Path[]): number | undefined {
  for (const path of paths) {
    const parsed = parseKiroTimestamp(getPath(root, path));
    if (parsed != null) return parsed;
  }
  return undefined;
}

function resolveUsageRoot(raw: unknown): unknown {
  if (isRecord(raw)) {
    if (raw['kiro.resourceNotifications.usageState'] != null) return raw['kiro.resourceNotifications.usageState'];
    if (raw.usageState != null) return raw.usageState;
  }
  return raw;
}

function findPrimaryBreakdown(root: unknown): unknown {
  if (!isRecord(root)) return undefined;
  const list = Array.isArray(root.usageBreakdownList)
    ? root.usageBreakdownList
    : Array.isArray(root.usageBreakdowns)
      ? root.usageBreakdowns
      : undefined;
  if (!list || list.length === 0) return undefined;
  return list.find((item) => isRecord(item) && typeof item.type === 'string' && item.type.toLowerCase() === 'credit') ?? list[0];
}

export function normalizeKiroPlan(value: string | undefined | null): string | undefined {
  const upper = value?.trim().toUpperCase();
  if (!upper) return undefined;
  if (upper.includes('FREE') || upper.includes('STANDALONE')) return 'FREE';
  if (upper.includes('PRO')) return 'PRO';
  if (upper.includes('INDIVIDUAL')) return 'INDIVIDUAL';
  if (upper.includes('BUSINESS') || upper.includes('TEAM')) return 'BUSINESS';
  if (upper.includes('ENTERPRISE')) return 'ENTERPRISE';
  return upper;
}

export function buildKiroUsageSummary(raw: unknown, nowMs = Date.now()): KiroUsageSummary {
  const root = resolveUsageRoot(raw);
  const breakdown = findPrimaryBreakdown(root);
  const freeTrial = isRecord(breakdown)
    ? breakdown.freeTrialUsage ?? breakdown.freeTrialInfo
    : undefined;
  const expiry = pickTimestamp(freeTrial, [['expiryDate'], ['freeTrialExpiry']]);

  return {
    email: pickString(raw, [['userInfo', 'email'], ['email']]),
    planName: pickString(root, [
      ['planName'],
      ['currentPlanName'],
      ['subscriptionInfo', 'subscriptionName'],
      ['subscriptionInfo', 'subscriptionTitle'],
      ['subscriptionInfo', 'type'],
      ['usageBreakdowns', 'planName'],
      ['plan', 'name'],
    ]) ?? pickString(breakdown, [['displayName'], ['displayNamePlural'], ['type'], ['unit']]),
    creditsTotal: pickNumber(root, [
      ['estimatedUsage', 'total'],
      ['usageBreakdowns', 'plan', 'totalCredits'],
    ]) ?? pickNumber(breakdown, [['usageLimitWithPrecision'], ['usageLimit'], ['limit'], ['total'], ['totalCredits']]),
    creditsUsed: pickNumber(root, [
      ['estimatedUsage', 'used'],
      ['usageBreakdowns', 'plan', 'usedCredits'],
    ]) ?? pickNumber(breakdown, [['currentUsageWithPrecision'], ['currentUsage'], ['used'], ['usedCredits']]),
    bonusTotal: pickNumber(freeTrial, [['usageLimitWithPrecision'], ['usageLimit'], ['limit'], ['total']])
      ?? pickNumber(root, [['bonusCredits', 'total'], ['bonus', 'total']]),
    bonusUsed: pickNumber(freeTrial, [['currentUsageWithPrecision'], ['currentUsage'], ['used']])
      ?? pickNumber(root, [['bonusCredits', 'used'], ['bonus', 'used']]),
    bonusExpireDays: pickNumber(freeTrial, [['daysRemaining'], ['expiryDays'], ['expireDays']])
      ?? (expiry == null ? undefined : Math.max(0, Math.ceil((expiry - nowMs) / 86_400_000))),
    usageResetAt: pickTimestamp(root, [
      ['resetAt'],
      ['resetTime'],
      ['resetOn'],
      ['nextDateReset'],
      ['usageBreakdowns', 'resetAt'],
    ]) ?? pickTimestamp(breakdown, [['resetDate'], ['resetAt']]),
  };
}
