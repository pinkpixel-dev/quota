import { invoke } from '@tauri-apps/api/core';

export interface AntigravityQuotaWindow {
  remainingPercent?: number | null;
  resetAt?: number | null;
}

export interface AntigravityQuotaSummary {
  geminiFiveHour: AntigravityQuotaWindow;
  geminiWeekly: AntigravityQuotaWindow;
  thirdPartyFiveHour: AntigravityQuotaWindow;
  thirdPartyWeekly: AntigravityQuotaWindow;
}

export interface AntigravityAccountSummary {
  id: string;
  email: string;
  authId?: string | null;
  name?: string | null;
  source: string;
  selectedAuthType?: string | null;
  projectId?: string | null;
  tierId?: string | null;
  planName?: string | null;
  quota: AntigravityQuotaSummary;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  usageUpdatedAt?: number | null;
  status?: string | null;
  statusReason?: string | null;
  createdAt: number;
  lastUsed: number;
}

export interface AntigravityOAuthStartResponse {
  loginId: string;
  authUrl: string;
  callbackUrl: string;
  expiresAt: number;
}

export function listAntigravityAccounts() {
  return invoke<AntigravityAccountSummary[]>('list_antigravity_accounts');
}

export function importAntigravityFromLocal() {
  return invoke<AntigravityAccountSummary>('import_antigravity_from_local');
}

export function startAntigravityOAuthLogin() {
  return invoke<AntigravityOAuthStartResponse>('antigravity_oauth_login_start');
}

export function completeAntigravityOAuthLogin(loginId: string) {
  return invoke<AntigravityAccountSummary>('antigravity_oauth_login_complete', { loginId });
}

export function cancelAntigravityOAuthLogin(loginId?: string | null) {
  return invoke<void>('antigravity_oauth_login_cancel', { loginId: loginId ?? null });
}

export function refreshAntigravityAccount(accountId: string) {
  return invoke<AntigravityAccountSummary>('refresh_antigravity_account', { accountId });
}

export function refreshAllAntigravityAccounts() {
  return invoke<AntigravityAccountSummary[]>('refresh_all_antigravity_accounts');
}

export function deleteAntigravityAccount(accountId: string) {
  return invoke<void>('delete_antigravity_account', { accountId });
}
