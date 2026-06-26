import { invoke } from "@tauri-apps/api/core";

export interface KiroAccountSummary {
  id: string;
  email: string;
  loginProvider: string | null;
  planName: string | null;
  creditsTotal: number | null;
  creditsUsed: number | null;
  bonusTotal: number | null;
  bonusUsed: number | null;
  usageResetAt: number | null;
  bonusExpireDays: number | null;
  status: string | null;
  statusReason: string | null;
  quotaQueryLastError: string | null;
  quotaQueryLastErrorAt: number | null;
  usageUpdatedAt: number | null;
  createdAt: number;
  lastUsed: number;
}

export interface KiroOAuthStartResponse {
  loginId: string;
  authUrl: string;
  callbackUrl: string;
  expiresAt: number;
}

export function listKiroAccounts(): Promise<KiroAccountSummary[]> {
  return invoke<KiroAccountSummary[]>("list_kiro_accounts");
}

export function importKiroFromLocal(): Promise<KiroAccountSummary[]> {
  return invoke<KiroAccountSummary[]>("import_kiro_from_local");
}

export function startKiroOAuthLogin(): Promise<KiroOAuthStartResponse> {
  return invoke<KiroOAuthStartResponse>("kiro_oauth_login_start");
}

export function completeKiroOAuthLogin(loginId: string): Promise<KiroAccountSummary> {
  return invoke<KiroAccountSummary>("kiro_oauth_login_complete", { loginId });
}

export function cancelKiroOAuthLogin(loginId?: string): Promise<void> {
  return invoke<void>("kiro_oauth_login_cancel", { loginId: loginId ?? null });
}

export function submitKiroOAuthCallbackUrl(loginId: string, callbackUrl: string): Promise<void> {
  return invoke<void>("kiro_oauth_submit_callback_url", { loginId, callbackUrl });
}

export function refreshKiroAccount(accountId: string): Promise<KiroAccountSummary> {
  return invoke<KiroAccountSummary>("refresh_kiro_account", { accountId });
}

export function refreshAllKiroAccounts(): Promise<KiroAccountSummary[]> {
  return invoke<KiroAccountSummary[]>("refresh_all_kiro_accounts");
}

export function deleteKiroAccount(accountId: string): Promise<void> {
  return invoke<void>("delete_kiro_account", { accountId });
}

export function getKiroPlanDisplayName(planName: string | null | undefined): string {
  if (!planName) return "FREE";
  const upper = planName.trim().toUpperCase();
  if (upper.includes("FREE") || upper.includes("STANDALONE")) return "FREE";
  if (upper.includes("PRO")) return "PRO";
  if (upper.includes("INDIVIDUAL")) return "INDIVIDUAL";
  if (upper.includes("BUSINESS") || upper.includes("TEAM")) return "BUSINESS";
  if (upper.includes("ENTERPRISE")) return "ENTERPRISE";
  return upper || "FREE";
}
