// @env node
import * as crypto from 'node:crypto';

import * as vscode from 'vscode';

import { PROVIDER_LABELS } from './constants';
import { buildCopilotUsageSummary as buildUsageSummary, type GitHubCopilotUsageSummary } from './githubCopilotUsage';
import type { QuotaTrack } from './types';

const GITHUB_DEVICE_CODE_ENDPOINT = 'https://github.com/login/device/code';
const GITHUB_DEVICE_TOKEN_ENDPOINT = 'https://github.com/login/oauth/access_token';
const GITHUB_USER_ENDPOINT = 'https://api.github.com/user';
const GITHUB_USER_EMAILS_ENDPOINT = 'https://api.github.com/user/emails';
const GITHUB_COPILOT_TOKEN_ENDPOINT = 'https://api.github.com/copilot_internal/v2/token';
const GITHUB_COPILOT_USER_INFO_ENDPOINT = 'https://api.github.com/copilot_internal/user';
const GITHUB_OAUTH_CLIENT_ID = '01ab8ac9400c4e429b23';
const GITHUB_OAUTH_SCOPE = 'read:user user:email repo workflow';
const APP_USER_AGENT = 'quota-vscode';
const GITHUB_COPILOT_SECRET_KEY = 'quota.githubCopilot.credentials';
const GITHUB_COPILOT_ACCOUNTS_KEY = 'quota.githubCopilot.accounts';

interface GitHubCopilotCredentialStore {
  accounts: Record<string, GitHubCopilotCredential>;
}

interface GitHubCopilotCredential {
  id: string;
  githubAccessToken: string;
  githubTokenType?: string;
  githubScope?: string;
  copilotToken: string;
}

interface GitHubCopilotAccount {
  id: string;
  githubLogin: string;
  githubId: number;
  githubName?: string;
  githubEmail?: string;
  plan?: string;
  chatEnabled?: boolean;
  usage: GitHubCopilotUsageSummary;
  usageUpdatedAt?: number | null;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

interface DeviceCodeResponse {
  device_code: string;
  user_code: string;
  verification_uri: string;
  verification_uri_complete?: string;
  expires_in: number;
  interval?: number;
}

interface DeviceTokenResponse {
  access_token?: string;
  token_type?: string;
  scope?: string;
  error?: string;
  error_description?: string;
}

interface GitHubUser {
  id: number;
  login: string;
  name?: string | null;
  email?: string | null;
}

interface GitHubEmail {
  email: string;
  primary?: boolean;
  verified?: boolean;
}

interface CopilotTokenResponse {
  token?: string;
  expires_at?: number;
  refresh_in?: number;
  sku?: string;
  chat_enabled?: boolean;
  limited_user_quotas?: unknown;
  limited_user_reset_date?: number;
  message?: string;
}

interface CopilotUserInfoResponse {
  copilot_plan?: string;
  quota_snapshots?: unknown;
  quota_reset_date?: string;
}

interface CopilotTokenBundle {
  token: string;
  plan?: string;
  chatEnabled?: boolean;
  quotaSnapshots?: unknown;
  quotaResetDate?: string;
  limitedUserQuotas?: unknown;
  limitedUserResetDate?: number;
}

interface GitHubCopilotPayload {
  githubLogin: string;
  githubId: number;
  githubName?: string;
  githubEmail?: string;
  githubAccessToken: string;
  githubTokenType?: string;
  githubScope?: string;
  copilot: CopilotTokenBundle;
}

function now(): number {
  return Date.now();
}

function normalize(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}

function clampPercent(value: number | undefined): number | undefined {
  if (value == null || !Number.isFinite(value)) return undefined;
  return Math.min(100, Math.max(0, Math.round(value)));
}

async function parseJsonResponse<T>(response: Response, failureLabel: string): Promise<T> {
  const body = await response.text();
  if (!response.ok) throw new Error(`${failureLabel} returned ${response.status} with body length ${body.length}.`);
  return JSON.parse(body) as T;
}

async function requestDeviceCode(): Promise<DeviceCodeResponse> {
  const response = await fetch(GITHUB_DEVICE_CODE_ENDPOINT, {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/x-www-form-urlencoded',
      'User-Agent': APP_USER_AGENT,
    },
    body: new URLSearchParams({
      client_id: GITHUB_OAUTH_CLIENT_ID,
      scope: GITHUB_OAUTH_SCOPE,
    }).toString(),
  });

