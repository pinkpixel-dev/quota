import { invoke } from '@tauri-apps/api/core';

export interface GitHubCopilotOAuthStartResponse {
  loginId: string;
  userCode: string;
  verificationUri: string;
  verificationUriComplete?: string | null;
  expiresIn: number;
  intervalSeconds: number;
}

export interface GitHubCopilotUsageSummary {
  inlineSuggestionsUsedPercent?: number | null;
  chatMessagesUsedPercent?: number | null;
  premiumRequestsUsedPercent?: number | null;
  inlineIncluded: boolean;
  chatIncluded: boolean;
  premiumIncluded: boolean;
  remainingCompletions?: number | null;
  remainingChat?: number | null;
  remainingPremiumRequests?: number | null;
  totalCompletions?: number | null;
  totalChat?: number | null;
  totalPremiumRequests?: number | null;
  usedPremiumRequests?: number | null;
  allowanceResetAt?: number | null;
}

export interface GitHubCopilotAccountSummary {
  id: string;
  githubLogin: string;
  githubName?: string | null;
  githubEmail?: string | null;
  plan?: string | null;
  chatEnabled?: boolean | null;
  usage: GitHubCopilotUsageSummary;
  usageUpdatedAt?: number | null;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

export function listGitHubCopilotAccounts() {
  return invoke<GitHubCopilotAccountSummary[]>('list_github_copilot_accounts');
}

export function startGitHubCopilotLogin() {
  return invoke<GitHubCopilotOAuthStartResponse>('github_copilot_oauth_login_start');
}

export function completeGitHubCopilotLogin(loginId: string) {
  return invoke<GitHubCopilotAccountSummary>('github_copilot_oauth_login_complete', { loginId });
}

export function cancelGitHubCopilotLogin(loginId?: string | null) {
  return invoke<void>('github_copilot_oauth_login_cancel', { loginId: loginId ?? null });
}

export function refreshGitHubCopilotAccount(accountId: string) {
  return invoke<GitHubCopilotAccountSummary>('refresh_github_copilot_account', { accountId });
}

export function refreshAllGitHubCopilotAccounts() {
  return invoke<GitHubCopilotAccountSummary[]>('refresh_all_github_copilot_accounts');
}

export function deleteGitHubCopilotAccount(accountId: string) {
  return invoke<void>('delete_github_copilot_account', { accountId });
}
