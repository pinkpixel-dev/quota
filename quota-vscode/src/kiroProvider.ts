// @env node
import * as crypto from 'node:crypto';
import * as http from 'node:http';

import * as vscode from 'vscode';

import { PROVIDER_LABELS } from './constants';
import { buildKiroUsageSummary, normalizeKiroPlan, type KiroUsageSummary } from './kiroUsage';
import type { QuotaTrack } from './types';

const KIRO_AUTH_PORTAL_URL = 'https://app.kiro.dev/signin';
const KIRO_TOKEN_ENDPOINT = 'https://prod.us-east-1.auth.desktop.kiro.dev/oauth/token';
const KIRO_REFRESH_ENDPOINT = 'https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken';
const KIRO_RUNTIME_DEFAULT_ENDPOINT = 'https://q.us-east-1.amazonaws.com';
const KIRO_SECRET_KEY = 'quota.kiro.credentials';
const KIRO_ACCOUNTS_KEY = 'quota.kiro.accounts';
const KIRO_OAUTH_TIMEOUT_MS = 600_000;
const CALLBACK_PORT_CANDIDATES = [3128, 4649, 6588, 8008, 9091, 49153, 50153, 51153, 52153, 53153];

interface KiroCredentialStore {
  accounts: Record<string, KiroCredential>;
}

interface KiroCredential {
  id: string;
  accessToken: string;
  refreshToken?: string;
  rawToken: unknown;
  profileArn?: string;
  idcRegion?: string;
}