  return parseJsonResponse<DeviceCodeResponse>(response, 'GitHub device code request');
}

async function exchangeDeviceToken(deviceCode: string): Promise<DeviceTokenResponse> {
  const response = await fetch(GITHUB_DEVICE_TOKEN_ENDPOINT, {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/x-www-form-urlencoded',
      'User-Agent': APP_USER_AGENT,
    },
    body: new URLSearchParams({
      client_id: GITHUB_OAUTH_CLIENT_ID,
      device_code: deviceCode,
      grant_type: 'urn:ietf:params:oauth:grant-type:device_code',
    }).toString(),
  });

  return parseJsonResponse<DeviceTokenResponse>(response, 'GitHub access token request');
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function pollDeviceToken(deviceCode: string, intervalSeconds: number, expiresInSeconds: number): Promise<DeviceTokenResponse> {
  const expiresAt = now() + expiresInSeconds * 1000;
  let waitSeconds = Math.max(1, intervalSeconds);

  while (now() < expiresAt) {
    const response = await exchangeDeviceToken(deviceCode);
    if (!response.error) return response;

    if (response.error === 'authorization_pending') {
      await sleep(waitSeconds * 1000);
      continue;
    }
    if (response.error === 'slow_down') {
      waitSeconds += 5;
      await sleep(waitSeconds * 1000);
      continue;
    }
    if (response.error === 'expired_token') throw new Error('GitHub authorization expired. Start again.');
    if (response.error === 'access_denied') throw new Error('GitHub authorization was denied.');
    throw new Error(response.error_description ?? `GitHub authorization failed: ${response.error}`);
  }

  throw new Error('GitHub authorization expired. Start again.');
}

async function fetchGitHubUser(githubAccessToken: string): Promise<GitHubUser> {
  const response = await fetch(GITHUB_USER_ENDPOINT, {
    headers: {
      Accept: 'application/vnd.github+json',
      Authorization: `Bearer ${githubAccessToken}`,
      'User-Agent': APP_USER_AGENT,
    },
  });
  return parseJsonResponse<GitHubUser>(response, 'GitHub user request');
}

async function fetchGitHubEmail(githubAccessToken: string): Promise<string | undefined> {
  const response = await fetch(GITHUB_USER_EMAILS_ENDPOINT, {
    headers: {
      Accept: 'application/vnd.github+json',
      Authorization: `Bearer ${githubAccessToken}`,
      'User-Agent': APP_USER_AGENT,
    },
  });
  const emails = await parseJsonResponse<GitHubEmail[]>(response, 'GitHub email request');
  return emails.find((item) => item.primary && item.verified)?.email
    ?? emails.find((item) => item.verified)?.email;
}

async function fetchCopilotUserInfo(githubAccessToken: string): Promise<CopilotUserInfoResponse | undefined> {
  const response = await fetch(GITHUB_COPILOT_USER_INFO_ENDPOINT, {
    headers: {
      Accept: 'application/json',
      Authorization: `token ${githubAccessToken}`,
      'User-Agent': APP_USER_AGENT,
      'X-GitHub-Api-Version': '2025-04-01',
    },
  });
  if (!response.ok) return undefined;
  return response.json() as Promise<CopilotUserInfoResponse>;
}

async function fetchCopilotToken(githubAccessToken: string): Promise<CopilotTokenBundle> {
  const response = await fetch(GITHUB_COPILOT_TOKEN_ENDPOINT, {
    headers: {
      Accept: 'application/json',
      Authorization: `token ${githubAccessToken}`,
      'User-Agent': APP_USER_AGENT,
      'X-GitHub-Api-Version': '2025-04-01',
    },
  });
  const payload = await parseJsonResponse<CopilotTokenResponse>(response, 'Copilot token request');
  const token = normalize(payload.token);
  if (!token) throw new Error(payload.message ?? 'Copilot token missing.');
  const userInfo = await fetchCopilotUserInfo(githubAccessToken);

  return {
    token,
    plan: normalize(userInfo?.copilot_plan) ?? normalize(payload.sku),
    chatEnabled: payload.chat_enabled,
    quotaSnapshots: userInfo?.quota_snapshots,
    quotaResetDate: userInfo?.quota_reset_date,
    limitedUserQuotas: payload.limited_user_quotas,
    limitedUserResetDate: payload.limited_user_reset_date,
  };
}

