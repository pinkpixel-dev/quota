// @env node
import * as crypto from 'node:crypto';
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';

import * as vscode from 'vscode';

import { reauthenticationMessage, tokenRefreshErrorMessage } from './authError';
import { PROVIDER_LABELS } from './constants';
import { buildGrokUsageSummary, type GrokUsageSummary } from './grokUsage';
import type { QuotaTrack } from './types';

const GROK_BILLING_ENDPOINT = 'https://cli-chat-proxy.grok.com/v1/billing';
const GROK_USER_ENDPOINT = 'https://cli-chat-proxy.grok.com/v1/user';
const GROK_SUBSCRIPTIONS_ENDPOINT = 'https://grok.com/rest/subscriptions';
const GROK_OAUTH_ISSUER = 'https://auth.x.ai';
const GROK_OAUTH_DEVICE_ENDPOINT = 'https://auth.x.ai/oauth2/device/code';
const GROK_OAUTH_TOKEN_ENDPOINT = 'https://auth.x.ai/oauth2/token';
const GROK_OAUTH_CLIENT_ID = 'b1a00492-073a-47ea-816f-4c329264a828';
const GROK_OAUTH_SCOPES = 'openid profile email offline_access grok-cli:access api:access conversations:read conversations:write';
const GROK_OAUTH_CLIENT_SURFACE = 'grok-build';
const GROK_SECRET_KEY = 'quota.grok.credentials';
const GROK_ACCOUNTS_KEY = 'quota.grok.accounts';

interface GrokCredentialStore {
  accounts: Record<string, GrokCredential>;
}

interface GrokCredential {
  id: string;
  accessToken: string;
  refreshToken?: string;
  expiresAt?: number;
  oidcIssuer?: string;
  oidcClientId?: string;
}