interface KiroAccount {
  id: string;
  email: string;
  loginProvider?: string;
  planName?: string;
  usage: KiroUsageSummary;
  profileArn?: string;
  idcRegion?: string;
  status?: string;
  statusReason?: string;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  usageUpdatedAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

interface KiroCallbackData {
  code: string;
  path: string;
  loginOption: string;
  issuerUrl?: string;
  idcRegion?: string;
}

interface KiroOAuthSession {
  server: http.Server;
  authUrl: string;
  callbackUrl: string;
  codeVerifier: string;
  callback: Promise<KiroCallbackData>;
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function getPath(root: unknown, path: string[]): unknown {
  let current = root;
  for (const key of path) {
    if (!isRecord(current)) return undefined;
    current = current[key];
  }
  return current;
}

function pickString(root: unknown, paths: string[][]): string | undefined {
  for (const path of paths) {
    const value = getPath(root, path);
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return undefined;
}

function normalizeTimestamp(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value) && value > 0) {
    return value > 10_000_000_000 ? Math.trunc(value) : Math.trunc(value * 1000);
  }
  return undefined;
}

function providerFromLoginOption(loginOption: string): string | undefined {
  const value = loginOption.trim().toLowerCase();
  if (!value) return undefined;
  if (value === 'google') return 'Google';
  if (value === 'github') return 'GitHub';
  return loginOption;
}

function parseProfileArnRegion(arn: string | undefined): string | undefined {
  const parts = arn?.split(':');
  const region = parts && parts[0]?.toLowerCase() === 'arn' ? parts[3] : undefined;
  return region?.trim() || undefined;
}

function runtimeEndpointForRegion(region: string | undefined): string {
  const normalized = region?.trim().toLowerCase() ?? 'us-east-1';
  if (normalized === 'eu-central-1') return 'https://q.eu-central-1.amazonaws.com';
  return KIRO_RUNTIME_DEFAULT_ENDPOINT;
}

function extractProfileArn(token: unknown): string | undefined {
  return pickString(token, [['profileArn'], ['profile_arn'], ['arn']]);
}

function decodeJwtEmail(token: string): string | undefined {
  const payload = token.split('.')[1];
  if (!payload) return undefined;
  try {
    const claims = JSON.parse(Buffer.from(payload, 'base64url').toString('utf8')) as unknown;
    const email = pickString(claims, [['email'], ['upn'], ['preferred_username']]);
    return email?.includes('@') ? email : undefined;
  } catch {
    return undefined;
  }
}

function buildAccountId(email: string, profileArn: string | undefined, accessToken: string): string {
  const identityEmail = email.includes('@') ? email : `__tok__${crypto.createHash('md5').update(accessToken).digest('hex')}`;
  const seed = `${identityEmail.trim().toLowerCase()}:${profileArn?.trim() ?? ''}`;
  return `kiro_${crypto.createHash('md5').update(seed).digest('hex')}`;
}

function clampPercent(value: number | undefined): number | undefined {
  if (value == null || !Number.isFinite(value)) return undefined;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function trackFromAccount(account: KiroAccount): QuotaTrack {
  const used = account.usage.creditsUsed;
  const total = account.usage.creditsTotal;
  const percentUsed = used != null && total != null && total > 0 ? (used / total) * 100 : undefined;

  return {
    id: 'kiro.promptCredits',
    providerId: 'kiro',
    providerLabel: PROVIDER_LABELS.kiro,
    label: 'Prompt credits',
    accountLabel: account.email || 'Kiro account',
    percentUsed: clampPercent(percentUsed),
    percentRemaining: percentUsed == null ? undefined : 100 - clampPercent(percentUsed)!,
    resetAt: account.usage.usageResetAt,
    updatedAt: account.usageUpdatedAt,
    error: account.quotaQueryLastError ?? null,
  };
}

async function parseJsonResponse<T>(response: Response, failureLabel: string): Promise<T> {
  const body = await response.text();
  if (!response.ok) throw new Error(`${failureLabel} returned ${response.status} with body length ${body.length}.`);
  const parsed = JSON.parse(body) as unknown;
  if (isRecord(parsed) && isRecord(parsed.data)) return parsed.data as T;
  return parsed as T;
}

function startOAuthSession(): Promise<KiroOAuthSession> {
  return new Promise((resolve, reject) => {
    const state = randomToken();
    const codeVerifier = randomToken();
    const codeChallenge = pkceChallenge(codeVerifier);

    const startOnPort = (portIndex: number): void => {
      const port = CALLBACK_PORT_CANDIDATES[portIndex];
      if (port == null) {
        reject(new Error('No available Kiro callback port found.'));
        return;
      }

      const callbackUrl = `http://localhost:${port}`;
      const authParams = new URLSearchParams({
        state,
        code_challenge: codeChallenge,
        code_challenge_method: 'S256',
        redirect_uri: callbackUrl,
        redirect_from: 'KiroIDE',
      });
      let timeout: NodeJS.Timeout | undefined;
      let callbackResolve: (value: KiroCallbackData) => void;
      let callbackReject: (reason: Error) => void;
      const callback = new Promise<KiroCallbackData>((resolveCallback, rejectCallback) => {
        callbackResolve = resolveCallback;
        callbackReject = rejectCallback;
      }).finally(() => {
        if (timeout) clearTimeout(timeout);
      });

      const server = http.createServer((request, response) => {
        const url = new URL(request.url ?? '/', callbackUrl);
        const error = normalize(url.searchParams.get('error'));
        const code = normalize(url.searchParams.get('code'));

        if (url.pathname === '/cancel') {
          response.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
          response.end('<h2>Cancelled</h2><p>Return to VS Code.</p>');
          callbackReject(new Error('Kiro OAuth login was cancelled.'));
          return;
        }

        if (!error && !code) {
          response.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
          response.end('<p>Waiting for Kiro login...</p>');
          return;
        }

        if (error) {
          const description = normalize(url.searchParams.get('error_description'));
          response.writeHead(400, { 'Content-Type': 'text/html; charset=utf-8' });
          response.end('<h2>Authorization failed</h2><p>Close this tab and return to VS Code.</p>');
          callbackReject(new Error(description ? `${error}: ${description}` : error));
          return;
        }

        const returnedState = normalize(url.searchParams.get('state'));
        if (!code || !returnedState || returnedState !== state) {
          response.writeHead(400, { 'Content-Type': 'text/html; charset=utf-8' });
          response.end('<h2>State mismatch</h2><p>Close this tab and try connecting again.</p>');
          callbackReject(new Error('State mismatch in Kiro OAuth callback.'));
          return;
        }

        response.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
        response.end('<h2>Connected</h2><p>You can close this tab and return to VS Code.</p><script>window.close();</script>');
        callbackResolve({
          code,
          path: url.pathname,
          loginOption: normalize(url.searchParams.get('login_option')) ?? normalize(url.searchParams.get('loginOption')) ?? '',
          issuerUrl: normalize(url.searchParams.get('issuer_url')) ?? normalize(url.searchParams.get('issuerUrl')),
          idcRegion: normalize(url.searchParams.get('idc_region')) ?? normalize(url.searchParams.get('idcRegion')),
        });
      });

      server.on('error', () => startOnPort(portIndex + 1));
      server.listen(port, '127.0.0.1', () => {
        timeout = setTimeout(() => callbackReject(new Error('Kiro OAuth login timed out.')), KIRO_OAUTH_TIMEOUT_MS);
        resolve({
          server,
          authUrl: `${KIRO_AUTH_PORTAL_URL}?${authParams.toString()}`,
          callbackUrl,
          codeVerifier,
          callback,
        });
      });
    };

    startOnPort(0);
  });
}

function redirectUriFromCallback(session: KiroOAuthSession, callback: KiroCallbackData): string {
  const path = callback.path.startsWith('/') ? callback.path : `/${callback.path || 'oauth/callback'}`;
  const loginOption = callback.loginOption ? `?login_option=${encodeURIComponent(callback.loginOption)}` : '';
  return `${session.callbackUrl}${path}${loginOption}`;
}

async function exchangeCodeForToken(session: KiroOAuthSession, callback: KiroCallbackData): Promise<unknown> {
  const response = await fetch(KIRO_TOKEN_ENDPOINT, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      code: callback.code,
      code_verifier: session.codeVerifier,
      redirect_uri: redirectUriFromCallback(session, callback),
    }),
  });
  const token = await parseJsonResponse<Record<string, unknown>>(response, 'Kiro token exchange');
  if (callback.loginOption) {
    token.login_option = callback.loginOption;
    token.provider = providerFromLoginOption(callback.loginOption);
    token.loginProvider = providerFromLoginOption(callback.loginOption);
  }
  if (callback.issuerUrl) token.issuer_url = callback.issuerUrl;
  if (callback.idcRegion) {
    token.idc_region = callback.idcRegion;
    token.idcRegion = callback.idcRegion;
  }
  return token;
}

