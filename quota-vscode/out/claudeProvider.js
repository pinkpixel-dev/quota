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
exports.ClaudeProvider = void 0;
// @env node
const crypto = __importStar(require("node:crypto"));
const vscode = __importStar(require("vscode"));
const claudeUsage_1 = require("./claudeUsage");
const constants_1 = require("./constants");
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
function firstString(values) {
    return values.find((value) => value != null);
}
function buildAccountId(email, accountUuid, organizationUuid) {
    const seed = [
        email.trim().toLowerCase(),
        accountUuid?.trim() ?? '',
        organizationUuid?.trim() ?? '',
    ].join(':');
    return `claude_${crypto.createHash('md5').update(seed).digest('hex')}`;
}
function subscriptionType(profile) {
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
function remainingPercentFromUsage(window) {
    const used = numberValue(readPath(window, ['utilization']));
    if (used == null)
        return undefined;
    return Math.min(100, Math.max(0, Math.round(100 - used)));
}
function resetAtFromUsage(window) {
    const value = readPath(window, ['resets_at']);
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
function quotaFromUsage(raw) {
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
function parseCallbackInput(input, expectedState) {
    const trimmed = input.trim();
    if (!trimmed)
        throw new Error('Claude callback URL or code is required.');
    const parseCode = (raw) => {
        const [beforeHash, hashState] = raw.split('#', 2);
        if (hashState && hashState !== expectedState)
            throw new Error('Claude OAuth callback state did not match.');
        return beforeHash.split('&', 1)[0].trim();
    };
    const url = (() => {
        try {
            return new URL(trimmed);
        }
        catch {
            return undefined;
        }
    })();
    if (url) {
        const code = normalize(url.searchParams.get('code'));
        const state = normalize(url.searchParams.get('state'));
        if (state && state !== expectedState)
            throw new Error('Claude OAuth callback state did not match.');
        if (code)
            return parseCode(code);
    }
    if (trimmed.startsWith('?')) {
        const params = new URLSearchParams(trimmed.slice(1));
        const code = normalize(params.get('code'));
        const state = normalize(params.get('state'));
        if (state && state !== expectedState)
            throw new Error('Claude OAuth callback state did not match.');
        if (code)
            return parseCode(code);
    }
    return parseCode(trimmed.replace(/^code=/, ''));
}
function buildOAuthStart() {
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
async function parseJsonResponse(response, failureLabel) {
    const body = await response.text();
    if (!response.ok) {
        throw new Error(`${failureLabel} returned ${response.status} with body length ${body.length}.`);
    }
    return JSON.parse(body);
}
async function exchangeOAuthCode(start, code) {
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
    return parseJsonResponse(response, 'Claude OAuth token exchange');
}
async function refreshToken(refreshToken) {
    const response = await fetch(CLAUDE_OAUTH_TOKEN_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            grant_type: 'refresh_token',
            refresh_token: refreshToken,
            client_id: CLAUDE_OAUTH_CLIENT_ID,
        }),
    });
    return parseJsonResponse(response, 'Claude token refresh');
}
async function requestProfile(accessToken) {
    const response = await fetch(CLAUDE_OAUTH_PROFILE_URL, {
        headers: {
            Authorization: `Bearer ${accessToken}`,
            'Content-Type': 'application/json',
        },
    });
    return parseJsonResponse(response, 'Claude OAuth profile');
}
async function requestUsage(accessToken) {
    const response = await fetch(CLAUDE_OAUTH_USAGE_URL, {
        headers: {
            Authorization: `Bearer ${accessToken}`,
            'anthropic-beta': CLAUDE_OAUTH_BETA_HEADER,
        },
    });
    return parseJsonResponse(response, 'Claude usage');
}
function accountFromTokenResponse(response, profile, existing) {
    if (response.error) {
        throw new Error(response.error_description ?? String(response.error));
    }
    const accessToken = normalize(response.access_token);
    if (!accessToken)
        throw new Error('Claude token response did not include an access token.');
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
    if (!email)
        throw new Error('Claude OAuth response did not include an email.');
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
function trackFromAccount(account, id, label, remaining, resetAt) {
    return {
        id,
        providerId: 'claude',
        providerLabel: constants_1.PROVIDER_LABELS.claude,
        label,
        accountLabel: account.displayName ? `${account.displayName} (${account.email})` : account.email,
        percentUsed: usedFromRemaining(remaining),
        percentRemaining: remaining ?? undefined,
        resetAt,
        updatedAt: account.usageUpdatedAt,
        error: account.quotaQueryLastError ?? null,
    };
}
class ClaudeProvider {
    context;
    constructor(context) {
        this.context = context;
    }
    async connect() {
        const start = buildOAuthStart();
        await vscode.env.openExternal(vscode.Uri.parse(start.authUrl));
        const callbackOrCode = await vscode.window.showInputBox({
            title: 'Connect Claude Code',
            prompt: 'Paste the Claude callback URL or authorization code after approving access in the browser.',
            ignoreFocusOut: true,
            password: true,
            placeHolder: 'https://platform.claude.com/oauth/code/callback?code=...',
        });
        if (!callbackOrCode)
            throw new Error('Claude OAuth login was cancelled.');
        const code = parseCallbackInput(callbackOrCode, start.state);
        const tokenResponse = await exchangeOAuthCode(start, code);
        const accessToken = normalize(tokenResponse.access_token);
        if (!accessToken)
            throw new Error('Claude token response did not include an access token.');
        const profile = await requestProfile(accessToken);
        const account = await this.upsertTokenResponse(tokenResponse, profile);
        await this.refreshAccount(account.id);
        return (await this.getAccount(account.id)) ?? account;
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
            throw new Error('Claude account is not connected.');
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
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            if ((0, claudeUsage_1.shouldSuppressClaudeError)(message)) {
                return await this.saveAccount({
                    ...account,
                    quotaQueryLastError: null,
                    quotaQueryLastErrorAt: null,
                    lastUsed: now(),
                });
            }
            return await this.saveAccount({
                ...account,
                quotaQueryLastError: message,
                quotaQueryLastErrorAt: now(),
                lastUsed: now(),
            });
        }
    }
    async disconnect() {
        const accounts = await this.getAccounts();
        if (accounts.length === 0) {
            void vscode.window.showInformationMessage('No Claude Code accounts are connected.');
            return;
        }
        const picked = await vscode.window.showQuickPick(accounts.map((account) => ({
            label: account.email,
            description: account.planType ?? account.organizationName ?? 'Claude Code account',
            account,
        })), { title: 'Disconnect Claude Code' });
        if (!picked)
            return;
        const confirmed = await vscode.window.showWarningMessage(`Disconnect Claude Code account ${picked.account.email}? Extension-stored tokens and cached quota data will be deleted.`, { modal: true }, 'Disconnect');
        if (confirmed !== 'Disconnect')
            return;
        const store = await this.getCredentialStore();
        delete store.accounts[picked.account.id];
        await this.saveCredentialStore(store);
        await this.context.globalState.update(CLAUDE_ACCOUNTS_KEY, accounts.filter((account) => account.id !== picked.account.id));
    }
    async getTracks() {
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
    async hasAccounts() {
        return (await this.getAccounts()).length > 0;
    }
    async ensureAccessToken(account, credential) {
        const shouldRefresh = credential.expiresAt != null && credential.expiresAt <= now() + 300_000;
        if (!shouldRefresh)
            return credential;
        if (!credential.refreshToken)
            throw new Error('Claude refresh token is missing.');
        const tokenResponse = await refreshToken(credential.refreshToken);
        const updated = await this.upsertTokenResponse(tokenResponse, undefined, account);
        const nextCredential = await this.getCredential(updated.id);
        if (!nextCredential)
            throw new Error('Claude token refresh did not persist credentials.');
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
        return this.context.globalState.get(CLAUDE_ACCOUNTS_KEY, []);
    }
    async getAccount(accountId) {
        return (await this.getAccounts()).find((account) => account.id === accountId);
    }
    async saveAccount(account) {
        const accounts = await this.getAccounts();
        const next = [account, ...accounts.filter((item) => item.id !== account.id)];
        await this.context.globalState.update(CLAUDE_ACCOUNTS_KEY, next);
        return account;
    }
    async getCredential(accountId) {
        const store = await this.getCredentialStore();
        return store.accounts[accountId];
    }
    async getCredentialStore() {
        const raw = await this.context.secrets.get(CLAUDE_SECRET_KEY);
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
        await this.context.secrets.store(CLAUDE_SECRET_KEY, JSON.stringify(store));
    }
}
exports.ClaudeProvider = ClaudeProvider;
//# sourceMappingURL=claudeProvider.js.map