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
exports.GitHubCopilotProvider = void 0;
// @env node
const crypto = __importStar(require("node:crypto"));
const vscode = __importStar(require("vscode"));
const constants_1 = require("./constants");
const githubCopilotUsage_1 = require("./githubCopilotUsage");
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
function now() {
    return Date.now();
}
function normalize(value) {
    return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}
function clampPercent(value) {
    if (value == null || !Number.isFinite(value))
        return undefined;
    return Math.min(100, Math.max(0, Math.round(value)));
}
async function parseJsonResponse(response, failureLabel) {
    const body = await response.text();
    if (!response.ok)
        throw new Error(`${failureLabel} returned ${response.status} with body length ${body.length}.`);
    return JSON.parse(body);
}
async function requestDeviceCode() {
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
    return parseJsonResponse(response, 'GitHub device code request');
}
async function exchangeDeviceToken(deviceCode) {
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
    return parseJsonResponse(response, 'GitHub access token request');
}
function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}
async function pollDeviceToken(deviceCode, intervalSeconds, expiresInSeconds) {
    const expiresAt = now() + expiresInSeconds * 1000;
    let waitSeconds = Math.max(1, intervalSeconds);
    while (now() < expiresAt) {
        const response = await exchangeDeviceToken(deviceCode);
        if (!response.error)
            return response;
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
            throw new Error('GitHub authorization expired. Start again.');
        if (response.error === 'access_denied')
            throw new Error('GitHub authorization was denied.');
        throw new Error(response.error_description ?? `GitHub authorization failed: ${response.error}`);
    }
    throw new Error('GitHub authorization expired. Start again.');
}
async function fetchGitHubUser(githubAccessToken) {
    const response = await fetch(GITHUB_USER_ENDPOINT, {
        headers: {
            Accept: 'application/vnd.github+json',
            Authorization: `Bearer ${githubAccessToken}`,
            'User-Agent': APP_USER_AGENT,
        },
    });
    return parseJsonResponse(response, 'GitHub user request');
}
async function fetchGitHubEmail(githubAccessToken) {
    const response = await fetch(GITHUB_USER_EMAILS_ENDPOINT, {
        headers: {
            Accept: 'application/vnd.github+json',
            Authorization: `Bearer ${githubAccessToken}`,
            'User-Agent': APP_USER_AGENT,
        },
    });
    const emails = await parseJsonResponse(response, 'GitHub email request');
    return emails.find((item) => item.primary && item.verified)?.email
        ?? emails.find((item) => item.verified)?.email;
}
async function fetchCopilotUserInfo(githubAccessToken) {
    const response = await fetch(GITHUB_COPILOT_USER_INFO_ENDPOINT, {
        headers: {
            Accept: 'application/json',
            Authorization: `token ${githubAccessToken}`,
            'User-Agent': APP_USER_AGENT,
            'X-GitHub-Api-Version': '2025-04-01',
        },
    });
    if (!response.ok)
        return undefined;
    return response.json();
}
async function fetchCopilotToken(githubAccessToken) {
    const response = await fetch(GITHUB_COPILOT_TOKEN_ENDPOINT, {
        headers: {
            Accept: 'application/json',
            Authorization: `token ${githubAccessToken}`,
            'User-Agent': APP_USER_AGENT,
            'X-GitHub-Api-Version': '2025-04-01',
        },
    });
    const payload = await parseJsonResponse(response, 'Copilot token request');
    const token = normalize(payload.token);
    if (!token)
        throw new Error(payload.message ?? 'Copilot token missing.');
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
async function buildPayloadFromGitHubAccessToken(githubAccessToken, githubTokenType, githubScope) {
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
function accountId(payload) {
    return `ghcp_${crypto.createHash('md5').update(`${payload.githubLogin}:${payload.githubId}`).digest('hex')}`;
}
function accountLabel(account) {
    return account.githubEmail ?? account.githubLogin;
}
function accountFromPayload(payload, existing) {
    const id = existing?.id ?? accountId(payload);
    const usage = (0, githubCopilotUsage_1.buildCopilotUsageSummary)({
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
function trackFromAccount(account, id, label, percentUsed) {
    return {
        id,
        providerId: 'githubCopilot',
        providerLabel: constants_1.PROVIDER_LABELS.githubCopilot,
        label,
        accountLabel: accountLabel(account),
        percentUsed: clampPercent(percentUsed),
        percentRemaining: percentUsed == null ? undefined : 100 - clampPercent(percentUsed),
        resetAt: account.usage.allowanceResetAt,
        updatedAt: account.usageUpdatedAt,
        error: account.quotaQueryLastError ?? null,
    };
}
class GitHubCopilotProvider {
    context;
    constructor(context) {
        this.context = context;
    }
    async connect() {
        const device = await requestDeviceCode();
        const authUri = device.verification_uri_complete ?? device.verification_uri;
        await vscode.env.openExternal(vscode.Uri.parse(authUri));
        void vscode.window.showInformationMessage(`GitHub Copilot device code: ${device.user_code}`);
        const tokenResponse = await pollDeviceToken(device.device_code, device.interval ?? 5, device.expires_in);
        const accessToken = normalize(tokenResponse.access_token);
        if (!accessToken)
            throw new Error('GitHub access token missing.');
        const payload = await buildPayloadFromGitHubAccessToken(accessToken, tokenResponse.token_type, tokenResponse.scope);
        return this.upsertPayload(payload);
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
            throw new Error('GitHub Copilot account is not connected.');
        try {
            const copilot = await fetchCopilotToken(credential.githubAccessToken);
            const payload = {
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
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
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
            void vscode.window.showInformationMessage('No GitHub Copilot accounts are connected.');
            return;
        }
        const picked = await vscode.window.showQuickPick(accounts.map((account) => ({
            label: accountLabel(account),
            description: account.plan ?? 'GitHub Copilot account',
            account,
        })), { title: 'Disconnect GitHub Copilot' });
        if (!picked)
            return;
        const confirmed = await vscode.window.showWarningMessage(`Disconnect GitHub Copilot account ${accountLabel(picked.account)}? Extension-stored tokens and cached quota data will be deleted.`, { modal: true }, 'Disconnect');
        if (confirmed !== 'Disconnect')
            return;
        const store = await this.getCredentialStore();
        delete store.accounts[picked.account.id];
        await this.saveCredentialStore(store);
        await this.context.globalState.update(GITHUB_COPILOT_ACCOUNTS_KEY, accounts.filter((account) => account.id !== picked.account.id));
    }
    async getTracks() {
        const accounts = await this.getAccounts();
        return accounts.flatMap((account) => [
            trackFromAccount(account, 'githubCopilot.premium', 'Premium requests', account.usage.premiumRequestsUsedPercent),
            trackFromAccount(account, 'githubCopilot.chat', 'Chat messages', account.usage.chatMessagesUsedPercent),
            trackFromAccount(account, 'githubCopilot.inline', 'Inline suggestions', account.usage.inlineSuggestionsUsedPercent),
        ]).filter((track) => track.percentUsed != null || track.percentRemaining != null || track.error);
    }
    async hasAccounts() {
        return (await this.getAccounts()).length > 0;
    }
    async upsertPayload(payload, existing) {
        const result = accountFromPayload(payload, existing);
        const store = await this.getCredentialStore();
        store.accounts[result.account.id] = result.credential;
        await this.saveCredentialStore(store);
        return this.saveAccount(result.account);
    }
    async getAccounts() {
        return this.context.globalState.get(GITHUB_COPILOT_ACCOUNTS_KEY, []);
    }
    async getAccount(accountId) {
        return (await this.getAccounts()).find((account) => account.id === accountId);
    }
    async saveAccount(account) {
        const accounts = await this.getAccounts();
        const next = [account, ...accounts.filter((item) => item.id !== account.id)];
        await this.context.globalState.update(GITHUB_COPILOT_ACCOUNTS_KEY, next);
        return account;
    }
    async getCredential(accountId) {
        const store = await this.getCredentialStore();
        return store.accounts[accountId];
    }
    async getCredentialStore() {
        const raw = await this.context.secrets.get(GITHUB_COPILOT_SECRET_KEY);
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
        await this.context.secrets.store(GITHUB_COPILOT_SECRET_KEY, JSON.stringify(store));
    }
}
exports.GitHubCopilotProvider = GitHubCopilotProvider;
//# sourceMappingURL=githubCopilotProvider.js.map