async function refreshToken(refreshToken: string): Promise<unknown> {
  const response = await fetch(KIRO_REFRESH_ENDPOINT, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ refreshToken }),
  });
  return parseJsonResponse<unknown>(response, 'Kiro token refresh');
}

async function fetchRuntimeUsage(accessToken: string, profileArn: string): Promise<unknown> {
  const endpoint = runtimeEndpointForRegion(parseProfileArnRegion(profileArn));
  const url = `${endpoint}/getUsageLimits?origin=AI_EDITOR&profileArn=${encodeURIComponent(profileArn)}&resourceType=AGENTIC_REQUEST&isEmailRequired=true`;
  const response = await fetch(url, {
    headers: { Authorization: `Bearer ${accessToken.trim()}` },
  });
  const body = await response.text();
  if (!response.ok) {
    const prefix = response.status === 403 ? 'BANNED:' : '';
    throw new Error(`${prefix}Kiro runtime usage returned ${response.status} with body length ${body.length}.`);
  }
  return JSON.parse(body) as unknown;
}

function accountFromToken(token: unknown, usage: KiroUsageSummary, existing?: KiroAccount): { account: KiroAccount; credential: KiroCredential } {
  const accessToken = pickString(token, [['accessToken'], ['access_token'], ['token'], ['idToken'], ['id_token']]);
  if (!accessToken) throw new Error('Kiro auth token missing access token field.');
  const refresh = pickString(token, [['refreshToken'], ['refresh_token']]);
  const profileArn = extractProfileArn(token);
  const email = usage.email
    ?? pickString(token, [['email'], ['userEmail'], ['login_hint'], ['loginHint']])
    ?? decodeJwtEmail(accessToken)
    ?? '';
  const id = existing?.id ?? buildAccountId(email, profileArn, accessToken);
  const rawPlan = usage.planName ?? pickString(token, [['planName'], ['plan'], ['planTier']]);

  return {
    account: {
      id,
      email,
      loginProvider: pickString(token, [['login_option'], ['provider'], ['loginProvider']]),
      planName: normalizeKiroPlan(rawPlan),
      usage: {
        ...usage,
        planName: normalizeKiroPlan(rawPlan),
        email,
      },
      profileArn,
      idcRegion: pickString(token, [['idc_region'], ['idcRegion'], ['region']]),
      quotaQueryLastError: null,
      quotaQueryLastErrorAt: null,
      usageUpdatedAt: now(),
      createdAt: existing?.createdAt ?? now(),
      lastUsed: now(),
    },
    credential: {
      id,
      accessToken,
      refreshToken: refresh,
      rawToken: token,
      profileArn,
      idcRegion: pickString(token, [['idc_region'], ['idcRegion'], ['region']]),
    },
  };
}