async function buildPayloadFromGitHubAccessToken(
  githubAccessToken: string,
  githubTokenType?: string,
  githubScope?: string,
): Promise<GitHubCopilotPayload> {
  const user = await fetchGitHubUser(githubAccessToken);
  const email = await fetchGitHubEmail(githubAccessToken).catch(() => undefined);
  const copilot = await fetchCopilotToken(githubAccessToken);

  return {
    githubLogin: user.login,
    githubId: user.id,
    githubName: normalize(user.name),
    githubEmail: email ?? normalize(user.email),
    githubAccessToken,
    githubTokenType,
    githubScope,
    copilot,
  };
}

function accountId(payload: Pick<GitHubCopilotPayload, 'githubLogin' | 'githubId'>): string {
  return `ghcp_${crypto.createHash('md5').update(`${payload.githubLogin}:${payload.githubId}`).digest('hex')}`;
}

function accountLabel(account: GitHubCopilotAccount): string {
  return account.githubEmail ?? account.githubLogin;
}

function accountFromPayload(payload: GitHubCopilotPayload, existing?: GitHubCopilotAccount): { account: GitHubCopilotAccount; credential: GitHubCopilotCredential } {
  const id = existing?.id ?? accountId(payload);
  const usage = buildUsageSummary({
    copilotToken: payload.copilot.token,
    quotaSnapshots: payload.copilot.quotaSnapshots,
    quotaResetDate: payload.copilot.quotaResetDate,
    limitedUserQuotas: payload.copilot.limitedUserQuotas,
    limitedUserResetDate: payload.copilot.limitedUserResetDate,
  });

  return {
    account: {
      id,
      githubLogin: payload.githubLogin,
      githubId: payload.githubId,
      githubName: payload.githubName,
      githubEmail: payload.githubEmail,
      plan: payload.copilot.plan,
      chatEnabled: payload.copilot.chatEnabled,
      usage,
      usageUpdatedAt: now(),
      quotaQueryLastError: null,
      quotaQueryLastErrorAt: null,
      createdAt: existing?.createdAt ?? now(),
      lastUsed: now(),
    },
    credential: {
      id,
      githubAccessToken: payload.githubAccessToken,
      githubTokenType: payload.githubTokenType,
      githubScope: payload.githubScope,
      copilotToken: payload.copilot.token,
    },
  };
}

function trackFromAccount(account: GitHubCopilotAccount, id: 'githubCopilot.premium' | 'githubCopilot.chat' | 'githubCopilot.inline', label: string, percentUsed?: number): QuotaTrack {
  return {
    id,
    providerId: 'githubCopilot',
    providerLabel: PROVIDER_LABELS.githubCopilot,
    label,
    accountLabel: accountLabel(account),
    percentUsed: clampPercent(percentUsed),
    percentRemaining: percentUsed == null ? undefined : 100 - clampPercent(percentUsed)!,
    resetAt: account.usage.allowanceResetAt,
    updatedAt: account.usageUpdatedAt,
    error: account.quotaQueryLastError ?? null,
  };
}

export class GitHubCopilotProvider {
  constructor(private readonly context: vscode.ExtensionContext) {}

  async connect(): Promise<GitHubCopilotAccount> {
    const device = await requestDeviceCode();
    const authUri = device.verification_uri_complete ?? device.verification_uri;
    await vscode.env.openExternal(vscode.Uri.parse(authUri));
    void vscode.window.showInformationMessage(`GitHub Copilot device code: ${device.user_code}`);

    const tokenResponse = await pollDeviceToken(
      device.device_code,
      device.interval ?? 5,
      device.expires_in,
    );
    const accessToken = normalize(tokenResponse.access_token);
    if (!accessToken) throw new Error('GitHub access token missing.');

    const payload = await buildPayloadFromGitHubAccessToken(accessToken, tokenResponse.token_type, tokenResponse.scope);
    return this.upsertPayload(payload);
  }

  async refreshAll(): Promise<GitHubCopilotAccount[]> {
    const accounts = await this.getAccounts();
    const refreshed: GitHubCopilotAccount[] = [];
    for (const account of accounts) {
      refreshed.push(await this.refreshAccount(account.id));
    }
    return refreshed;
  }

