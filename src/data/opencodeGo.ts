import { invoke } from '@tauri-apps/api/core';

export interface OpenCodeGoWindow {
  usedPercent: number | null;
  remainingPercent: number | null;
  resetAt: number | null;
}

export interface OpenCodeGoUsage {
  fiveHour: OpenCodeGoWindow;
  weekly: OpenCodeGoWindow;
  monthly: OpenCodeGoWindow;
}

export interface OpenCodeGoAccountSummary {
  id: string;
  label: string;
  usage: OpenCodeGoUsage;
  quotaQueryLastError: string | null;
  quotaQueryLastErrorAt: number | null;
  usageUpdatedAt: number | null;
  createdAt: number;
  lastUsed: number;
}

export function listOpenCodeGoAccounts() {
  return invoke<OpenCodeGoAccountSummary[]>('list_opencode_go_accounts');
}

export function addOpenCodeGoAccount(apiKey: string, name?: string | null) {
  return invoke<OpenCodeGoAccountSummary>('add_opencode_go_account', { apiKey, name: name ?? null });
}

export function refreshOpenCodeGoAccount(accountId: string) {
  return invoke<OpenCodeGoAccountSummary>('refresh_opencode_go_account', { accountId });
}

export function refreshAllOpenCodeGoAccounts() {
  return invoke<OpenCodeGoAccountSummary[]>('refresh_all_opencode_go_accounts');
}

export function deleteOpenCodeGoAccount(accountId: string) {
  return invoke<void>('delete_opencode_go_account', { accountId });
}