export class KiroProvider {
  constructor(private readonly context: vscode.ExtensionContext) {}

  async connect(): Promise<KiroAccount> {
    const session = await startOAuthSession();
    try {
      await vscode.env.openExternal(vscode.Uri.parse(session.authUrl));
      const callback = await session.callback;
      const token = await exchangeCodeForToken(session, callback);
      const accessToken = pickString(token, [['accessToken'], ['access_token'], ['token'], ['idToken'], ['id_token']]);
      const profileArn = extractProfileArn(token);
      if (!accessToken) throw new Error('Kiro auth token missing access token field.');
      if (!profileArn) throw new Error('Kiro auth token did not include a profile ARN.');
      const usage = buildKiroUsageSummary(await fetchRuntimeUsage(accessToken, profileArn));
      return await this.upsertToken(token, usage);
    } finally {
      session.server.close();
    }
  }

  async refreshAll(): Promise<KiroAccount[]> {
    const accounts = await this.getAccounts();
    const refreshed: KiroAccount[] = [];
    for (const account of accounts) refreshed.push(await this.refreshAccount(account.id));
    return refreshed;
  }

  async refreshAccount(accountId: string): Promise<KiroAccount> {
    const account = await this.getAccount(accountId);
    const credential = await this.getCredential(accountId);
    if (!account || !credential) throw new Error('Kiro account is not connected.');
    const profileArn = credential.profileArn ?? account.profileArn;

    if (!profileArn) {
      return this.saveAccount({
        ...account,
        quotaQueryLastError: 'Cannot refresh: no profile ARN in stored credentials.',
        quotaQueryLastErrorAt: now(),
      });
    }

    try {
      const usage = buildKiroUsageSummary(await fetchRuntimeUsage(credential.accessToken, profileArn));
      return await this.upsertToken(credential.rawToken, usage, account);
    } catch (error) {
      const firstMessage = error instanceof Error ? error.message : String(error);
      if (credential.refreshToken) {
        try {
          const token = await refreshToken(credential.refreshToken);
          const accessToken = pickString(token, [['accessToken'], ['access_token'], ['token'], ['idToken'], ['id_token']]);
          const nextProfileArn = extractProfileArn(token) ?? profileArn;
          if (!accessToken) throw new Error('Kiro refresh token response did not include an access token.');
          const usage = buildKiroUsageSummary(await fetchRuntimeUsage(accessToken, nextProfileArn));
          return await this.upsertToken(token, usage, account);
        } catch (refreshError) {
          const message = refreshError instanceof Error ? refreshError.message : String(refreshError);
          return this.saveAccount({ ...account, quotaQueryLastError: message, quotaQueryLastErrorAt: now() });
        }
      }

      const isBanned = firstMessage.startsWith('BANNED:');
      const cleanMessage = isBanned ? firstMessage.slice('BANNED:'.length) : firstMessage;
      return this.saveAccount({
        ...account,
        status: isBanned ? 'banned' : account.status,
        statusReason: isBanned ? cleanMessage : account.statusReason,
        quotaQueryLastError: cleanMessage,
        quotaQueryLastErrorAt: now(),
      });
    }
  }

