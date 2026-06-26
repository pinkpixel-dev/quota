import { invoke } from '@tauri-apps/api/core';

export interface CursorAccountSummary {
  id: string;
  email: string;
  authId?: string | null;
  signUpType?: string | null;
  membershipType?: string | null;
  subscriptionStatus?: string | null;
  source: string;
  totalPercent?: number | null;
  autoPercent?: number | null;
  apiPercent?: number | null;
  billingCycleEnd?: number | null;
  planUsed?: number | null;
  planLimit?: number | null;
  onDemandEnabled?: boolean | null;
  onDemandUsed?: number | null;
  onDemandLimit?: number | null;
  quotaQueryLastError?: string | null;
  usageUpdatedAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

export interface CursorOAuthStartResponse {
  loginId: string;
  verificationUri: string;
  expiresIn: number;
  intervalSeconds: number;
}

export function listCursorAccounts() {
  return invoke<CursorAccountSummary[]>('list_cursor_accounts');
}

export function importCursorFromLocal() {
  return invoke<CursorAccountSummary>('import_cursor_from_local');
}

export function startCursorOAuthLogin() {
  return invoke<CursorOAuthStartResponse>('cursor_oauth_login_start');
}

export function completeCursorOAuthLogin(loginId: string) {
  return invoke<CursorAccountSummary>('cursor_oauth_login_complete', { loginId });
}

export function cancelCursorOAuthLogin(loginId?: string | null) {
  return invoke<void>('cursor_oauth_login_cancel', { loginId: loginId ?? null });
}

export function refreshCursorAccount(accountId: string) {
  return invoke<CursorAccountSummary>('refresh_cursor_account', { accountId });
}

export function refreshAllCursorAccounts() {
  return invoke<CursorAccountSummary[]>('refresh_all_cursor_accounts');
}

export function deleteCursorAccount(accountId: string) {
  return invoke<void>('delete_cursor_account', { accountId });
}

export function getCursorPlanBadge(membershipType?: string | null): string {
  if (!membershipType) return 'FREE';
  const lower = membershipType.toLowerCase();
  if (lower.includes('enterprise')) return 'ENTERPRISE';
  if (lower.includes('ultra')) return 'ULTRA';
  if (lower.includes('pro_plus') || lower.includes('pro+')) return 'PRO+';
  if (lower.includes('pro') || lower.includes('individual')) return 'PRO';
  if (lower.includes('team') || lower.includes('business')) return 'TEAM';
  return membershipType.toUpperCase();
}
