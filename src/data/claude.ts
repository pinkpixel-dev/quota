import { invoke } from '@tauri-apps/api/core';

export interface ClaudeQuotaSummary {
  fiveHourRemainingPercent?: number | null;
  fiveHourResetAt?: number | null;
  weeklyRemainingPercent?: number | null;
  weeklyResetAt?: number | null;
  weeklySonnetRemainingPercent?: number | null;
  weeklySonnetResetAt?: number | null;
  extraUsageRemainingPercent?: number | null;
  extraUsageResetAt?: number | null;
  extraUsageUsedCents?: number | null;
  extraUsageLimitCents?: number | null;
}

export interface ClaudeAccountSummary {
  id: string;
  email: string;
  authMode: string;
  accountUuid?: string | null;
  organizationUuid?: string | null;
  organizationName?: string | null;
  displayName?: string | null;
  avatarUrl?: string | null;
  planType?: string | null;
  quota: ClaudeQuotaSummary;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  usageUpdatedAt?: number | null;
  profileUpdatedAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

export interface ClaudeOAuthStartResponse {
  loginId: string;
  authUrl: string;
  callbackUrl: string;
  expiresAt: number;
}

export function listClaudeAccounts() {
  return invoke<ClaudeAccountSummary[]>('list_claude_accounts');
}

export function startClaudeOAuthLogin() {
  return invoke<ClaudeOAuthStartResponse>('claude_oauth_login_start');
}

export function completeClaudeOAuthLogin(loginId: string, callbackOrCode: string, emailHint?: string | null) {
  return invoke<ClaudeAccountSummary>('claude_oauth_login_complete', {
    loginId,
    callbackOrCode,
    emailHint: emailHint?.trim() || null,
  });
}

export function cancelClaudeOAuthLogin(loginId?: string | null) {
  return invoke<void>('claude_oauth_login_cancel', { loginId: loginId ?? null });
}

export function refreshClaudeAccount(accountId: string) {
  return invoke<ClaudeAccountSummary>('refresh_claude_account', { accountId });
}

export function refreshAllClaudeAccounts() {
  return invoke<ClaudeAccountSummary[]>('refresh_all_claude_accounts');
}

export function deleteClaudeAccount(accountId: string) {
  return invoke<void>('delete_claude_account', { accountId });
}