  async disconnect(): Promise<void> {
    const accounts = await this.getAccounts();
    if (accounts.length === 0) {
      void vscode.window.showInformationMessage('No Kiro accounts are connected.');
      return;
    }

    const picked = await vscode.window.showQuickPick(
      accounts.map((account) => ({
        label: account.email || 'Kiro account',
        description: account.planName ?? account.loginProvider ?? 'Kiro account',
        account,
      })),
      { title: 'Disconnect Kiro' },
    );
    if (!picked) return;

    const confirmed = await vscode.window.showWarningMessage(
      `Disconnect Kiro account ${picked.account.email || 'Kiro account'}? Extension-stored tokens and cached quota data will be deleted.`,
      { modal: true },
      'Disconnect',
    );
    if (confirmed !== 'Disconnect') return;

    const store = await this.getCredentialStore();
    delete store.accounts[picked.account.id];
    await this.saveCredentialStore(store);
    await this.context.globalState.update(KIRO_ACCOUNTS_KEY, accounts.filter((account) => account.id !== picked.account.id));
  }

  async getTracks(): Promise<QuotaTrack[]> {
    const accounts = await this.getAccounts();
    return accounts.map(trackFromAccount).filter((track) => track.percentUsed != null || track.percentRemaining != null || track.error);
  }

  async hasAccounts(): Promise<boolean> {
    return (await this.getAccounts()).length > 0;
  }

  private async upsertToken(token: unknown, usage: KiroUsageSummary, existing?: KiroAccount): Promise<KiroAccount> {
    const result = accountFromToken(token, usage, existing);
    const store = await this.getCredentialStore();
    store.accounts[result.account.id] = result.credential;
    await this.saveCredentialStore(store);
    return this.saveAccount(result.account);
  }

  private async getAccounts(): Promise<KiroAccount[]> {
    return this.context.globalState.get<KiroAccount[]>(KIRO_ACCOUNTS_KEY, []);
  }

  private async getAccount(accountId: string): Promise<KiroAccount | undefined> {
    return (await this.getAccounts()).find((account) => account.id === accountId);
  }

  private async saveAccount(account: KiroAccount): Promise<KiroAccount> {
    const accounts = await this.getAccounts();
    const next = [account, ...accounts.filter((item) => item.id !== account.id)];
    await this.context.globalState.update(KIRO_ACCOUNTS_KEY, next);
    return account;
  }

  private async getCredential(accountId: string): Promise<KiroCredential | undefined> {
    const store = await this.getCredentialStore();
    return store.accounts[accountId];
  }

  private async getCredentialStore(): Promise<KiroCredentialStore> {
    const raw = await this.context.secrets.get(KIRO_SECRET_KEY);
    if (!raw) return { accounts: {} };

    try {
      const parsed = JSON.parse(raw) as KiroCredentialStore;
      return parsed && typeof parsed === 'object' && parsed.accounts ? parsed : { accounts: {} };
    } catch {
      return { accounts: {} };
    }
  }

  private async saveCredentialStore(store: KiroCredentialStore): Promise<void> {
    await this.context.secrets.store(KIRO_SECRET_KEY, JSON.stringify(store));
  }
}