interface GrokAccount {
  id: string;
  email: string;
  displayName?: string;
  userId?: string;
  principalId?: string;
  teamId?: string;
  teamName?: string;
  organizationId?: string;
  organizationName?: string;
  plan?: string;
  tier?: number;
  hasGrokCodeAccess?: boolean;
  usage: GrokUsageSummary;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  usageUpdatedAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

interface GrokAuthFileEntry {
  key?: string;
  refresh_token?: string;
  expires_at?: string;
  email?: string;
  first_name?: string;
  user_id?: string;
  principal_id?: string;
  team_id?: string;
  oidc_issuer?: string;
  oidc_client_id?: string;
}

interface DeviceCodeResponse {
  device_code?: string;
  user_code?: string;
  verification_uri?: string;
  verification_uri_complete?: string;
  expires_in?: number;
  interval?: number;
  error?: string;
  error_description?: string;
}

interface TokenResponse {
  access_token?: string;
  refresh_token?: string;
  id_token?: string;
  expires_in?: number;
  error?: string;
  error_description?: string;
}

interface GrokIdentity {
  userId?: string;
  email?: string;
  displayName?: string;
  principalId?: string;
  teamId?: string;
  teamName?: string;
  organizationId?: string;
  organizationName?: string;
  hasGrokCodeAccess?: boolean;
}

function now(): number {
  return Date.now();
}

function normalize(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function clampPercent(value: number | undefined): number | undefined {
  if (value == null || !Number.isFinite(value)) return undefined;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function grokHome(): string {
  return normalize(process.env.GROK_HOME) ?? path.join(os.homedir(), '.grok');
}

function decodeJwtPayload(token: string): Record<string, unknown> | undefined {
  const payload = token.split('.')[1];
  if (!payload) return undefined;
  try {
    const claims = JSON.parse(Buffer.from(payload, 'base64url').toString('utf8')) as unknown;
    return isRecord(claims) ? claims : undefined;
  } catch {
    return undefined;
  }
}

function buildAccountId(email: string, principalId: string | undefined): string {
  const seed = principalId ? `${email.trim().toLowerCase()}|${principalId}` : email.trim().toLowerCase();
  return `grok_${crypto.createHash('md5').update(seed).digest('hex')}`;
}

function splitIssuerKey(issuerKey: string): { issuer?: string; clientId?: string } {
  const separator = issuerKey.indexOf('::');
  if (separator < 0) return { issuer: normalize(issuerKey) };
  return {
    issuer: normalize(issuerKey.slice(0, separator)),
    clientId: normalize(issuerKey.slice(separator + 2)),
  };
}

function parseExpiry(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value) && value > 0) {
    return value > 10_000_000_000 ? Math.trunc(value) : Math.trunc(value * 1000);
  }
  if (typeof value === 'string' && value.trim()) {
    const parsed = Date.parse(value.trim());
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function authHeaders(accessToken: string): Record<string, string> {
  // Grok rejects any X-XAI-Token-Auth value with 401; bearer auth only.
  return {
    Accept: 'application/json',
    Authorization: `Bearer ${accessToken.trim()}`,
  };
}

async function parseJsonResponse<T>(response: Response, failureLabel: string): Promise<T> {
  const body = await response.text();
  if (!response.ok) {
    if (response.status === 401) {
      throw new Error(`unauthorized:${failureLabel} returned 401.`);
    }
    if (response.status === 403) {
      // A valid token without the grok-cli/api scopes. Refreshing reissues the same
      // scopes, so the account has to be connected again.
      throw new Error(`forbidden:${failureLabel} returned 403.`);
    }
    throw new Error(`${failureLabel} returned ${response.status} with body length ${body.length}.`);
  }
  return JSON.parse(body) as T;
}

async function requestDeviceCode(): Promise<DeviceCodeResponse> {
  const response = await fetch(GROK_OAUTH_DEVICE_ENDPOINT, {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/x-www-form-urlencoded',
      'x-grok-client-surface': GROK_OAUTH_CLIENT_SURFACE,
    },
    body: new URLSearchParams({
      client_id: GROK_OAUTH_CLIENT_ID,
      scope: GROK_OAUTH_SCOPES,
    }).toString(),
  });

  const device = await parseJsonResponse<DeviceCodeResponse>(response, 'Grok device code request');
  if (device.error) {
    throw new Error(device.error_description ?? `Grok device authorization failed: ${device.error}`);
  }
  if (!normalize(device.device_code)) throw new Error('Grok device code response did not include a device code.');
  return device;
}

async function exchangeDeviceToken(deviceCode: string): Promise<TokenResponse> {
  const response = await fetch(GROK_OAUTH_TOKEN_ENDPOINT, {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/x-www-form-urlencoded',
      'x-grok-client-surface': GROK_OAUTH_CLIENT_SURFACE,
    },
    body: new URLSearchParams({
      grant_type: 'urn:ietf:params:oauth:grant-type:device_code',
      client_id: GROK_OAUTH_CLIENT_ID,
      device_code: deviceCode,
    }).toString(),
  });

  const body = await response.text();
  try {
    return JSON.parse(body) as TokenResponse;
  } catch {
    throw new Error(`Grok token request returned ${response.status} with body length ${body.length}.`);
  }
}

async function pollDeviceToken(
  deviceCode: string,
  intervalSeconds: number,
  expiresInSeconds: number,
  token: vscode.CancellationToken,
): Promise<TokenResponse> {
  const expiresAt = now() + expiresInSeconds * 1000;
  let waitSeconds = Math.max(1, intervalSeconds);

  while (now() < expiresAt) {
    if (token.isCancellationRequested) throw new Error('Grok authorization was cancelled.');

    const response = await exchangeDeviceToken(deviceCode);
    if (!response.error) {
      if (!normalize(response.access_token)) throw new Error('Grok token response did not include an access token.');
      return response;
    }

    if (response.error === 'authorization_pending') {
      await sleep(waitSeconds * 1000);
      continue;
    }
    if (response.error === 'slow_down') {
      waitSeconds += 5;
      await sleep(waitSeconds * 1000);
      continue;
    }
    if (response.error === 'expired_token') throw new Error('Grok authorization expired. Start again.');
    if (response.error === 'access_denied') throw new Error('Grok authorization was denied.');
    throw new Error(response.error_description ?? `Grok authorization failed: ${response.error}`);
  }

  throw new Error('Grok authorization expired. Start again.');
}

async function refreshAccessToken(refreshToken: string): Promise<TokenResponse> {
  const response = await fetch(GROK_OAUTH_TOKEN_ENDPOINT, {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/x-www-form-urlencoded',
      'x-grok-client-surface': GROK_OAUTH_CLIENT_SURFACE,
    },
    body: new URLSearchParams({
      grant_type: 'refresh_token',
      client_id: GROK_OAUTH_CLIENT_ID,
      refresh_token: refreshToken,
    }).toString(),
  });