  async refreshAccount(accountId: string): Promise<GitHubCopilotAccount> {
    const account = await this.getAccount(accountId);
    const credential = await this.getCredential(accountId);
    if (!account || !credential) throw new Error('GitHub Copilot account is not connected.');

    try {
      const copilot = await fetchCopilotToken(credential.githubAccessToken);
      const payload: GitHubCopilotPayload = {
        githubLogin: account.githubLogin,
        githubId: account.githubId,
        githubName: account.githubName,
        githubEmail: account.githubEmail,
        githubAccessToken: credential.githubAccessToken,
        githubTokenType: credential.githubTokenType,
        githubScope: credential.githubScope,
        copilot,
      };
      return await this.upsertPayload(payload, account);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return await this.saveAccount({
        ...account,
        quotaQueryLastError: message,
        quotaQueryLastErrorAt: now(),
      });
    }
  }

  async disconnect(): Promise<void> {
    const accounts = await this.getAccounts();
    if (accounts.length === 0) {
      void vscode.window.showInformationMessage('No GitHub Copilot accounts are connected.');
      return;
    }

    const picked = await vscode.window.showQuickPick(
      accounts.map((account) => ({
        label: accountLabel(account),
        description: account.plan ?? 'GitHub Copilot account',
        account,
      })),
      { title: 'Disconnect GitHub Copilot' },
    );
    if (!picked) return;

    const confirmed = await vscode.window.showWarningMessage(
      `Disconnect GitHub Copilot account ${accountLabel(picked.account)}? Extension-stored tokens and cached quota data will be deleted.`,
      { modal: true },
      'Disconnect',
    );
    if (confirmed !== 'Disconnect') return;

    const store = await this.getCredentialStore();
    delete store.accounts[picked.account.id];
    await this.saveCredentialStore(store);
    await this.context.globalState.update(GITHUB_COPILOT_ACCOUNTS_KEY, accounts.filter((account) => account.id !== picked.account.id));
  }

  async getTracks(): Promise<QuotaTrack[]> {
    const accounts = await this.getAccounts();
    return accounts.flatMap((account) => [
      trackFromAccount(account, 'githubCopilot.premium', 'Premium requests', account.usage.premiumRequestsUsedPercent),
      trackFromAccount(account, 'githubCopilot.chat', 'Chat messages', account.usage.chatMessagesUsedPercent),
      trackFromAccount(account, 'githubCopilot.inline', 'Inline suggestions', account.usage.inlineSuggestionsUsedPercent),
    ]).filter((track) => track.percentUsed != null || track.percentRemaining != null || track.error);
  }

  async hasAccounts(): Promise<boolean> {
    return (await this.getAccounts()).length > 0;
  }

  private async upsertPayload(payload: GitHubCopilotPayload, existing?: GitHubCopilotAccount): Promise<GitHubCopilotAccount> {
    const result = accountFromPayload(payload, existing);
    const store = await this.getCredentialStore();
    store.accounts[result.account.id] = result.credential;
    await this.saveCredentialStore(store);
    return this.saveAccount(result.account);
  }

  private async getAccounts(): Promise<GitHubCopilotAccount[]> {
    return this.context.globalState.get<GitHubCopilotAccount[]>(GITHUB_COPILOT_ACCOUNTS_KEY, []);
  }

  private async getAccount(accountId: string): Promise<GitHubCopilotAccount | undefined> {
    return (await this.getAccounts()).find((account) => account.id === accountId);
  }

  private async saveAccount(account: GitHubCopilotAccount): Promise<GitHubCopilotAccount> {
    const accounts = await this.getAccounts();
    const next = [account, ...accounts.filter((item) => item.id !== account.id)];
    await this.context.globalState.update(GITHUB_COPILOT_ACCOUNTS_KEY, next);
    return account;
  }

  private async getCredential(accountId: string): Promise<GitHubCopilotCredential | undefined> {
    const store = await this.getCredentialStore();
    return store.accounts[accountId];
  }

  private async getCredentialStore(): Promise<GitHubCopilotCredentialStore> {
    const raw = await this.context.secrets.get(GITHUB_COPILOT_SECRET_KEY);
    if (!raw) return { accounts: {} };

    try {
      const parsed = JSON.parse(raw) as GitHubCopilotCredentialStore;
      return parsed && typeof parsed === 'object' && parsed.accounts ? parsed : { accounts: {} };
    } catch {
      return { accounts: {} };
    }
  }

  private async saveCredentialStore(store: GitHubCopilotCredentialStore): Promise<void> {
    await this.context.secrets.store(GITHUB_COPILOT_SECRET_KEY, JSON.stringify(store));
  }
}
