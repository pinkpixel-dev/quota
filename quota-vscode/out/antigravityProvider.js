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
exports.AntigravityProvider = void 0;
// @env node
const crypto = __importStar(require("node:crypto"));
const http = __importStar(require("node:http"));
const os = __importStar(require("node:os"));
const vscode = __importStar(require("vscode"));
const antigravityUsage_1 = require("./antigravityUsage");
const constants_1 = require("./constants");
const GOOGLE_TOKEN_ENDPOINT = 'https://oauth2.googleapis.com/token';
const GOOGLE_USERINFO_ENDPOINT = 'https://www.googleapis.com/oauth2/v2/userinfo';
const ANTIGRAVITY_OAUTH_AUTHORIZE_ENDPOINT = 'https://accounts.google.com/o/oauth2/v2/auth';
const ANTIGRAVITY_OAUTH_CLIENT_ID = '1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com';
const ANTIGRAVITY_OAUTH_CLIENT_SECRET = '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';
const ANTIGRAVITY_OAUTH_CALLBACK_PATH = '/oauth-callback';
const ANTIGRAVITY_OAUTH_TIMEOUT_MS = 300_000;
const CODE_ASSIST_BASE_ENDPOINT = 'https://daily-cloudcode-pa.googleapis.com';
const CODE_ASSIST_LOAD_ENDPOINT = 'v1internal:loadCodeAssist';
const CODE_ASSIST_FETCH_MODELS_ENDPOINT = 'v1internal:fetchAvailableModels';
const CODE_ASSIST_RETRIEVE_QUOTA_ENDPOINT = 'v1internal:retrieveUserQuotaSummary';
const ANTIGRAVITY_IDE_VERSION = '1.20.5';
const ANTIGRAVITY_GOOGLE_API_NODEJS_CLIENT_VERSION = '10.3.0';
const ANTIGRAVITY_X_GOOG_API_CLIENT = 'gl-node/22.21.1';
const ANTIGRAVITY_SECRET_KEY = 'quota.antigravity.credentials';
const ANTIGRAVITY_ACCOUNTS_KEY = 'quota.antigravity.accounts';
const ANTIGRAVITY_SCOPES = [
    'https://www.googleapis.com/auth/cloud-platform',
    'https://www.googleapis.com/auth/userinfo.email',
    'https://www.googleapis.com/auth/userinfo.profile',
    'https://www.googleapis.com/auth/cclog',
    'https://www.googleapis.com/auth/experimentsandconfigs',
].join(' ');
function now() {
    return Date.now();
}
function emptyQuota() {
    return {
        geminiFiveHour: {},
        geminiWeekly: {},
        thirdPartyFiveHour: {},
        thirdPartyWeekly: {},
    };
}
function normalize(value) {
    return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}
