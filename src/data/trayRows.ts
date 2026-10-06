import type { AntigravityAccountSummary } from './antigravity';
import type { ClaudeAccountSummary } from './claude';
import type { CodexAccountSummary } from './codex';
import type { CursorAccountSummary } from './cursor';
import type { GitHubCopilotAccountSummary } from './githubCopilot';
import type { GrokAccountSummary } from './grok';
import type { KiroAccountSummary } from './kiro';
import type { OpenCodeGoAccountSummary } from './opencodeGo';
import type { UsageDisplayMode } from './usageDisplay';

export type TrayProviderKey =
  | 'githubCopilot'
  | 'codex'
  | 'antigravity'
  | 'claude'
  | 'kiro'
  | 'cursor'
  | 'grok'
  | 'opencodeGo';

export interface TrayUsageAccounts {
  githubCopilot: GitHubCopilotAccountSummary[];
  codex: CodexAccountSummary[];
  antigravity: AntigravityAccountSummary[];
  claude: ClaudeAccountSummary[];
  kiro: KiroAccountSummary[];
  cursor: CursorAccountSummary[];
  grok: GrokAccountSummary[];
  opencodeGo: OpenCodeGoAccountSummary[];
}

function clampPercent(value: number): number {
  return Math.round(Math.max(0, Math.min(100, value)));
}

// Value-only helpers stay local so this module runs under Node's type stripping in tests.
function percentText(label: string, usedPercent: number, mode: UsageDisplayMode): string {
  const usedValue = clampPercent(usedPercent);
  return mode === 'used' ? `${label} ${usedValue}% used` : `${label} ${100 - usedValue}% left`;
}

function remaining(label: string, value: number | null | undefined, mode: UsageDisplayMode): string | null {
  return value == null ? null : percentText(label, 100 - value, mode);
}

function used(label: string, value: number | null | undefined, mode: UsageDisplayMode): string | null {
  return value == null ? null : percentText(label, value, mode);
}

function money(value: number): string {
  return `$${value.toFixed(2)}`;
}

function compactRow(provider: string, account: string, metrics: Array<string | null>): string {
  const visibleMetrics = metrics.filter((metric): metric is string => Boolean(metric));
  return `${provider} · ${account || 'Account'} · ${visibleMetrics.join(' · ') || 'No usage data yet'}`;
}

function formatCopilotRows(accounts: GitHubCopilotAccountSummary[], mode: UsageDisplayMode): string[] {
  return accounts.map((account) => {
    const usage = account.usage;
    const premium = used('Premium', usage.premiumRequestsUsedPercent, mode)
      ?? (usage.totalPremiumRequests != null && usage.usedPremiumRequests != null
        ? `Premium ${usage.usedPremiumRequests}/${usage.totalPremiumRequests} used`
        : usage.premiumIncluded ? 'Premium included' : null);
    const chat = used('Chat', usage.chatMessagesUsedPercent, mode)
      ?? (usage.chatIncluded ? 'Chat included' : null);
    const inline = used('Inline', usage.inlineSuggestionsUsedPercent, mode)
      ?? (usage.inlineIncluded ? 'Inline included' : null);

    return compactRow('Copilot', account.githubEmail || `@${account.githubLogin}`, [premium, chat, inline]);
  });
}

function formatCodexRows(accounts: CodexAccountSummary[], mode: UsageDisplayMode): string[] {
  return accounts.map((account) => compactRow('Codex', account.email, [
    remaining('5h', account.quota.hourlyRemainingPercent, mode),
    remaining('Week', account.quota.weeklyRemainingPercent, mode),
  ]));
}

function formatAntigravityRows(accounts: AntigravityAccountSummary[], mode: UsageDisplayMode): string[] {
  return accounts.map((account) => compactRow('Antigravity', account.email, [
    remaining('Gemini 5h', account.quota.geminiFiveHour.remainingPercent, mode),
    remaining('Gemini week', account.quota.geminiWeekly.remainingPercent, mode),
    remaining('Claude/GPT 5h', account.quota.thirdPartyFiveHour.remainingPercent, mode),
    remaining('Claude/GPT week', account.quota.thirdPartyWeekly.remainingPercent, mode),
  ]));
}

