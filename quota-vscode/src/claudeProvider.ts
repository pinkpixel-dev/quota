// @env node
import * as crypto from 'node:crypto';

import * as vscode from 'vscode';

import { PROVIDER_LABELS } from './constants';
import type { QuotaTrack, TrackId } from './types';

const CLAUDE_OAUTH_AUTHORIZE_URL = 'https://claude.com/cai/oauth/authorize';
const CLAUDE_OAUTH_CALLBACK_URL = 'https://platform.claude.com/oauth/code/callback';
const CLAUDE_OAUTH_TOKEN_URL = 'https://platform.claude.com/v1/oauth/token';
const CLAUDE_OAUTH_PROFILE_URL = 'https://api.anthropic.com/api/oauth/profile';
const CLAUDE_OAUTH_USAGE_URL = 'https://api.anthropic.com/api/oauth/usage';
const CLAUDE_OAUTH_CLIENT_ID = '9d1c250a-e61b-44d9-88ed-5944d1962f5e';
const CLAUDE_OAUTH_BETA_HEADER = 'oauth-2025-04-20';
const CLAUDE_SECRET_KEY = 'quota.claude.credentials';
const CLAUDE_ACCOUNTS_KEY = 'quota.claude.accounts';
const CLAUDE_OAUTH_SCOPES = [
  'org:create_api_key',
  'user:profile',
  'user:inference',
  'user:sessions:claude_code',
  'user:mcp_servers',
  'user:file_upload',
].join(' ');

interface ClaudeCredentialStore {
  accounts: Record<string, ClaudeCredential>;
}

interface ClaudeCredential {
  id: string;
  accessToken: string;
  refreshToken?: string;
  tokenType?: string;
  expiresAt?: number;
}