function randomToken() {
    return crypto.randomBytes(32).toString('base64url');
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
function numberValue(value) {
    if (typeof value === 'number' && Number.isFinite(value))
        return value;
    if (typeof value === 'string' && value.trim().length > 0) {
        const parsed = Number(value);
        return Number.isFinite(parsed) ? parsed : undefined;
    }
    return undefined;
}
function readPath(root, path) {
    let current = root;
    for (const key of path) {
        if (current == null || typeof current !== 'object')
            return undefined;
        current = current[key];
    }
    return current;
}
function readString(root, path) {
    return normalize(readPath(root, path));
}
function readArray(root, path) {
    const value = readPath(root, path);
    return Array.isArray(value) ? value : [];
}
function firstString(values) {
    return values.find((value) => value != null);
}
function decodeJwtPayload(token) {
    if (!token)
        return undefined;
    const payload = token.split('.')[1];
    if (!payload)
        return undefined;
    try {
        return JSON.parse(Buffer.from(payload, 'base64url').toString('utf8'));
    }
    catch {
        return undefined;
    }
}
function buildAccountId(email, authId) {
    const seed = `${email.trim().toLowerCase()}:${authId?.trim() ?? ''}`;
    return `antigravity_${crypto.createHash('md5').update(seed).digest('hex')}`;
}
function platformName() {
    const platform = os.platform();
    const arch = os.arch();
    if (platform === 'darwin' && arch === 'x64')
        return 'DARWIN_AMD64';
    if (platform === 'darwin' && arch === 'arm64')
        return 'DARWIN_ARM64';
    if (platform === 'linux' && arch === 'x64')
        return 'LINUX_AMD64';
    if (platform === 'linux' && arch === 'arm64')
        return 'LINUX_ARM64';
    if (platform === 'win32' && arch === 'x64')
        return 'WINDOWS_AMD64';
    return 'PLATFORM_UNSPECIFIED';
}
function userAgentOs() {
    switch (os.platform()) {
        case 'darwin':
            return 'darwin';
        case 'linux':
            return 'linux';
        case 'win32':
            return 'windows';
        default:
            return 'windows';
    }
}
function userAgentArch() {
    return os.arch() === 'arm64' ? 'arm64' : 'amd64';
}
function codeAssistUrl(path) {
    return `${CODE_ASSIST_BASE_ENDPOINT}/${path}`;
}
function codeAssistUserAgent(endpoint) {
    const base = `antigravity/${ANTIGRAVITY_IDE_VERSION} ${userAgentOs()}/${userAgentArch()}`;
    if (endpoint.includes(CODE_ASSIST_LOAD_ENDPOINT)) {
        return `${base} google-api-nodejs-client/${ANTIGRAVITY_GOOGLE_API_NODEJS_CLIENT_VERSION}`;
    }
    return base;
}
function buildCodeAssistHeaders(endpoint, accessToken) {
    return {
        Authorization: `Bearer ${accessToken}`,
        'Content-Type': 'application/json',
        'User-Agent': codeAssistUserAgent(endpoint),
        'x-goog-api-client': ANTIGRAVITY_X_GOOG_API_CLIENT,
        Accept: '*/*',
    };
}
function loadCodeAssistPayload() {
    return {
        mode: 'FULL_ELIGIBILITY_CHECK',
        metadata: {
            ideName: 'antigravity',
            ideType: 'ANTIGRAVITY',
            ideVersion: ANTIGRAVITY_IDE_VERSION,
            pluginVersion: 'quota-vscode',
            platform: platformName(),
            updateChannel: 'stable',
            pluginType: 'GEMINI',
        },
    };
}
function parseTierPlanName(tierId) {
    const lower = tierId?.trim().toLowerCase();
    if (!lower)
        return undefined;
    if (lower.includes('ultra'))
        return 'Ultra';
    if (lower.includes('pro') || lower.includes('premium'))
        return 'Pro';
    if (lower.includes('free') || lower === 'standard-tier')
        return 'Free';
    return tierId;
}
function resetAt(value) {
    const numeric = numberValue(value);
    if (numeric != null) {
        if (numeric <= 0)
            return undefined;
        return numeric > 10_000_000_000 ? numeric : numeric * 1000;
    }
    if (typeof value === 'string' && value.trim().length > 0) {
        const parsed = Date.parse(value);
        return Number.isNaN(parsed) ? undefined : parsed;
    }
    return undefined;
}
function parseQuotaWindow(bucket) {
    const remainingFraction = numberValue(readPath(bucket, ['remainingFraction']));
    return {
        remainingPercent: remainingFraction == null ? undefined : clampPercent(remainingFraction * 100),
        resetAt: resetAt(readPath(bucket, ['resetTime'])),
    };
}
function quotaFromValue(raw) {
    const quota = emptyQuota();
    for (const group of readArray(raw, ['groups'])) {
        for (const bucket of readArray(group, ['buckets'])) {
            const bucketId = readString(bucket, ['bucketId']);
            const window = parseQuotaWindow(bucket);
            if (bucketId === 'gemini-5h')
                quota.geminiFiveHour = window;
            else if (bucketId === 'gemini-weekly')
                quota.geminiWeekly = window;
            else if (bucketId === '3p-5h')
                quota.thirdPartyFiveHour = window;
            else if (bucketId === '3p-weekly')
                quota.thirdPartyWeekly = window;
        }
    }
    return quota;
}
function isForbiddenError(error) {
    const lower = error.toLowerCase();
    return lower.includes('status=403')
        || lower.includes('403 forbidden')
        || lower.includes('permission_denied')
        || lower.includes('caller does not have permission');
}
async function parseJsonResponse(response, failureLabel) {
    const body = await response.text();
    if (!response.ok) {
        throw new Error(`${failureLabel} returned ${response.status} with body length ${body.length}.`);
    }
    if (!body.trim())
        return {};
    return JSON.parse(body);
}
async function postCodeAssist(accessToken, endpoint, payload) {
    const response = await fetch(endpoint, {
        method: 'POST',
        headers: buildCodeAssistHeaders(endpoint, accessToken),
        body: JSON.stringify(payload),
    });
    return parseJsonResponse(response, 'Antigravity quota request');
}
async function loadCodeAssistStatus(accessToken) {
    const raw = await postCodeAssist(accessToken, codeAssistUrl(CODE_ASSIST_LOAD_ENDPOINT), loadCodeAssistPayload());
    return (0, antigravityUsage_1.parseAntigravityLoadStatus)(raw);
}
async function retrieveUserQuota(accessToken, projectId) {
    const projectPayload = { project: projectId };
    await postCodeAssist(accessToken, codeAssistUrl(CODE_ASSIST_FETCH_MODELS_ENDPOINT), projectPayload);
    return postCodeAssist(accessToken, codeAssistUrl(CODE_ASSIST_RETRIEVE_QUOTA_ENDPOINT), projectPayload);
}
async function fetchGoogleUserInfo(accessToken) {
    const response = await fetch(GOOGLE_USERINFO_ENDPOINT, {
        headers: { Authorization: `Bearer ${accessToken}` },
    });
    if (!response.ok)
        return undefined;
    return response.json();
}
async function exchangeOAuthCode(session, code) {
    const response = await fetch(GOOGLE_TOKEN_ENDPOINT, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
            code,
            client_id: ANTIGRAVITY_OAUTH_CLIENT_ID,
            client_secret: ANTIGRAVITY_OAUTH_CLIENT_SECRET,
            redirect_uri: session.redirectUri,
            grant_type: 'authorization_code',
        }).toString(),
    });
    return parseJsonResponse(response, 'Antigravity OAuth token exchange');
}
async function refreshToken(refreshToken) {
    const response = await fetch(GOOGLE_TOKEN_ENDPOINT, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
            client_id: ANTIGRAVITY_OAUTH_CLIENT_ID,
            client_secret: ANTIGRAVITY_OAUTH_CLIENT_SECRET,
            refresh_token: refreshToken,
            grant_type: 'refresh_token',
        }).toString(),
    });
    return parseJsonResponse(response, 'Antigravity token refresh');
}
function startOAuthSession() {
    return new Promise((resolve, reject) => {
        const state = randomToken();
        let timeout;
        const callback = new Promise((callbackResolve, callbackReject) => {
            const server = http.createServer((request, response) => {
                const address = server.address();
                const port = typeof address === 'object' && address ? address.port : 0;
                const redirectUri = `http://127.0.0.1:${port}${ANTIGRAVITY_OAUTH_CALLBACK_PATH}`;
                const url = new URL(request.url ?? '/', redirectUri);
                if (url.pathname === '/cancel') {
                    response.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8' });
                    response.end('Antigravity connection cancelled.');
                    callbackReject(new Error('Antigravity OAuth login was cancelled.'));
                    return;
                }
                const code = normalize(url.searchParams.get('code'));
                const returnedState = normalize(url.searchParams.get('state'));
                const error = normalize(url.searchParams.get('error'));
                if (url.pathname !== ANTIGRAVITY_OAUTH_CALLBACK_PATH || error || !code || returnedState !== state) {
                    response.writeHead(400, { 'Content-Type': 'text/plain; charset=utf-8' });
                    response.end('Antigravity connection failed. Return to VS Code and try again.');
                    callbackReject(new Error(error ? `Antigravity OAuth error: ${error}` : 'Antigravity OAuth callback was invalid.'));
                    return;
                }
                response.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8' });
                response.end('Antigravity connected. You can return to VS Code.');
                callbackResolve({ code, state: returnedState });
            });
            server.on('error', (error) => {
                callbackReject(error);
                reject(error);
            });
            server.listen(0, '127.0.0.1', () => {
                const address = server.address();
                const port = typeof address === 'object' && address ? address.port : 0;
                const redirectUri = `http://127.0.0.1:${port}${ANTIGRAVITY_OAUTH_CALLBACK_PATH}`;
                const authUrl = `${ANTIGRAVITY_OAUTH_AUTHORIZE_ENDPOINT}?${new URLSearchParams({
                    response_type: 'code',
                    client_id: ANTIGRAVITY_OAUTH_CLIENT_ID,
                    redirect_uri: redirectUri,
                    access_type: 'offline',
                    scope: ANTIGRAVITY_SCOPES,
                    state,
                    prompt: 'consent',
                }).toString()}`;
                timeout = setTimeout(() => callbackReject(new Error('Antigravity OAuth login timed out.')), ANTIGRAVITY_OAUTH_TIMEOUT_MS);
                resolve({ server, authUrl, redirectUri, state, callback });
            });
        });
        callback.finally(() => {
            if (timeout)
                clearTimeout(timeout);
        }).catch(() => undefined);
    });
}
function accountFromTokenResponse(response, profile, existing) {
    if (response.error) {
        throw new Error(response.error_description ?? String(response.error));
    }
    const accessToken = normalize(response.access_token);
    if (!accessToken)
        throw new Error('Antigravity token response did not include an access token.');
    const idToken = normalize(response.id_token);
    const jwt = decodeJwtPayload(idToken);
    const email = normalize(profile?.email) ?? normalize(jwt?.email) ?? existing?.email ?? 'unknown@gmail.com';
    const authId = normalize(profile?.id) ?? normalize(jwt?.sub) ?? existing?.authId;
    const name = normalize(profile?.name) ?? normalize(jwt?.name) ?? existing?.name;
    const nowMs = now();
    return {
        account: {
            id: buildAccountId(email, authId),
            email,
            source: 'oauth',
            authId,
            name,
            selectedAuthType: 'oauth-personal',
            projectId: existing?.projectId,
            tierId: existing?.tierId,
            planName: existing?.planName,
            credits: existing?.credits ?? [],
            quota: existing?.quota ?? emptyQuota(),
            quotaQueryLastError: existing?.quotaQueryLastError ?? null,
            quotaQueryLastErrorAt: existing?.quotaQueryLastErrorAt ?? null,
            usageUpdatedAt: existing?.usageUpdatedAt ?? null,
            status: existing?.status ?? null,
            statusReason: existing?.statusReason ?? null,
            createdAt: existing?.createdAt ?? nowMs,
            lastUsed: nowMs,
        },
        credential: {
            id: buildAccountId(email, authId),
            accessToken,
            refreshToken: normalize(response.refresh_token),
            idToken,
            tokenType: normalize(response.token_type),
            scope: normalize(response.scope),
            expiryDate: response.expires_in == null ? undefined : nowMs + response.expires_in * 1000,
        },
    };
}
function trackFromAccount(account, id, label, window) {
    return {
        id,
        providerId: 'antigravity',
        providerLabel: constants_1.PROVIDER_LABELS.antigravity,
        label,
        accountLabel: account.name ? `${account.name} (${account.email})` : account.email,
        percentUsed: usedFromRemaining(window.remainingPercent),
        percentRemaining: window.remainingPercent ?? undefined,
        resetAt: window.resetAt,
        updatedAt: account.usageUpdatedAt,
        error: account.quotaQueryLastError ?? null,
    };
}
class AntigravityProvider {
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
            const accessToken = normalize(tokenResponse.access_token);
            const profile = accessToken ? await fetchGoogleUserInfo(accessToken) : undefined;
            const account = await this.upsertTokenResponse(tokenResponse, profile);
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
        for (const account of accounts)
            refreshed.push(await this.refreshAccount(account.id));
        return refreshed;
    }
    async refreshAccount(accountId) {
        const account = await this.getAccount(accountId);
        const credential = await this.getCredential(accountId);
        if (!account || !credential)
            throw new Error('Antigravity account is not connected.');
        try {
            const validCredential = await this.ensureAccessToken(account, credential);
            const loadStatus = await loadCodeAssistStatus(validCredential.accessToken);
            const profile = await fetchGoogleUserInfo(validCredential.accessToken);
            const nextAccount = {
                ...account,
                email: normalize(profile?.email) ?? account.email,
                authId: account.authId ?? normalize(profile?.id),
                name: account.name ?? normalize(profile?.name),
                projectId: loadStatus.projectId,
                tierId: loadStatus.tierId,
                planName: loadStatus.tierName ?? parseTierPlanName(loadStatus.tierId) ?? account.planName,
                credits: loadStatus.credits,
                lastUsed: now(),
            };
            if (!nextAccount.projectId) {
                throw new Error('Antigravity Cloud Code did not return a project id.');
            }
            const rawQuota = await retrieveUserQuota(validCredential.accessToken, nextAccount.projectId);
            return await this.saveAccount({
                ...nextAccount,
                quota: quotaFromValue(rawQuota),
                quotaQueryLastError: null,
                quotaQueryLastErrorAt: null,
                usageUpdatedAt: now(),
                status: null,
                statusReason: null,
            });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            return await this.saveAccount({
                ...account,
                quotaQueryLastError: message,
                quotaQueryLastErrorAt: now(),
                status: isForbiddenError(message) ? 'forbidden' : account.status,
                statusReason: isForbiddenError(message) ? message : account.statusReason,
                lastUsed: now(),
            });
        }
    }
    async disconnect() {
        const accounts = await this.getAccounts();
        if (accounts.length === 0) {
            void vscode.window.showInformationMessage('No Antigravity accounts are connected.');
            return;
        }
        const picked = await vscode.window.showQuickPick(accounts.map((account) => ({
            label: account.email,
            description: account.planName ?? account.tierId ?? 'Antigravity account',
            account,
        })), { title: 'Disconnect Antigravity' });
        if (!picked)
            return;
        const confirmed = await vscode.window.showWarningMessage(`Disconnect Antigravity account ${picked.account.email}? Extension-stored tokens and cached quota data will be deleted.`, { modal: true }, 'Disconnect');
        if (confirmed !== 'Disconnect')
            return;
        const store = await this.getCredentialStore();
        delete store.accounts[picked.account.id];
        await this.saveCredentialStore(store);
        await this.context.globalState.update(ANTIGRAVITY_ACCOUNTS_KEY, accounts.filter((account) => account.id !== picked.account.id));
    }
    async getTracks() {
        const accounts = await this.getAccounts();
        return accounts.flatMap((account) => [
            trackFromAccount(account, 'antigravity.gemini', 'Gemini Models', account.quota.geminiFiveHour),
            trackFromAccount(account, 'antigravity.geminiWeekly', 'Gemini Models weekly', account.quota.geminiWeekly),
            trackFromAccount(account, 'antigravity.claude', 'Claude/GPT models', account.quota.thirdPartyFiveHour),
            trackFromAccount(account, 'antigravity.claudeWeekly', 'Claude/GPT models weekly', account.quota.thirdPartyWeekly),
            (0, antigravityUsage_1.buildAntigravityCreditsTrack)(account),
        ]).filter((track) => (track != null
            && (track.percentUsed != null || track.percentRemaining != null || track.valueLabel != null || track.error != null)));
    }
    async hasAccounts() {
        return (await this.getAccounts()).length > 0;
    }
    async ensureAccessToken(account, credential) {
        const shouldRefresh = credential.expiryDate != null && credential.expiryDate <= now() + 60_000;
        if (!shouldRefresh)
            return credential;
        if (!credential.refreshToken)
            throw new Error('Antigravity refresh token is missing.');
        const response = await refreshToken(credential.refreshToken);
        const updated = await this.upsertTokenResponse(response, undefined, account);
        const nextCredential = await this.getCredential(updated.id);
        if (!nextCredential)
            throw new Error('Antigravity token refresh did not persist credentials.');
        return nextCredential;
    }
    async upsertTokenResponse(response, profile, existing) {
        const result = accountFromTokenResponse(response, profile, existing);
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
        return this.context.globalState.get(ANTIGRAVITY_ACCOUNTS_KEY, []);
    }
    async getAccount(accountId) {
        return (await this.getAccounts()).find((account) => account.id === accountId);
    }
    async saveAccount(account) {
        const accounts = await this.getAccounts();
        const next = [account, ...accounts.filter((item) => item.id !== account.id)];
        await this.context.globalState.update(ANTIGRAVITY_ACCOUNTS_KEY, next);
        return account;
    }
    async getCredential(accountId) {
        const store = await this.getCredentialStore();
        return store.accounts[accountId];
    }
    async getCredentialStore() {
        const raw = await this.context.secrets.get(ANTIGRAVITY_SECRET_KEY);
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
        await this.context.secrets.store(ANTIGRAVITY_SECRET_KEY, JSON.stringify(store));
    }
}
exports.AntigravityProvider = AntigravityProvider;
//# sourceMappingURL=antigravityProvider.js.map