function formatClaudeRows(accounts: ClaudeAccountSummary[], mode: UsageDisplayMode): string[] {
  return accounts.map((account) => compactRow('Claude', account.email, [
    remaining('5h', account.quota.fiveHourRemainingPercent, mode),
    remaining('Week', account.quota.weeklyRemainingPercent, mode),
    remaining('Sonnet', account.quota.weeklySonnetRemainingPercent, mode),
    remaining('Extra', account.quota.extraUsageRemainingPercent, mode),
  ]));
}

function formatKiroRows(accounts: KiroAccountSummary[], mode: UsageDisplayMode): string[] {
  return accounts.map((account) => {
    const credits = account.creditsTotal != null && account.creditsUsed != null
      ? `Credits ${Math.max(0, account.creditsTotal - account.creditsUsed)}/${account.creditsTotal} left`
      : null;
    const bonus = account.bonusTotal != null && account.bonusUsed != null
      ? `Add-on ${Math.max(0, account.bonusTotal - account.bonusUsed)}/${account.bonusTotal} left`
      : null;

    return compactRow('Kiro', account.email, [credits, bonus]);
  });
}

function formatCursorRows(accounts: CursorAccountSummary[], mode: UsageDisplayMode): string[] {
  return accounts.map((account) => {
    const onDemand = account.onDemandEnabled
      ? `On-demand ${money(account.onDemandUsed ?? 0)}${account.onDemandLimit != null ? `/${money(account.onDemandLimit)}` : ''}`
      : account.onDemandEnabled === false ? 'On-demand off' : null;

    return compactRow('Cursor', account.email, [
      used('Total', account.totalPercent, mode),
      used('Auto', account.autoPercent, mode),
      used('API', account.apiPercent, mode),
      onDemand,
    ]);
  });
}

function formatGrokRows(accounts: GrokAccountSummary[], mode: UsageDisplayMode): string[] {
  return accounts.map((account) => {
    const period = account.quota.periodLabel?.trim() || 'Credits';
    const monthly = account.quota.monthlyLimit != null && account.quota.monthlyLimit > 0
      ? `Month ${money(account.quota.monthlyUsed ?? 0)}/${money(account.quota.monthlyLimit)}`
      : null;
    const onDemand = account.quota.onDemandCap != null && account.quota.onDemandCap > 0
      ? `On-demand ${money(account.quota.onDemandUsed ?? 0)}/${money(account.quota.onDemandCap)}`
      : null;

    return compactRow('Grok', account.email, [
      remaining(period, account.quota.creditRemainingPercent, mode),
      monthly,
      onDemand,
    ]);
  });
}

function formatOpenCodeGoRows(accounts: OpenCodeGoAccountSummary[], mode: UsageDisplayMode): string[] {
  return accounts.map((account) => compactRow('OpenCode Go', account.label, [
    remaining('5h', account.usage.fiveHour.remainingPercent, mode),
    remaining('Week', account.usage.weekly.remainingPercent, mode),
    remaining('Month', account.usage.monthly.remainingPercent, mode),
  ]));
}

export function buildTrayUsageRows(
  accounts: TrayUsageAccounts,
  providerOrder: readonly TrayProviderKey[],
  mode: UsageDisplayMode,
): string[] {
  const formatters: Record<TrayProviderKey, () => string[]> = {
    githubCopilot: () => formatCopilotRows(accounts.githubCopilot, mode),
    codex: () => formatCodexRows(accounts.codex, mode),
    antigravity: () => formatAntigravityRows(accounts.antigravity, mode),
    claude: () => formatClaudeRows(accounts.claude, mode),
    kiro: () => formatKiroRows(accounts.kiro, mode),
    cursor: () => formatCursorRows(accounts.cursor, mode),
    grok: () => formatGrokRows(accounts.grok, mode),
    opencodeGo: () => formatOpenCodeGoRows(accounts.opencodeGo, mode),
  };

  return providerOrder.flatMap((provider) => formatters[provider]());
}