interface ClaudeAccount {
  id: string;
  email: string;
  authMode: 'oauth';
  accountUuid?: string;
  organizationUuid?: string;
  organizationName?: string;
  displayName?: string;
  avatarUrl?: string;
  planType?: string;
  quota: ClaudeQuotaSummary;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  usageUpdatedAt?: number | null;
  profileUpdatedAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

interface ClaudeQuotaSummary {
  fiveHourRemainingPercent?: number | null;
  fiveHourResetAt?: number | null;
  weeklyRemainingPercent?: number | null;
  weeklyResetAt?: number | null;
  weeklySonnetRemainingPercent?: number | null;
  weeklySonnetResetAt?: number | null;
  extraUsageRemainingPercent?: number | null;
  extraUsageResetAt?: number | null;
}

interface ClaudeTokenResponse {
  access_token?: string;
  refresh_token?: string;
  token_type?: string;
  expires_in?: number;
  scope?: string;
  error?: unknown;
  error_description?: string;
}

interface OAuthStart {
  authUrl: string;
  codeVerifier: string;
  state: string;
}

function now(): number {
  return Date.now();
}

function normalize(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}

function randomToken(): string {
  return crypto.randomBytes(32).toString('base64url');
}

function pkceChallenge(codeVerifier: string): string {
  return crypto.createHash('sha256').update(codeVerifier).digest('base64url');
}

function clampPercent(value: number | undefined): number | undefined {
  if (value == null || !Number.isFinite(value)) return undefined;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function usedFromRemaining(value: number | null | undefined): number | undefined {
  const remaining = clampPercent(value ?? undefined);
  return remaining == null ? undefined : 100 - remaining;
}

function numberValue(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string' && value.trim().length > 0) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function readPath(root: unknown, path: string[]): unknown {
  let current = root;
  for (const key of path) {
    if (current == null || typeof current !== 'object') return undefined;
    current = (current as Record<string, unknown>)[key];
  }
  return current;
}

function readString(root: unknown, path: string[]): string | undefined {
  return normalize(readPath(root, path));
}

function firstString(values: Array<string | undefined>): string | undefined {
  return values.find((value) => value != null);
}

function buildAccountId(email: string, accountUuid?: string, organizationUuid?: string): string {
  const seed = [
    email.trim().toLowerCase(),
    accountUuid?.trim() ?? '',
    organizationUuid?.trim() ?? '',
  ].join(':');
  return `claude_${crypto.createHash('md5').update(seed).digest('hex')}`;
}

function subscriptionType(profile: unknown): string | undefined {
  switch (readString(profile, ['organization', 'organization_type'])) {
    case 'claude_max':
      return 'Max';
    case 'claude_pro':
      return 'Pro';
    case 'claude_enterprise':
      return 'Enterprise';
    case 'claude_team':
      return 'Team';
    default:
      return undefined;
  }
}

function remainingPercentFromUsage(window: unknown): number | undefined {
  const used = numberValue(readPath(window, ['utilization']));
  if (used == null) return undefined;
  return Math.min(100, Math.max(0, Math.round(100 - used)));
}

function resetAtFromUsage(window: unknown): number | undefined {
  const value = readPath(window, ['resets_at']);
  const numeric = numberValue(value);
  if (numeric != null) {
    if (numeric <= 0) return undefined;
    return numeric > 10_000_000_000 ? numeric : numeric * 1000;
  }

  if (typeof value === 'string' && value.trim().length > 0) {
    const parsed = Date.parse(value);
    return Number.isNaN(parsed) ? undefined : parsed;
  }

  return undefined;
}

function quotaFromUsage(raw: unknown): ClaudeQuotaSummary {
  const fiveHour = readPath(raw, ['five_hour']);
  const weekly = readPath(raw, ['seven_day']);
  const weeklySonnet = readPath(raw, ['seven_day_sonnet'])
    ?? readPath(raw, ['seven_day_sonnet_4'])
    ?? readPath(raw, ['seven_day_model']);
  const extraUsage = readPath(raw, ['extra_usage']);
  const extraEnabled = readPath(extraUsage, ['is_enabled']) === true;

  return {
    fiveHourRemainingPercent: remainingPercentFromUsage(fiveHour),
    fiveHourResetAt: resetAtFromUsage(fiveHour),
    weeklyRemainingPercent: remainingPercentFromUsage(weekly),
    weeklyResetAt: resetAtFromUsage(weekly),
    weeklySonnetRemainingPercent: remainingPercentFromUsage(weeklySonnet),
    weeklySonnetResetAt: resetAtFromUsage(weeklySonnet),
    extraUsageRemainingPercent: extraEnabled ? remainingPercentFromUsage(extraUsage) : undefined,
    extraUsageResetAt: extraEnabled ? resetAtFromUsage(extraUsage) : undefined,
  };
}

function parseCallbackInput(input: string, expectedState: string): string {
  const trimmed = input.trim();
  if (!trimmed) throw new Error('Claude callback URL or code is required.');

  const parseCode = (raw: string): string => {
    const [beforeHash, hashState] = raw.split('#', 2);
    if (hashState && hashState !== expectedState) throw new Error('Claude OAuth callback state did not match.');
    return beforeHash.split('&', 1)[0].trim();
  };

  const url = (() => {
    try {
      return new URL(trimmed);
    } catch {
      return undefined;
    }
  })();

  if (url) {
    const code = normalize(url.searchParams.get('code'));
    const state = normalize(url.searchParams.get('state'));
    if (state && state !== expectedState) throw new Error('Claude OAuth callback state did not match.');
    if (code) return parseCode(code);
  }

  if (trimmed.startsWith('?')) {
    const params = new URLSearchParams(trimmed.slice(1));
    const code = normalize(params.get('code'));
    const state = normalize(params.get('state'));
    if (state && state !== expectedState) throw new Error('Claude OAuth callback state did not match.');
    if (code) return parseCode(code);
  }

  return parseCode(trimmed.replace(/^code=/, ''));
}

function buildOAuthStart(): OAuthStart {
  const state = randomToken();
  const codeVerifier = randomToken();
  const params = new URLSearchParams({
    code: 'true',
    client_id: CLAUDE_OAUTH_CLIENT_ID,
    response_type: 'code',
    redirect_uri: CLAUDE_OAUTH_CALLBACK_URL,
    scope: CLAUDE_OAUTH_SCOPES,
    code_challenge: pkceChallenge(codeVerifier),
    code_challenge_method: 'S256',
    state,
  });

  return {
    authUrl: `${CLAUDE_OAUTH_AUTHORIZE_URL}?${params.toString()}`,
    codeVerifier,
    state,
  };
}

async function parseJsonResponse<T>(response: Response, failureLabel: string): Promise<T> {
  const body = await response.text();
  if (!response.ok) {
    throw new Error(`${failureLabel} returned ${response.status} with body length ${body.length}.`);
  }
  return JSON.parse(body) as T;
}

async function exchangeOAuthCode(start: OAuthStart, code: string): Promise<ClaudeTokenResponse> {
  const response = await fetch(CLAUDE_OAUTH_TOKEN_URL, {
    method: 'POST',
    headers: {
      Accept: 'application/json, text/plain, */*',
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      grant_type: 'authorization_code',
      client_id: CLAUDE_OAUTH_CLIENT_ID,
      code,
      redirect_uri: CLAUDE_OAUTH_CALLBACK_URL,
      code_verifier: start.codeVerifier,
      state: start.state,
    }),
  });

  return parseJsonResponse<ClaudeTokenResponse>(response, 'Claude OAuth token exchange');
}

async function refreshToken(refreshToken: string): Promise<ClaudeTokenResponse> {
  const response = await fetch(CLAUDE_OAUTH_TOKEN_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      grant_type: 'refresh_token',
      refresh_token: refreshToken,
      client_id: CLAUDE_OAUTH_CLIENT_ID,
    }),
  });

  return parseJsonResponse<ClaudeTokenResponse>(response, 'Claude token refresh');
}

