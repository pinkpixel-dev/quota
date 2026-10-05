import type { AntigravityAccountSummary } from './antigravity';
import type { ClaudeAccountSummary } from './claude';
import type { CodexAccountSummary } from './codex';
import type { CursorAccountSummary } from './cursor';
import type { GitHubCopilotAccountSummary } from './githubCopilot';
import type { GrokAccountSummary } from './grok';
import type { KiroAccountSummary } from './kiro';
import type { OpenCodeGoAccountSummary } from './opencodeGo';

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

function remaining(label: string, value: number | null | undefined): string | null {
  return value == null ? null : `${label} ${clampPercent(value)}% left`;
}

function used(label: string, value: number | null | undefined): string | null {
  return value == null ? null : `${label} ${clampPercent(value)}% used`;
}

function money(value: number): string {
  return `$${value.toFixed(2)}`;
}

function compactRow(provider: string, account: string, metrics: Array<string | null>): string {
  const visibleMetrics = metrics.filter((metric): metric is string => Boolean(metric));
  return `${provider} · ${account || 'Account'} · ${visibleMetrics.join(' · ') || 'No usage data yet'}`;
}

function formatCopilotRows(accounts: GitHubCopilotAccountSummary[]): string[] {
  return accounts.map((account) => {
    const usage = account.usage;
    const premium = used('Premium', usage.premiumRequestsUsedPercent)
      ?? (usage.totalPremiumRequests != null && usage.usedPremiumRequests != null
        ? `Premium ${usage.usedPremiumRequests}/${usage.totalPremiumRequests} used`
        : usage.premiumIncluded ? 'Premium included' : null);
    const chat = used('Chat', usage.chatMessagesUsedPercent)
      ?? (usage.chatIncluded ? 'Chat included' : null);
    const inline = used('Inline', usage.inlineSuggestionsUsedPercent)
      ?? (usage.inlineIncluded ? 'Inline included' : null);

    return compactRow('Copilot', account.githubEmail || `@${account.githubLogin}`, [premium, chat, inline]);
  });
}

function formatCodexRows(accounts: CodexAccountSummary[]): string[] {
  return accounts.map((account) => compactRow('Codex', account.email, [
    remaining('5h', account.quota.hourlyRemainingPercent),
    remaining('Week', account.quota.weeklyRemainingPercent),
  ]));
}

function formatAntigravityRows(accounts: AntigravityAccountSummary[]): string[] {
  return accounts.map((account) => compactRow('Antigravity', account.email, [
    remaining('Gemini 5h', account.quota.geminiFiveHour.remainingPercent),
    remaining('Gemini week', account.quota.geminiWeekly.remainingPercent),
    remaining('Claude/GPT 5h', account.quota.thirdPartyFiveHour.remainingPercent),
    remaining('Claude/GPT week', account.quota.thirdPartyWeekly.remainingPercent),
  ]));
}

function formatClaudeRows(accounts: ClaudeAccountSummary[]): string[] {
  return accounts.map((account) => compactRow('Claude', account.email, [
    remaining('5h', account.quota.fiveHourRemainingPercent),
    remaining('Week', account.quota.weeklyRemainingPercent),
    remaining('Sonnet', account.quota.weeklySonnetRemainingPercent),
    remaining('Extra', account.quota.extraUsageRemainingPercent),
  ]));
}

function formatKiroRows(accounts: KiroAccountSummary[]): string[] {
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

function formatCursorRows(accounts: CursorAccountSummary[]): string[] {
  return accounts.map((account) => {
    const onDemand = account.onDemandEnabled
      ? `On-demand ${money(account.onDemandUsed ?? 0)}${account.onDemandLimit != null ? `/${money(account.onDemandLimit)}` : ''}`
      : account.onDemandEnabled === false ? 'On-demand off' : null;

    return compactRow('Cursor', account.email, [
      used('Total', account.totalPercent),
      used('Auto', account.autoPercent),
      used('API', account.apiPercent),
      onDemand,
    ]);
  });
}

function formatGrokRows(accounts: GrokAccountSummary[]): string[] {
  return accounts.map((account) => {
    const period = account.quota.periodLabel?.trim() || 'Credits';
    const monthly = account.quota.monthlyLimit != null && account.quota.monthlyLimit > 0
      ? `Month ${money(account.quota.monthlyUsed ?? 0)}/${money(account.quota.monthlyLimit)}`
      : null;
    const onDemand = account.quota.onDemandCap != null && account.quota.onDemandCap > 0
      ? `On-demand ${money(account.quota.onDemandUsed ?? 0)}/${money(account.quota.onDemandCap)}`
      : null;

    return compactRow('Grok', account.email, [
      remaining(period, account.quota.creditRemainingPercent),
      monthly,
      onDemand,
    ]);
  });
}

function formatOpenCodeGoRows(accounts: OpenCodeGoAccountSummary[]): string[] {
  return accounts.map((account) => compactRow('OpenCode Go', account.label, [
    remaining('5h', account.usage.fiveHour.remainingPercent),
    remaining('Week', account.usage.weekly.remainingPercent),
    remaining('Month', account.usage.monthly.remainingPercent),
  ]));
}

export function buildTrayUsageRows(
  accounts: TrayUsageAccounts,
  providerOrder: readonly TrayProviderKey[],
): string[] {
  const formatters: Record<TrayProviderKey, () => string[]> = {
    githubCopilot: () => formatCopilotRows(accounts.githubCopilot),
    codex: () => formatCodexRows(accounts.codex),
    antigravity: () => formatAntigravityRows(accounts.antigravity),
    claude: () => formatClaudeRows(accounts.claude),
    kiro: () => formatKiroRows(accounts.kiro),
    cursor: () => formatCursorRows(accounts.cursor),
    grok: () => formatGrokRows(accounts.grok),
    opencodeGo: () => formatOpenCodeGoRows(accounts.opencodeGo),
  };

  return providerOrder.flatMap((provider) => formatters[provider]());
}