  const body = await response.text();
  if (!response.ok) throw new Error(tokenRefreshErrorMessage('Grok', response.status, body));

  const parsed = JSON.parse(body) as TokenResponse;
  if (parsed.error) throw new Error(tokenRefreshErrorMessage('Grok', response.status, body));
  if (!normalize(parsed.access_token)) throw new Error('Grok refresh response did not include an access token.');
  return parsed;
}

async function fetchBilling(accessToken: string, format?: string): Promise<unknown> {
  const url = format ? `${GROK_BILLING_ENDPOINT}?format=${encodeURIComponent(format)}` : GROK_BILLING_ENDPOINT;
  const response = await fetch(url, { headers: authHeaders(accessToken) });
  return parseJsonResponse<unknown>(response, 'Grok billing request');
}

async function fetchIdentity(accessToken: string): Promise<GrokIdentity> {
  const response = await fetch(GROK_USER_ENDPOINT, { headers: authHeaders(accessToken) });
  const raw = await parseJsonResponse<Record<string, unknown>>(response, 'Grok user request');

  return {
    userId: normalize(raw.userId),
    email: normalize(raw.email),
    displayName: normalize(raw.firstName),
    principalId: normalize(raw.principalId),
    teamId: normalize(raw.teamId),
    teamName: normalize(raw.teamName),
    organizationId: normalize(raw.organizationId),
    organizationName: normalize(raw.organizationName),
    hasGrokCodeAccess: typeof raw.hasGrokCodeAccess === 'boolean' ? raw.hasGrokCodeAccess : undefined,
  };
}

async function fetchSubscriptionTier(accessToken: string): Promise<string | undefined> {
  try {
    const response = await fetch(GROK_SUBSCRIPTIONS_ENDPOINT, { headers: authHeaders(accessToken) });
    if (!response.ok) return undefined;
    const raw = (await response.json()) as unknown;
    const list = isRecord(raw) && Array.isArray(raw.subscriptions) ? raw.subscriptions : [];
    const active = list.find((item) => isRecord(item) && normalize(item.tier) != null);
    return isRecord(active) ? normalize(active.tier) : undefined;
  } catch {
    return undefined;
  }
}

function tracksFromAccount(account: GrokAccount): QuotaTrack[] {
  const usage = account.usage;
  const accountLabel = account.email || account.displayName || 'Grok account';
  const base = {
    providerId: 'grok' as const,
    providerLabel: PROVIDER_LABELS.grok,
    accountLabel,
    updatedAt: account.usageUpdatedAt,
    error: account.quotaQueryLastError ?? null,
  };

  const monthlyPercent =
    usage.monthlyLimit != null && usage.monthlyLimit > 0 && usage.monthlyUsed != null
      ? (usage.monthlyUsed / usage.monthlyLimit) * 100
      : undefined;
  const onDemandPercent =
    usage.onDemandCap != null && usage.onDemandCap > 0 && usage.onDemandUsed != null
      ? (usage.onDemandUsed / usage.onDemandCap) * 100
      : undefined;

  return [
    {
      ...base,
      id: 'grok.credits',
      label: usage.periodLabel ? `${usage.periodLabel} credits` : 'Credit window',
      percentUsed: clampPercent(usage.creditUsedPercent),
      percentRemaining: clampPercent(usage.creditRemainingPercent),
      resetAt: usage.periodResetAt ?? null,
    },
    {
      ...base,
      id: 'grok.monthlySpend',
      label: 'Monthly spend',
      percentUsed: clampPercent(monthlyPercent),
      percentRemaining: monthlyPercent == null ? undefined : 100 - clampPercent(monthlyPercent)!,
      resetAt: usage.monthlyPeriodEndAt ?? null,
    },
    {
      ...base,
      id: 'grok.onDemand',
      label: 'On-demand spend',
      percentUsed: clampPercent(onDemandPercent),
      percentRemaining: onDemandPercent == null ? undefined : 100 - clampPercent(onDemandPercent)!,
      resetAt: usage.monthlyPeriodEndAt ?? null,
    },
  ];
}