async function requestProfile(accessToken: string): Promise<unknown> {
  const response = await fetch(CLAUDE_OAUTH_PROFILE_URL, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
  });
  return parseJsonResponse<unknown>(response, 'Claude OAuth profile');
}

async function requestUsage(accessToken: string): Promise<unknown> {
  const response = await fetch(CLAUDE_OAUTH_USAGE_URL, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'anthropic-beta': CLAUDE_OAUTH_BETA_HEADER,
    },
  });
  return parseJsonResponse<unknown>(response, 'Claude usage');
}

function accountFromTokenResponse(response: ClaudeTokenResponse, profile: unknown, existing?: ClaudeAccount): { account: ClaudeAccount; credential: ClaudeCredential } {
  if (response.error) {
    throw new Error(response.error_description ?? String(response.error));
  }

  const accessToken = normalize(response.access_token);
  if (!accessToken) throw new Error('Claude token response did not include an access token.');

  const accountUuid = firstString([
    readString(profile, ['account', 'uuid']),
    readString(response, ['account', 'uuid']),
    existing?.accountUuid,
  ]);
  const organizationUuid = firstString([
    readString(profile, ['organization', 'uuid']),
    readString(response, ['organization', 'uuid']),
    existing?.organizationUuid,
  ]);
  const email = firstString([
    readString(profile, ['account', 'email']),
    readString(profile, ['account', 'email_address']),
    readString(response, ['account', 'email_address']),
    existing?.email,
  ]);
  if (!email) throw new Error('Claude OAuth response did not include an email.');

  const id = buildAccountId(email, accountUuid, organizationUuid);
  const nowMs = now();
  const expiresIn = numberValue(response.expires_in);

  return {
    account: {
      id,
      email,
      authMode: 'oauth',
      accountUuid,
      organizationUuid,
      organizationName: firstString([
        readString(profile, ['organization', 'name']),
        readString(profile, ['organization', 'display_name']),
        readString(response, ['organization', 'name']),
      ]) ?? existing?.organizationName,
      displayName: readString(profile, ['account', 'display_name']) ?? existing?.displayName,
      avatarUrl: firstString([
        readString(profile, ['account', 'avatar_url']),
        readString(profile, ['account', 'avatarUrl']),
      ]) ?? existing?.avatarUrl,
      planType: subscriptionType(profile) ?? existing?.planType,
      quota: existing?.quota ?? {},
      quotaQueryLastError: existing?.quotaQueryLastError ?? null,
      quotaQueryLastErrorAt: existing?.quotaQueryLastErrorAt ?? null,
      usageUpdatedAt: existing?.usageUpdatedAt ?? null,
      profileUpdatedAt: nowMs,
      createdAt: existing?.createdAt ?? nowMs,
      lastUsed: nowMs,
    },
    credential: {
      id,
      accessToken,
      refreshToken: normalize(response.refresh_token),
      tokenType: normalize(response.token_type),
      expiresAt: expiresIn == null ? undefined : nowMs + expiresIn * 1000,
    },
  };
}

