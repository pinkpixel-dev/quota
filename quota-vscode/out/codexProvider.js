"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.CodexProvider = void 0;
// @env node
const crypto = __importStar(require("node:crypto"));
const http = __importStar(require("node:http"));
const vscode = __importStar(require("vscode"));
const constants_1 = require("./constants");
const CODEX_USAGE_ENDPOINT = 'https://chatgpt.com/backend-api/wham/usage';
const CODEX_OAUTH_AUTHORIZE_ENDPOINT = 'https://auth.openai.com/oauth/authorize';
const CODEX_OAUTH_TOKEN_ENDPOINT = 'https://auth.openai.com/oauth/token';
const CODEX_OAUTH_CLIENT_ID = 'app_EMoamEEZ73f0CkXaXp7hrann';
const CODEX_OAUTH_SCOPES = 'openid profile email offline_access api.connectors.read api.connectors.invoke';
const CODEX_OAUTH_CALLBACK_PORT = 1455;
const CODEX_OAUTH_TIMEOUT_MS = 300_000;
const CODEX_SECRET_KEY = 'quota.codex.credentials';
const CODEX_ACCOUNTS_KEY = 'quota.codex.accounts';
function now() {
    return Date.now();
}
function normalize(value) {
    return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}
function randomToken() {
    return crypto.randomBytes(32).toString('base64url');
}
function pkceChallenge(codeVerifier) {
    return crypto.createHash('sha256').update(codeVerifier).digest('base64url');
}
function clampPercent(value) {
    if (value == null || !Number.isFinite(value))
        return undefined;
    return Math.min(100, Math.max(0, Math.round(value)));
}
function usedFromRemaining(value) {
    const remaining = clampPercent(value ?? undefined);
    return remaining == null ? undefined : 100 - remaining;
}
function decodeJwtPayload(token) {
    const parts = token.split('.');
    if (parts.length < 2)
        throw new Error('Invalid Codex JWT token format.');
    const json = Buffer.from(parts[1], 'base64url').toString('utf8');
    return JSON.parse(json);
}
function buildAccountId(email, accountId, organizationId) {
    const seed = [email.trim(), accountId, organizationId].filter(Boolean).join('|');
    return `codex_${crypto.createHash('md5').update(seed).digest('hex')}`;
}
function accountFromTokenResponse(response, existing) {
    if (response.error) {
        throw new Error(response.error_description ?? String(response.error));
    }
    const idToken = normalize(response.id_token);
    const accessToken = normalize(response.access_token);
    if (!idToken)
        throw new Error('Codex token response did not include an id_token.');
    if (!accessToken)
        throw new Error('Codex token response did not include an access_token.');
    const payload = decodeJwtPayload(idToken);
    const authData = payload['https://api.openai.com/auth'];
    const profileData = payload['https://api.openai.com/profile'];
    const email = normalize(payload.email) ?? normalize(profileData?.email);
    if (!email)
        throw new Error('Codex id_token does not include an email.');
    const userId = normalize(authData?.chatgpt_user_id) ?? normalize(authData?.user_id) ?? normalize(payload.sub);
    const plan = normalize(authData?.chatgpt_plan_type);
    const accountId = normalize(authData?.account_id) ?? normalize(authData?.chatgpt_account_id);
    const organizationId = normalize(authData?.organization_id) ?? normalize(authData?.chatgpt_organization_id);
    const id = buildAccountId(email, accountId, organizationId);
    const createdAt = existing?.createdAt ?? now();
    const refreshToken = normalize(response.refresh_token);
    return {
        account: {
            id,
            email,
            authMode: 'oauth',
            userId,
            plan: plan ?? existing?.plan,
            accountId,
            organizationId,
            quota: existing?.quota ?? {},
            quotaQueryLastError: existing?.quotaQueryLastError ?? null,
            quotaQueryLastErrorAt: existing?.quotaQueryLastErrorAt ?? null,
            usageUpdatedAt: existing?.usageUpdatedAt ?? null,
            createdAt,
            lastUsed: now(),
        },
        credential: {
            id,
            idToken,
            accessToken,
            refreshToken,
        },
    };
}
function remainingPercent(window) {
    return window == null ? undefined : 100 - Math.min(100, Math.max(0, Math.round(window.used_percent ?? 0)));
}
function windowMinutes(window) {
    const seconds = window?.limit_window_seconds;
    if (seconds == null || seconds <= 0)
        return undefined;
    return Math.ceil(seconds / 60);
}
function resetAt(window) {
    if (window?.reset_at != null)
        return window.reset_at * 1000;
    if (window?.reset_after_seconds == null || window.reset_after_seconds < 0)
        return undefined;
    return now() + window.reset_after_seconds * 1000;
}
function quotaFromUsage(value) {
    const primary = value.rate_limit?.primary_window;
    const secondary = value.rate_limit?.secondary_window;
    return {
        plan: normalize(value.plan_type),
        quota: {
            hourlyRemainingPercent: remainingPercent(primary),
            hourlyResetAt: resetAt(primary),
            hourlyWindowMinutes: windowMinutes(primary),
            weeklyRemainingPercent: remainingPercent(secondary),
            weeklyResetAt: resetAt(secondary),
            weeklyWindowMinutes: windowMinutes(secondary),
        },
    };
}
function trackFromAccount(account, id, label) {
    const isPrimary = id === 'codex.primary';
    const remaining = isPrimary ? account.quota.hourlyRemainingPercent : account.quota.weeklyRemainingPercent;
    const reset = isPrimary ? account.quota.hourlyResetAt : account.quota.weeklyResetAt;
    return {
        id,
        providerId: 'codex',
        providerLabel: constants_1.PROVIDER_LABELS.codex,
        label,
        accountLabel: account.email,
        percentUsed: usedFromRemaining(remaining),
        percentRemaining: remaining ?? undefined,
        resetAt: reset,
        updatedAt: account.usageUpdatedAt,
        error: account.quotaQueryLastError ?? null,
    };
}
async function parseJsonResponse(response, failureLabel) {
    const body = await response.text();
    if (!response.ok) {
        throw new Error(`${failureLabel} returned ${response.status} with body length ${body.length}.`);
    }
    return JSON.parse(body);
}
async function exchangeOAuthCode(session, code) {
    const response = await fetch(CODEX_OAUTH_TOKEN_ENDPOINT, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
            grant_type: 'authorization_code',
            client_id: CODEX_OAUTH_CLIENT_ID,
            code,
            redirect_uri: session.redirectUri,
            code_verifier: session.codeVerifier,
        }).toString(),
    });
    return parseJsonResponse(response, 'Codex OAuth token exchange');
}
async function refreshToken(refreshToken) {
    const response = await fetch(CODEX_OAUTH_TOKEN_ENDPOINT, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
            grant_type: 'refresh_token',
            client_id: CODEX_OAUTH_CLIENT_ID,
            refresh_token: refreshToken,
        }).toString(),
    });
    return parseJsonResponse(response, 'Codex token refresh');
}
async function fetchCodexUsage(account, credential) {
    const headers = {
        Accept: 'application/json',
        Authorization: `Bearer ${credential.accessToken}`,
    };
    if (account.accountId)
        headers['ChatGPT-Account-Id'] = account.accountId;
    const response = await fetch(CODEX_USAGE_ENDPOINT, { headers });
    if (response.status === 401 || response.status === 403) {
        throw new Error(`unauthorized:${response.status}`);
    }
    const usage = await parseJsonResponse(response, 'Codex quota API');
    return quotaFromUsage(usage);
}
function startOAuthSession() {
    return new Promise((resolve, reject) => {
        const state = randomToken();
        const codeVerifier = randomToken();
        const redirectUri = `http://localhost:${CODEX_OAUTH_CALLBACK_PORT}/auth/callback`;
        const codeChallenge = pkceChallenge(codeVerifier);
        const authParams = new URLSearchParams({
            client_id: CODEX_OAUTH_CLIENT_ID,
            response_type: 'code',
            redirect_uri: redirectUri,
            scope: CODEX_OAUTH_SCOPES,
            state,
            code_challenge: codeChallenge,
            code_challenge_method: 'S256',
            originator: 'codex_vscode',
        });
        let timeout;
        const callback = new Promise((callbackResolve, callbackReject) => {
            const server = http.createServer((request, response) => {
                const url = new URL(request.url ?? '/', redirectUri);
                if (url.pathname === '/cancel') {
                    response.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8' });
                    response.end('Codex connection cancelled.');
                    callbackReject(new Error('Codex OAuth login was cancelled.'));
                    return;
                }
                const code = normalize(url.searchParams.get('code'));
                const returnedState = normalize(url.searchParams.get('state'));
                const error = normalize(url.searchParams.get('error'));
                if (error) {
                    response.writeHead(400, { 'Content-Type': 'text/plain; charset=utf-8' });
                    response.end('Codex connection failed. Return to VS Code and try again.');
                    callbackReject(new Error(`Codex OAuth error: ${error}`));
                    return;
                }
                if (!code || !returnedState || returnedState !== state) {
                    response.writeHead(400, { 'Content-Type': 'text/plain; charset=utf-8' });
                    response.end('Codex connection failed. Return to VS Code and try again.');
                    callbackReject(new Error('Codex OAuth callback was invalid.'));
                    return;
                }
                response.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8' });
                response.end('Codex connected. You can return to VS Code.');
                callbackResolve({ code, state: returnedState });
            });
            server.on('error', (error) => {
                callbackReject(error);
                reject(error);
            });
            server.listen(CODEX_OAUTH_CALLBACK_PORT, () => {
                timeout = setTimeout(() => callbackReject(new Error('Codex OAuth login timed out.')), CODEX_OAUTH_TIMEOUT_MS);
                resolve({
                    server,
                    authUrl: `${CODEX_OAUTH_AUTHORIZE_ENDPOINT}?${authParams.toString()}`,
                    redirectUri,
                    codeVerifier,
                    state,
                    callback,
                });
            });
        });
        callback.finally(() => {
            if (timeout)
                clearTimeout(timeout);
        }).catch(() => undefined);
    });
}
class CodexProvider {
    context;
    constructor(context) {
        this.context = context;
    }
    async connect() {
        const session = await startOAuthSession();
        try {
            await vscode.env.openExternal(vscode.Uri.parse(session.authUrl));
            const { code } = await session.callback;
            const tokenResponse = await exchangeOAuthCode(session, code);
            const account = await this.upsertTokenResponse(tokenResponse);
            await this.refreshAccount(account.id);
            return (await this.getAccount(account.id)) ?? account;
        }
        finally {
            session.server.close();
        }
    }
    async refreshAll() {
        const accounts = await this.getAccounts();
        const refreshed = [];
        for (const account of accounts) {
            refreshed.push(await this.refreshAccount(account.id));
        }
        return refreshed;
    }
    async refreshAccount(accountId) {
        const account = await this.getAccount(accountId);
        const credential = await this.getCredential(accountId);
        if (!account || !credential)
            throw new Error('Codex account is not connected.');
        try {
            const parsed = await fetchCodexUsage(account, credential);
            return await this.saveAccount({
                ...account,
                plan: parsed.plan ?? account.plan,
                quota: parsed.quota,
                quotaQueryLastError: null,
                quotaQueryLastErrorAt: null,
                usageUpdatedAt: now(),
                lastUsed: now(),
            });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            if (message.startsWith('unauthorized:') && credential.refreshToken) {
                try {
                    const tokenResponse = await refreshToken(credential.refreshToken);
                    const updated = await this.upsertTokenResponse(tokenResponse, account);
                    return await this.refreshAccount(updated.id);
                }
                catch (refreshError) {
                    const refreshMessage = refreshError instanceof Error ? refreshError.message : String(refreshError);
                    return await this.saveAccount({
                        ...account,
                        quotaQueryLastError: refreshMessage,
                        quotaQueryLastErrorAt: now(),
                    });
                }
            }
            return await this.saveAccount({
                ...account,
                quotaQueryLastError: message,
                quotaQueryLastErrorAt: now(),
            });
        }
    }
    async disconnect() {
        const accounts = await this.getAccounts();
        if (accounts.length === 0) {
            void vscode.window.showInformationMessage('No Codex accounts are connected.');
            return;
        }
        const picked = await vscode.window.showQuickPick(accounts.map((account) => ({
            label: account.email,
            description: account.plan ?? 'Codex account',
            account,
        })), { title: 'Disconnect Codex' });
        if (!picked)
            return;
        const confirmed = await vscode.window.showWarningMessage(`Disconnect Codex account ${picked.account.email}? Extension-stored tokens and cached quota data will be deleted.`, { modal: true }, 'Disconnect');
        if (confirmed !== 'Disconnect')
            return;
        const store = await this.getCredentialStore();
        delete store.accounts[picked.account.id];
        await this.saveCredentialStore(store);
        await this.context.globalState.update(CODEX_ACCOUNTS_KEY, accounts.filter((account) => account.id !== picked.account.id));
    }
    async getTracks() {
        const accounts = await this.getAccounts();
        return accounts.flatMap((account) => [
            trackFromAccount(account, 'codex.primary', '5h usage'),
            trackFromAccount(account, 'codex.weekly', 'Weekly usage'),
        ]).filter((track) => track.percentUsed != null || track.percentRemaining != null || track.error);
    }
    async hasAccounts() {
        return (await this.getAccounts()).length > 0;
    }
    async upsertTokenResponse(response, existing) {
        const result = accountFromTokenResponse(response, existing);
        const store = await this.getCredentialStore();
        store.accounts[result.account.id] = result.credential;
        if (!result.credential.refreshToken && existing) {
            const oldCredential = await this.getCredential(existing.id);
            if (oldCredential?.refreshToken)
                store.accounts[result.account.id].refreshToken = oldCredential.refreshToken;
        }
        await this.saveCredentialStore(store);
        return this.saveAccount(result.account);
    }
    async getAccounts() {
        return this.context.globalState.get(CODEX_ACCOUNTS_KEY, []);
    }
    async getAccount(accountId) {
        return (await this.getAccounts()).find((account) => account.id === accountId);
    }
    async saveAccount(account) {
        const accounts = await this.getAccounts();
        const next = [account, ...accounts.filter((item) => item.id !== account.id)];
        await this.context.globalState.update(CODEX_ACCOUNTS_KEY, next);
        return account;
    }
    async getCredential(accountId) {
        const store = await this.getCredentialStore();
        return store.accounts[accountId];
    }
    async getCredentialStore() {
        const raw = await this.context.secrets.get(CODEX_SECRET_KEY);
        if (!raw)
            return { accounts: {} };
        try {
            const parsed = JSON.parse(raw);
            return parsed && typeof parsed === 'object' && parsed.accounts ? parsed : { accounts: {} };
        }
        catch {
            return { accounts: {} };
        }
    }
    async saveCredentialStore(store) {
        await this.context.secrets.store(CODEX_SECRET_KEY, JSON.stringify(store));
    }
}
exports.CodexProvider = CodexProvider;
//# sourceMappingURL=codexProvider.js.map