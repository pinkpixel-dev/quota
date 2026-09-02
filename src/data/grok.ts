import { invoke } from '@tauri-apps/api/core';

export interface GrokProductUsage {
  product: string;
  usedPercent: number;
  remainingPercent: number;
}

export interface GrokQuotaSummary {
  creditRemainingPercent?: number | null;
  creditUsedPercent?: number | null;
  periodLabel?: string | null;
  periodStartAt?: number | null;
  periodResetAt?: number | null;
  periodWindowMinutes?: number | null;
  monthlyUsed?: number | null;
  monthlyLimit?: number | null;
  monthlyPeriodStartAt?: number | null;
  monthlyPeriodEndAt?: number | null;
  onDemandUsed?: number | null;
  onDemandCap?: number | null;
  prepaidBalance?: number | null;
  productUsage: GrokProductUsage[];
}

export interface GrokAccountSummary {
  id: string;
  email: string;
  displayName?: string | null;
  userId?: string | null;
  teamId?: string | null;
  teamName?: string | null;
  organizationId?: string | null;
  organizationName?: string | null;
  plan?: string | null;
  tier?: number | null;
  hasGrokCodeAccess: boolean;
  quota: GrokQuotaSummary;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  requiresReauthentication: boolean;
  usageUpdatedAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

export interface GrokOAuthStartResponse {
  loginId: string;
  authUrl: string;
  callbackUrl: string;
  userCode: string;
  expiresAt: number;
}

export function listGrokAccounts() {
  return invoke<GrokAccountSummary[]>('list_grok_accounts');
}

export function importGrokFromLocal() {
  return invoke<GrokAccountSummary[]>('import_grok_from_local');
}

export function startGrokOAuthLogin() {
  return invoke<GrokOAuthStartResponse>('grok_oauth_login_start');
}

export function completeGrokOAuthLogin(loginId: string) {
  return invoke<GrokAccountSummary>('grok_oauth_login_complete', { loginId });
}

export function cancelGrokOAuthLogin(loginId?: string | null) {
  return invoke<void>('grok_oauth_login_cancel', { loginId: loginId ?? null });
}

export function refreshGrokAccount(accountId: string) {
  return invoke<GrokAccountSummary>('refresh_grok_account', { accountId });
}

export function refreshAllGrokAccounts() {
  return invoke<GrokAccountSummary[]>('refresh_all_grok_accounts');
}

export function deleteGrokAccount(accountId: string) {
  return invoke<void>('delete_grok_account', { accountId });
}

export function getGrokPlanDisplayName(
  plan: string | null | undefined,
  tier: number | null | undefined,
): string {
  const normalized = plan?.trim();
  if (normalized) {
    return normalized.toUpperCase();
  }
  if (tier != null) {
    return `TIER ${tier}`;
  }
  return 'GROK CLI';
}