function trackFromAccount(account: ClaudeAccount, id: TrackId, label: string, remaining: number | null | undefined, resetAt: number | null | undefined): QuotaTrack {
  return {
    id,
    providerId: 'claude',
    providerLabel: PROVIDER_LABELS.claude,
    label,
    accountLabel: account.displayName ? `${account.displayName} (${account.email})` : account.email,
    percentUsed: usedFromRemaining(remaining),
    percentRemaining: remaining ?? undefined,
    resetAt,
    updatedAt: account.usageUpdatedAt,
    error: account.quotaQueryLastError ?? null,
  };
}

export class ClaudeProvider {
  constructor(private readonly context: vscode.ExtensionContext) {}

  async connect(): Promise<ClaudeAccount> {
    const start = buildOAuthStart();
    await vscode.env.openExternal(vscode.Uri.parse(start.authUrl));

    const callbackOrCode = await vscode.window.showInputBox({
      title: 'Connect Claude Code',
      prompt: 'Paste the Claude callback URL or authorization code after approving access in the browser.',
      ignoreFocusOut: true,
      password: true,
      placeHolder: 'https://platform.claude.com/oauth/code/callback?code=...',
    });
    if (!callbackOrCode) throw new Error('Claude OAuth login was cancelled.');

    const code = parseCallbackInput(callbackOrCode, start.state);
    const tokenResponse = await exchangeOAuthCode(start, code);
    const accessToken = normalize(tokenResponse.access_token);
    if (!accessToken) throw new Error('Claude token response did not include an access token.');
    const profile = await requestProfile(accessToken);
    const account = await this.upsertTokenResponse(tokenResponse, profile);
    await this.refreshAccount(account.id);
    return (await this.getAccount(account.id)) ?? account;
  }

  async refreshAll(): Promise<ClaudeAccount[]> {
    const accounts = await this.getAccounts();
    const refreshed: ClaudeAccount[] = [];
    for (const account of accounts) {
      refreshed.push(await this.refreshAccount(account.id));
    }
    return refreshed;
  }

