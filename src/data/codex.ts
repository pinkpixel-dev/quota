import { invoke } from '@tauri-apps/api/core';

export interface CodexQuotaSummary {
  hourlyRemainingPercent?: number | null;
  hourlyResetAt?: number | null;
  hourlyWindowMinutes?: number | null;
  weeklyRemainingPercent?: number | null;
  weeklyResetAt?: number | null;
  weeklyWindowMinutes?: number | null;
}

export interface CodexAccountSummary {
  id: string;
  email: string;
  authMode: string;
  apiBaseUrl?: string | null;
  userId?: string | null;
  plan?: string | null;
  accountId?: string | null;
  organizationId?: string | null;
  quota: CodexQuotaSummary;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  usageUpdatedAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

export interface CodexOAuthStartResponse {
  loginId: string;
  authUrl: string;
  callbackUrl: string;
  expiresAt: number;
}

export function listCodexAccounts() {
  return invoke<CodexAccountSummary[]>('list_codex_accounts');
}

export function importCodexFromLocal() {
  return invoke<CodexAccountSummary>('import_codex_from_local');
}

export function startCodexOAuthLogin() {
  return invoke<CodexOAuthStartResponse>('codex_oauth_login_start');
}

export function completeCodexOAuthLogin(loginId: string) {
  return invoke<CodexAccountSummary>('codex_oauth_login_complete', { loginId });
}

export function cancelCodexOAuthLogin(loginId?: string | null) {
  return invoke<void>('codex_oauth_login_cancel', { loginId: loginId ?? null });
}

export function refreshCodexAccount(accountId: string) {
  return invoke<CodexAccountSummary>('refresh_codex_account', { accountId });
}

export function refreshAllCodexAccounts() {
  return invoke<CodexAccountSummary[]>('refresh_all_codex_accounts');
}

export function deleteCodexAccount(accountId: string) {
  return invoke<void>('delete_codex_account', { accountId });
}
