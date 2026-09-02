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
exports.GrokProvider = void 0;
// @env node
const crypto = __importStar(require("node:crypto"));
const fs = __importStar(require("node:fs/promises"));
const os = __importStar(require("node:os"));
const path = __importStar(require("node:path"));
const vscode = __importStar(require("vscode"));
const authError_1 = require("./authError");
const constants_1 = require("./constants");
const grokUsage_1 = require("./grokUsage");
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
function now() {
    return Date.now();
}
function normalize(value) {
    return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}
function isRecord(value) {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function clampPercent(value) {
    if (value == null || !Number.isFinite(value))
        return undefined;
    return Math.min(100, Math.max(0, Math.round(value)));
}
function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}
function grokHome() {
    return normalize(process.env.GROK_HOME) ?? path.join(os.homedir(), '.grok');
}
function decodeJwtPayload(token) {
    const payload = token.split('.')[1];
    if (!payload)
        return undefined;
    try {
        const claims = JSON.parse(Buffer.from(payload, 'base64url').toString('utf8'));
        return isRecord(claims) ? claims : undefined;
    }
    catch {
        return undefined;
    }
}
function buildAccountId(email, principalId) {
    const seed = principalId ? `${email.trim().toLowerCase()}|${principalId}` : email.trim().toLowerCase();
    return `grok_${crypto.createHash('md5').update(seed).digest('hex')}`;
}
function splitIssuerKey(issuerKey) {
    const separator = issuerKey.indexOf('::');
    if (separator < 0)
        return { issuer: normalize(issuerKey) };
    return {
        issuer: normalize(issuerKey.slice(0, separator)),
        clientId: normalize(issuerKey.slice(separator + 2)),
    };
}
function parseExpiry(value) {
    if (typeof value === 'number' && Number.isFinite(value) && value > 0) {
        return value > 10_000_000_000 ? Math.trunc(value) : Math.trunc(value * 1000);
    }
    if (typeof value === 'string' && value.trim()) {
        const parsed = Date.parse(value.trim());
        return Number.isFinite(parsed) ? parsed : undefined;
    }
    return undefined;
}
function authHeaders(accessToken) {
    // Grok rejects any X-XAI-Token-Auth value with 401; bearer auth only.
    return {
        Accept: 'application/json',
        Authorization: `Bearer ${accessToken.trim()}`,
    };
}
async function parseJsonResponse(response, failureLabel) {
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
    return JSON.parse(body);
}
async function requestDeviceCode() {
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
    const device = await parseJsonResponse(response, 'Grok device code request');
    if (device.error) {
        throw new Error(device.error_description ?? `Grok device authorization failed: ${device.error}`);
    }
    if (!normalize(device.device_code))
        throw new Error('Grok device code response did not include a device code.');
    return device;
}
async function exchangeDeviceToken(deviceCode) {
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
        return JSON.parse(body);
    }
    catch {
        throw new Error(`Grok token request returned ${response.status} with body length ${body.length}.`);
    }
}
async function pollDeviceToken(deviceCode, intervalSeconds, expiresInSeconds, token) {
    const expiresAt = now() + expiresInSeconds * 1000;
    let waitSeconds = Math.max(1, intervalSeconds);
    while (now() < expiresAt) {
        if (token.isCancellationRequested)
            throw new Error('Grok authorization was cancelled.');
        const response = await exchangeDeviceToken(deviceCode);
        if (!response.error) {
            if (!normalize(response.access_token))
                throw new Error('Grok token response did not include an access token.');
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
        if (response.error === 'expired_token')
            throw new Error('Grok authorization expired. Start again.');
        if (response.error === 'access_denied')
            throw new Error('Grok authorization was denied.');
        throw new Error(response.error_description ?? `Grok authorization failed: ${response.error}`);
    }
    throw new Error('Grok authorization expired. Start again.');
}
async function refreshAccessToken(refreshToken) {
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
    if (!response.ok)
        throw new Error((0, authError_1.tokenRefreshErrorMessage)('Grok', response.status, body));
    const parsed = JSON.parse(body);
    if (parsed.error)
        throw new Error((0, authError_1.tokenRefreshErrorMessage)('Grok', response.status, body));
    if (!normalize(parsed.access_token))
        throw new Error('Grok refresh response did not include an access token.');
    return parsed;
}
async function fetchBilling(accessToken, format) {
    const url = format ? `${GROK_BILLING_ENDPOINT}?format=${encodeURIComponent(format)}` : GROK_BILLING_ENDPOINT;
    const response = await fetch(url, { headers: authHeaders(accessToken) });
    return parseJsonResponse(response, 'Grok billing request');
}
async function fetchIdentity(accessToken) {
    const response = await fetch(GROK_USER_ENDPOINT, { headers: authHeaders(accessToken) });
    const raw = await parseJsonResponse(response, 'Grok user request');
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
async function fetchSubscriptionTier(accessToken) {
    try {
        const response = await fetch(GROK_SUBSCRIPTIONS_ENDPOINT, { headers: authHeaders(accessToken) });
        if (!response.ok)
            return undefined;
        const raw = (await response.json());
        const list = isRecord(raw) && Array.isArray(raw.subscriptions) ? raw.subscriptions : [];
        const active = list.find((item) => isRecord(item) && normalize(item.tier) != null);
        return isRecord(active) ? normalize(active.tier) : undefined;
    }
    catch {
        return undefined;
    }
}
function tracksFromAccount(account) {
    const usage = account.usage;
    const accountLabel = account.email || account.displayName || 'Grok account';
    const base = {
        providerId: 'grok',
        providerLabel: constants_1.PROVIDER_LABELS.grok,
        accountLabel,
        updatedAt: account.usageUpdatedAt,
        error: account.quotaQueryLastError ?? null,
    };
    const monthlyPercent = usage.monthlyLimit != null && usage.monthlyLimit > 0 && usage.monthlyUsed != null
        ? (usage.monthlyUsed / usage.monthlyLimit) * 100
        : undefined;
    const onDemandPercent = usage.onDemandCap != null && usage.onDemandCap > 0 && usage.onDemandUsed != null
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
            percentRemaining: monthlyPercent == null ? undefined : 100 - clampPercent(monthlyPercent),
            resetAt: usage.monthlyPeriodEndAt ?? null,
        },
        {
            ...base,
            id: 'grok.onDemand',
            label: 'On-demand spend',
            percentUsed: clampPercent(onDemandPercent),
            percentRemaining: onDemandPercent == null ? undefined : 100 - clampPercent(onDemandPercent),
            resetAt: usage.monthlyPeriodEndAt ?? null,
        },
    ];
}
class GrokProvider {
    context;
    constructor(context) {
        this.context = context;
    }
    async connect() {
        const device = await requestDeviceCode();
        const authUri = normalize(device.verification_uri_complete)
            ?? normalize(device.verification_uri)
            ?? 'https://accounts.x.ai/oauth2/device';
        const userCode = normalize(device.user_code) ?? '';
        await vscode.env.openExternal(vscode.Uri.parse(authUri));
        if (userCode)
            await vscode.env.clipboard.writeText(userCode);
        const tokenResponse = await vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: userCode
                ? `Grok device code ${userCode} (copied). Confirm it in the browser.`
                : 'Waiting for Grok authorization in the browser.',
            cancellable: true,
        }, (_progress, cancellationToken) => pollDeviceToken(device.device_code, device.interval ?? 5, device.expires_in ?? 1800, cancellationToken));
        return this.upsertTokenResponse(tokenResponse);
    }
    async importLocal() {
        const authPath = path.join(grokHome(), 'auth.json');
        let raw;
        try {
            raw = await fs.readFile(authPath, 'utf8');
        }
        catch {
            throw new Error(`Could not read the Grok auth file: ${authPath}`);
        }
        const parsed = JSON.parse(raw);
        if (!isRecord(parsed))
            throw new Error('The Grok auth file is not a JSON object.');
        const imported = [];
        let lastError;
        for (const [issuerKey, value] of Object.entries(parsed)) {
            if (!isRecord(value))
                continue;
            const entry = value;
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
            const credential = {
                id: buildAccountId(email, principalId),
                accessToken,
                refreshToken: normalize(entry.refresh_token),
                expiresAt: parseExpiry(entry.expires_at) ?? parseExpiry(claims?.exp),
                oidcIssuer: normalize(entry.oidc_issuer) ?? issuerInfo.issuer ?? GROK_OAUTH_ISSUER,
                oidcClientId: normalize(entry.oidc_client_id) ?? issuerInfo.clientId ?? GROK_OAUTH_CLIENT_ID,
            };
            const existing = await this.getAccount(credential.id);
            const account = {
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
        for (const account of imported)
            await this.refreshAccount(account.id);
        return this.getAccounts();
    }
    async refreshAll() {
        const accounts = await this.getAccounts();
        const refreshed = [];
        for (const account of accounts)
            refreshed.push(await this.refreshAccount(account.id));
        return refreshed;
    }
    async refreshAccount(accountId) {
        const account = await this.getAccount(accountId);
        const credential = await this.getCredential(accountId);
        if (!account || !credential)
            throw new Error('Grok account is not connected.');
        try {
            return await this.applyUsage(account, credential.accessToken);
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            if (message.startsWith('forbidden:')) {
                return this.saveAccount({
                    ...account,
                    quotaQueryLastError: (0, authError_1.reauthenticationMessage)('Grok'),
                    quotaQueryLastErrorAt: now(),
                });
            }
            if (!message.startsWith('unauthorized:')) {
                return this.saveAccount({ ...account, quotaQueryLastError: message, quotaQueryLastErrorAt: now() });
            }
            if (!credential.refreshToken) {
                return this.saveAccount({
                    ...account,
                    quotaQueryLastError: (0, authError_1.reauthenticationMessage)('Grok'),
                    quotaQueryLastErrorAt: now(),
                });
            }
            try {
                const tokenResponse = await refreshAccessToken(credential.refreshToken);
                const nextCredential = {
                    ...credential,
                    accessToken: normalize(tokenResponse.access_token),
                    refreshToken: normalize(tokenResponse.refresh_token) ?? credential.refreshToken,
                    expiresAt: tokenResponse.expires_in != null ? now() + tokenResponse.expires_in * 1000 : credential.expiresAt,
                };
                await this.saveCredential(nextCredential);
                return await this.applyUsage(account, nextCredential.accessToken);
            }
            catch (refreshError) {
                const refreshMessage = refreshError instanceof Error ? refreshError.message : String(refreshError);
                const stillRejected = refreshMessage.startsWith('unauthorized:') || refreshMessage.startsWith('forbidden:');
                return this.saveAccount({
                    ...account,
                    quotaQueryLastError: stillRejected ? (0, authError_1.reauthenticationMessage)('Grok') : refreshMessage,
                    quotaQueryLastErrorAt: now(),
                });
            }
        }
    }
    async disconnect() {
        const accounts = await this.getAccounts();
        if (accounts.length === 0) {
            void vscode.window.showInformationMessage('No Grok accounts are connected.');
            return;
        }
        const picked = await vscode.window.showQuickPick(accounts.map((account) => ({
            label: account.email || 'Grok account',
            description: account.plan ?? account.teamName ?? 'Grok account',
            account,
        })), { title: 'Disconnect Grok' });
        if (!picked)
            return;
        const confirmed = await vscode.window.showWarningMessage(`Disconnect Grok account ${picked.account.email || 'Grok account'}? Extension-stored tokens and cached quota data will be deleted.`, { modal: true }, 'Disconnect');
        if (confirmed !== 'Disconnect')
            return;
        const store = await this.getCredentialStore();
        delete store.accounts[picked.account.id];
        await this.saveCredentialStore(store);
        await this.context.globalState.update(GROK_ACCOUNTS_KEY, accounts.filter((account) => account.id !== picked.account.id));
    }
    async getTracks() {
        const accounts = await this.getAccounts();
        return accounts
            .flatMap(tracksFromAccount)
            .filter((track) => track.percentUsed != null || track.percentRemaining != null || track.error);
    }
    async hasAccounts() {
        return (await this.getAccounts()).length > 0;
    }
    async applyUsage(account, accessToken) {
        const [credits, history] = await Promise.all([
            fetchBilling(accessToken, 'credits'),
            fetchBilling(accessToken, 'history').catch(() => undefined),
        ]);
        const usage = (0, grokUsage_1.buildGrokUsageSummary)(credits, history);
        const identity = await fetchIdentity(accessToken).catch(() => ({}));
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
    async upsertTokenResponse(tokenResponse, existing) {
        const accessToken = normalize(tokenResponse.access_token);
        if (!accessToken)
            throw new Error('Grok token response did not include an access token.');
        const claims = decodeJwtPayload(normalize(tokenResponse.id_token) ?? accessToken);
        const identity = await fetchIdentity(accessToken).catch(() => ({}));
        const email = identity.email ?? normalize(claims?.email);
        if (!email)
            throw new Error('Grok authorization did not return an account email.');
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
    async getAccounts() {
        return this.context.globalState.get(GROK_ACCOUNTS_KEY, []);
    }
    async getAccount(accountId) {
        return (await this.getAccounts()).find((account) => account.id === accountId);
    }
    async saveAccount(account) {
        const accounts = await this.getAccounts();
        const next = [account, ...accounts.filter((item) => item.id !== account.id)];
        await this.context.globalState.update(GROK_ACCOUNTS_KEY, next);
        return account;
    }
    async getCredential(accountId) {
        const store = await this.getCredentialStore();
        return store.accounts[accountId];
    }
    async saveCredential(credential) {
        const store = await this.getCredentialStore();
        store.accounts[credential.id] = credential;
        await this.saveCredentialStore(store);
    }
    async getCredentialStore() {
        const raw = await this.context.secrets.get(GROK_SECRET_KEY);
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
        await this.context.secrets.store(GROK_SECRET_KEY, JSON.stringify(store));
    }
}
exports.GrokProvider = GrokProvider;
//# sourceMappingURL=grokProvider.js.map