  async refreshAccount(accountId: string): Promise<ClaudeAccount> {
    const account = await this.getAccount(accountId);
    const credential = await this.getCredential(accountId);
    if (!account || !credential) throw new Error('Claude account is not connected.');

    try {
      const validCredential = await this.ensureAccessToken(account, credential);
      const usage = await requestUsage(validCredential.accessToken);
      return await this.saveAccount({
        ...account,
        quota: quotaFromUsage(usage),
        quotaQueryLastError: null,
        quotaQueryLastErrorAt: null,
        usageUpdatedAt: now(),
        lastUsed: now(),
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return await this.saveAccount({
        ...account,
        quotaQueryLastError: message,
        quotaQueryLastErrorAt: now(),
        lastUsed: now(),
      });
    }
  }

  async disconnect(): Promise<void> {
    const accounts = await this.getAccounts();
    if (accounts.length === 0) {
      void vscode.window.showInformationMessage('No Claude Code accounts are connected.');
      return;
    }

    const picked = await vscode.window.showQuickPick(
      accounts.map((account) => ({
        label: account.email,
        description: account.planType ?? account.organizationName ?? 'Claude Code account',
        account,
      })),
      { title: 'Disconnect Claude Code' },
    );

    if (!picked) return;

    const confirmed = await vscode.window.showWarningMessage(
      `Disconnect Claude Code account ${picked.account.email}? Extension-stored tokens and cached quota data will be deleted.`,
      { modal: true },
      'Disconnect',
    );
    if (confirmed !== 'Disconnect') return;

    const store = await this.getCredentialStore();
    delete store.accounts[picked.account.id];
    await this.saveCredentialStore(store);
    await this.context.globalState.update(CLAUDE_ACCOUNTS_KEY, accounts.filter((account) => account.id !== picked.account.id));
  }

  async getTracks(): Promise<QuotaTrack[]> {
    const accounts = await this.getAccounts();
    return accounts.flatMap((account) => {
      const tracks = [
        trackFromAccount(account, 'claude.fiveHour', '5h usage', account.quota.fiveHourRemainingPercent, account.quota.fiveHourResetAt),
        trackFromAccount(account, 'claude.weekly', 'Weekly usage', account.quota.weeklyRemainingPercent, account.quota.weeklyResetAt),
      ];
      if (account.quota.weeklySonnetRemainingPercent != null || account.quota.weeklySonnetResetAt != null) {
        tracks.push(trackFromAccount(account, 'claude.weeklySonnet', 'Weekly Sonnet', account.quota.weeklySonnetRemainingPercent, account.quota.weeklySonnetResetAt));
      }
      if (account.quota.extraUsageRemainingPercent != null || account.quota.extraUsageResetAt != null) {
        tracks.push(trackFromAccount(account, 'claude.extraUsage', 'Extra usage', account.quota.extraUsageRemainingPercent, account.quota.extraUsageResetAt));
      }
      return tracks;
    }).filter((track) => track.percentUsed != null || track.percentRemaining != null || track.error);
  }

  async hasAccounts(): Promise<boolean> {
    return (await this.getAccounts()).length > 0;
  }

  private async ensureAccessToken(account: ClaudeAccount, credential: ClaudeCredential): Promise<ClaudeCredential> {
    const shouldRefresh = credential.expiresAt != null && credential.expiresAt <= now() + 300_000;
    if (!shouldRefresh) return credential;
    if (!credential.refreshToken) throw new Error('Claude refresh token is missing.');

    const tokenResponse = await refreshToken(credential.refreshToken);
    const updated = await this.upsertTokenResponse(tokenResponse, undefined, account);
    const nextCredential = await this.getCredential(updated.id);
    if (!nextCredential) throw new Error('Claude token refresh did not persist credentials.');
    return nextCredential;
  }

  private async upsertTokenResponse(response: ClaudeTokenResponse, profile?: unknown, existing?: ClaudeAccount): Promise<ClaudeAccount> {
    const result = accountFromTokenResponse(response, profile, existing);
    const store = await this.getCredentialStore();
    store.accounts[result.account.id] = result.credential;
    if (!result.credential.refreshToken && existing) {
      const oldCredential = await this.getCredential(existing.id);
      if (oldCredential?.refreshToken) store.accounts[result.account.id].refreshToken = oldCredential.refreshToken;
    }
    await this.saveCredentialStore(store);
    return this.saveAccount(result.account);
  }

  private async getAccounts(): Promise<ClaudeAccount[]> {
    return this.context.globalState.get<ClaudeAccount[]>(CLAUDE_ACCOUNTS_KEY, []);
  }

  private async getAccount(accountId: string): Promise<ClaudeAccount | undefined> {
    return (await this.getAccounts()).find((account) => account.id === accountId);
  }

  private async saveAccount(account: ClaudeAccount): Promise<ClaudeAccount> {
    const accounts = await this.getAccounts();
    const next = [account, ...accounts.filter((item) => item.id !== account.id)];
    await this.context.globalState.update(CLAUDE_ACCOUNTS_KEY, next);
    return account;
  }

  private async getCredential(accountId: string): Promise<ClaudeCredential | undefined> {
    const store = await this.getCredentialStore();
    return store.accounts[accountId];
  }

  private async getCredentialStore(): Promise<ClaudeCredentialStore> {
    const raw = await this.context.secrets.get(CLAUDE_SECRET_KEY);
    if (!raw) return { accounts: {} };

    try {
      const parsed = JSON.parse(raw) as ClaudeCredentialStore;
      return parsed && typeof parsed === 'object' && parsed.accounts ? parsed : { accounts: {} };
    } catch {
      return { accounts: {} };
    }
  }

  private async saveCredentialStore(store: ClaudeCredentialStore): Promise<void> {
    await this.context.secrets.store(CLAUDE_SECRET_KEY, JSON.stringify(store));
  }
}