export class GrokProvider {
  constructor(private readonly context: vscode.ExtensionContext) {}

  async connect(): Promise<GrokAccount> {
    const device = await requestDeviceCode();
    const authUri = normalize(device.verification_uri_complete)
      ?? normalize(device.verification_uri)
      ?? 'https://accounts.x.ai/oauth2/device';
    const userCode = normalize(device.user_code) ?? '';

    await vscode.env.openExternal(vscode.Uri.parse(authUri));
    if (userCode) await vscode.env.clipboard.writeText(userCode);

    const tokenResponse = await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: userCode
          ? `Grok device code ${userCode} (copied). Confirm it in the browser.`
          : 'Waiting for Grok authorization in the browser.',
        cancellable: true,
      },
      (_progress, cancellationToken) =>
        pollDeviceToken(
          device.device_code!,
          device.interval ?? 5,
          device.expires_in ?? 1800,
          cancellationToken,
        ),
    );

    return this.upsertTokenResponse(tokenResponse);
  }

  async importLocal(): Promise<GrokAccount[]> {
    const authPath = path.join(grokHome(), 'auth.json');
    let raw: string;
    try {
      raw = await fs.readFile(authPath, 'utf8');
    } catch {
      throw new Error(`Could not read the Grok auth file: ${authPath}`);
    }

    const parsed = JSON.parse(raw) as unknown;
    if (!isRecord(parsed)) throw new Error('The Grok auth file is not a JSON object.');

    const imported: GrokAccount[] = [];
    let lastError: string | undefined;

    for (const [issuerKey, value] of Object.entries(parsed)) {
      if (!isRecord(value)) continue;
      const entry = value as GrokAuthFileEntry;
      const accessToken = normalize(entry.key);
      if (!accessToken) {
        lastError = 'A Grok auth entry does not include an access token.';
        continue;
      }

      const claims = decodeJwtPayload(accessToken);
      const email = normalize(entry.email) ?? normalize(claims?.email);
      if (!email) {
        lastError = 'A Grok auth entry does not include an email.';
        continue;
      }

      const principalId = normalize(entry.principal_id)
        ?? normalize(claims?.principal_id)
        ?? normalize(claims?.sub);
      const issuerInfo = splitIssuerKey(issuerKey);
      const credential: GrokCredential = {
        id: buildAccountId(email, principalId),
        accessToken,
        refreshToken: normalize(entry.refresh_token),
        expiresAt: parseExpiry(entry.expires_at) ?? parseExpiry(claims?.exp),
        oidcIssuer: normalize(entry.oidc_issuer) ?? issuerInfo.issuer ?? GROK_OAUTH_ISSUER,
        oidcClientId: normalize(entry.oidc_client_id) ?? issuerInfo.clientId ?? GROK_OAUTH_CLIENT_ID,
      };

      const existing = await this.getAccount(credential.id);
      const account: GrokAccount = {
        id: credential.id,
        email,
        displayName: normalize(entry.first_name) ?? existing?.displayName,
        userId: normalize(entry.user_id) ?? principalId,
        principalId,
        teamId: normalize(entry.team_id) ?? normalize(claims?.team_id),
        teamName: existing?.teamName,
        organizationId: existing?.organizationId,
        organizationName: existing?.organizationName,
        plan: existing?.plan,
        tier: typeof claims?.tier === 'number' ? claims.tier : existing?.tier,
        hasGrokCodeAccess: existing?.hasGrokCodeAccess,
        usage: existing?.usage ?? { productUsage: [] },
        quotaQueryLastError: null,
        quotaQueryLastErrorAt: null,
        usageUpdatedAt: existing?.usageUpdatedAt ?? null,
        createdAt: existing?.createdAt ?? now(),
        lastUsed: now(),
      };

      await this.saveCredential(credential);
      imported.push(await this.saveAccount(account));
    }

    if (imported.length === 0) {
      throw new Error(lastError ?? 'The Grok auth file has no importable credentials.');
    }

    for (const account of imported) await this.refreshAccount(account.id);
    return this.getAccounts();
  }

  async refreshAll(): Promise<GrokAccount[]> {
    const accounts = await this.getAccounts();
    const refreshed: GrokAccount[] = [];
    for (const account of accounts) refreshed.push(await this.refreshAccount(account.id));
    return refreshed;
  }

  async refreshAccount(accountId: string): Promise<GrokAccount> {
    const account = await this.getAccount(accountId);
    const credential = await this.getCredential(accountId);
    if (!account || !credential) throw new Error('Grok account is not connected.');

    try {
      return await this.applyUsage(account, credential.accessToken);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message.startsWith('forbidden:')) {
        return this.saveAccount({
          ...account,
          quotaQueryLastError: reauthenticationMessage('Grok'),
          quotaQueryLastErrorAt: now(),
        });
      }
      if (!message.startsWith('unauthorized:')) {
        return this.saveAccount({ ...account, quotaQueryLastError: message, quotaQueryLastErrorAt: now() });
      }

      if (!credential.refreshToken) {
        return this.saveAccount({
          ...account,
          quotaQueryLastError: reauthenticationMessage('Grok'),
          quotaQueryLastErrorAt: now(),
        });
      }

      try {
        const tokenResponse = await refreshAccessToken(credential.refreshToken);
        const nextCredential: GrokCredential = {
          ...credential,
          accessToken: normalize(tokenResponse.access_token)!,
          refreshToken: normalize(tokenResponse.refresh_token) ?? credential.refreshToken,
          expiresAt: tokenResponse.expires_in != null ? now() + tokenResponse.expires_in * 1000 : credential.expiresAt,
        };
        await this.saveCredential(nextCredential);
        return await this.applyUsage(account, nextCredential.accessToken);
      } catch (refreshError) {
        const refreshMessage = refreshError instanceof Error ? refreshError.message : String(refreshError);
        const stillRejected = refreshMessage.startsWith('unauthorized:') || refreshMessage.startsWith('forbidden:');
        return this.saveAccount({
          ...account,
          quotaQueryLastError: stillRejected ? reauthenticationMessage('Grok') : refreshMessage,
          quotaQueryLastErrorAt: now(),
        });
      }
    }
  }

  async disconnect(): Promise<void> {
    const accounts = await this.getAccounts();
    if (accounts.length === 0) {
      void vscode.window.showInformationMessage('No Grok accounts are connected.');
      return;
    }

    const picked = await vscode.window.showQuickPick(
      accounts.map((account) => ({
        label: account.email || 'Grok account',
        description: account.plan ?? account.teamName ?? 'Grok account',
        account,
      })),
      { title: 'Disconnect Grok' },
    );
    if (!picked) return;

    const confirmed = await vscode.window.showWarningMessage(
      `Disconnect Grok account ${picked.account.email || 'Grok account'}? Extension-stored tokens and cached quota data will be deleted.`,
      { modal: true },
      'Disconnect',
    );
    if (confirmed !== 'Disconnect') return;

    const store = await this.getCredentialStore();
    delete store.accounts[picked.account.id];
    await this.saveCredentialStore(store);
    await this.context.globalState.update(
      GROK_ACCOUNTS_KEY,
      accounts.filter((account) => account.id !== picked.account.id),
    );
  }

  async getTracks(): Promise<QuotaTrack[]> {
    const accounts = await this.getAccounts();
    return accounts
      .flatMap(tracksFromAccount)
      .filter((track) => track.percentUsed != null || track.percentRemaining != null || track.error);
  }

  async hasAccounts(): Promise<boolean> {
    return (await this.getAccounts()).length > 0;
  }

  private async applyUsage(account: GrokAccount, accessToken: string): Promise<GrokAccount> {
    const [credits, history] = await Promise.all([
      fetchBilling(accessToken, 'credits'),
      fetchBilling(accessToken, 'history').catch(() => undefined),
    ]);
    const usage = buildGrokUsageSummary(credits, history);
    const identity = await fetchIdentity(accessToken).catch((): GrokIdentity => ({}));
    const tier = usage.plan ?? (await fetchSubscriptionTier(accessToken));

    return this.saveAccount({
      ...account,
      email: identity.email ?? account.email,
      displayName: identity.displayName ?? account.displayName,
      userId: identity.userId ?? account.userId,
      principalId: identity.principalId ?? account.principalId,
      teamId: identity.teamId ?? account.teamId,
      teamName: identity.teamName ?? account.teamName,
      organizationId: identity.organizationId ?? account.organizationId,
      organizationName: identity.organizationName ?? account.organizationName,
      hasGrokCodeAccess: identity.hasGrokCodeAccess ?? account.hasGrokCodeAccess,
      plan: tier ?? account.plan,
      usage,
      quotaQueryLastError: null,
      quotaQueryLastErrorAt: null,
      usageUpdatedAt: now(),
      lastUsed: now(),
    });
  }

  private async upsertTokenResponse(tokenResponse: TokenResponse, existing?: GrokAccount): Promise<GrokAccount> {
    const accessToken = normalize(tokenResponse.access_token);
    if (!accessToken) throw new Error('Grok token response did not include an access token.');

    const claims = decodeJwtPayload(normalize(tokenResponse.id_token) ?? accessToken);
    const identity = await fetchIdentity(accessToken).catch((): GrokIdentity => ({}));
    const email = identity.email ?? normalize(claims?.email);
    if (!email) throw new Error('Grok authorization did not return an account email.');

    const principalId = identity.principalId ?? normalize(claims?.principal_id) ?? normalize(claims?.sub);
    const id = existing?.id ?? buildAccountId(email, principalId);

    await this.saveCredential({
      id,
      accessToken,
      refreshToken: normalize(tokenResponse.refresh_token),
      expiresAt: tokenResponse.expires_in != null ? now() + tokenResponse.expires_in * 1000 : parseExpiry(claims?.exp),
      oidcIssuer: GROK_OAUTH_ISSUER,
      oidcClientId: GROK_OAUTH_CLIENT_ID,
    });

    const account = await this.saveAccount({
      id,
      email,
      displayName: identity.displayName ?? existing?.displayName,
      userId: identity.userId ?? principalId,
      principalId,
      teamId: identity.teamId ?? normalize(claims?.team_id),
      teamName: identity.teamName,
      organizationId: identity.organizationId,
      organizationName: identity.organizationName,
      plan: existing?.plan,
      tier: typeof claims?.tier === 'number' ? claims.tier : existing?.tier,
      hasGrokCodeAccess: identity.hasGrokCodeAccess,
      usage: existing?.usage ?? { productUsage: [] },
      quotaQueryLastError: null,
      quotaQueryLastErrorAt: null,
      usageUpdatedAt: existing?.usageUpdatedAt ?? null,
      createdAt: existing?.createdAt ?? now(),
      lastUsed: now(),
    });

    return this.refreshAccount(account.id);
  }

  private async getAccounts(): Promise<GrokAccount[]> {
    return this.context.globalState.get<GrokAccount[]>(GROK_ACCOUNTS_KEY, []);
  }

  private async getAccount(accountId: string): Promise<GrokAccount | undefined> {
    return (await this.getAccounts()).find((account) => account.id === accountId);
  }

  private async saveAccount(account: GrokAccount): Promise<GrokAccount> {
    const accounts = await this.getAccounts();
    const next = [account, ...accounts.filter((item) => item.id !== account.id)];
    await this.context.globalState.update(GROK_ACCOUNTS_KEY, next);
    return account;
  }

  private async getCredential(accountId: string): Promise<GrokCredential | undefined> {
    const store = await this.getCredentialStore();
    return store.accounts[accountId];
  }

  private async saveCredential(credential: GrokCredential): Promise<void> {
    const store = await this.getCredentialStore();
    store.accounts[credential.id] = credential;
    await this.saveCredentialStore(store);
  }

  private async getCredentialStore(): Promise<GrokCredentialStore> {
    const raw = await this.context.secrets.get(GROK_SECRET_KEY);
    if (!raw) return { accounts: {} };

    try {
      const parsed = JSON.parse(raw) as GrokCredentialStore;
      return parsed && typeof parsed === 'object' && parsed.accounts ? parsed : { accounts: {} };
    } catch {
      return { accounts: {} };
    }
  }

  private async saveCredentialStore(store: GrokCredentialStore): Promise<void> {
    await this.context.secrets.store(GROK_SECRET_KEY, JSON.stringify(store));
  }
}
