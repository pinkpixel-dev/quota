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
exports.OpenCodeGoProvider = void 0;
// @env node
const crypto = __importStar(require("node:crypto"));
const vscode = __importStar(require("vscode"));
const constants_1 = require("./constants");
const opencodeGoUsage_1 = require("./opencodeGoUsage");
const OPENCODE_GO_USAGE_URL = 'https://opencode.ai/zen/go/v1/usage';
const OPENCODE_GO_SECRET_KEY = 'quota.opencodeGo.credentials';
const OPENCODE_GO_ACCOUNTS_KEY = 'quota.opencodeGo.accounts';
const REQUEST_TIMEOUT_MS = 15_000;
function accountIdForKey(apiKey) {
    return crypto.createHash('sha256').update(apiKey).digest('hex').slice(0, 16);
}
function track(account, id, label, window) {
    return {
        id,
        providerId: 'opencodeGo',
        providerLabel: constants_1.PROVIDER_LABELS.opencodeGo,
        label,
        accountLabel: account.label,
        percentUsed: window?.percentUsed,
        percentRemaining: window?.percentUsed == null ? undefined : 100 - window.percentUsed,
        resetAt: window?.resetAt ?? null,
        resetLabel: window?.startsOnFirstUse ? 'Starts on first use' : undefined,
        updatedAt: account.usageUpdatedAt,
        error: account.quotaQueryLastError ?? null,
    };
}
function tracksFromAccount(account) {
    return [
        track(account, 'opencodeGo.fiveHour', '5h usage', account.usage.rolling),
        track(account, 'opencodeGo.weekly', 'Weekly usage', account.usage.weekly),
        track(account, 'opencodeGo.monthly', 'Monthly usage', account.usage.monthly),
    ];
}
class OpenCodeGoProvider {
    context;
    constructor(context) {
        this.context = context;
    }
    async connect() {
        const apiKey = (await vscode.window.showInputBox({
            title: 'Connect OpenCode Go',
            prompt: 'Paste your OpenCode Go API key from the OpenCode console.',
            password: true,
            ignoreFocusOut: true,
            validateInput: (value) => (value.trim() ? undefined : 'Enter an API key.'),
        }))?.trim();
        if (!apiKey)
            return undefined;
        const name = await vscode.window.showInputBox({
            title: 'Connect OpenCode Go',
            prompt: 'Optional name for this account. Leave blank to show the last four characters of the key.',
            ignoreFocusOut: true,
        });
        if (name === undefined)
            return undefined;
        // Fails before anything is saved, so a mistyped key never becomes an account.
        const usage = await this.fetchUsage(apiKey);
        const id = accountIdForKey(apiKey);
        const existing = await this.getAccount(id);
        const timestamp = Date.now();
        const store = await this.getCredentialStore();
        store.accounts[id] = apiKey;
        await this.saveCredentialStore(store);
        return this.saveAccount({
            id,
            label: name.trim() || (0, opencodeGoUsage_1.maskOpenCodeGoKey)(apiKey),
            usage,
            quotaQueryLastError: null,
            quotaQueryLastErrorAt: null,
            usageUpdatedAt: timestamp,
            createdAt: existing?.createdAt ?? timestamp,
            lastUsed: timestamp,
        });
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
        const apiKey = (await this.getCredentialStore()).accounts[accountId];
        if (!account || !apiKey)
            throw new Error('OpenCode Go account is not connected.');
        try {
            const usage = await this.fetchUsage(apiKey);
            const timestamp = Date.now();
            return await this.saveAccount({
                ...account,
                usage,
                quotaQueryLastError: null,
                quotaQueryLastErrorAt: null,
                usageUpdatedAt: timestamp,
                lastUsed: timestamp,
            });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            return this.saveAccount({ ...account, quotaQueryLastError: message, quotaQueryLastErrorAt: Date.now() });
        }
    }
    async disconnect() {
        const accounts = await this.getAccounts();
        if (accounts.length === 0) {
            void vscode.window.showInformationMessage('No OpenCode Go accounts are connected.');
            return;
        }
        const picked = await vscode.window.showQuickPick(accounts.map((account) => ({ label: account.label, account })), { title: 'Disconnect OpenCode Go' });
        if (!picked)
            return;
        const confirmed = await vscode.window.showWarningMessage(`Disconnect OpenCode Go account ${picked.account.label}? The stored API key and cached quota data will be deleted.`, { modal: true }, 'Disconnect');
        if (confirmed !== 'Disconnect')
            return;
        const store = await this.getCredentialStore();
        delete store.accounts[picked.account.id];
        await this.saveCredentialStore(store);
        await this.context.globalState.update(OPENCODE_GO_ACCOUNTS_KEY, accounts.filter((account) => account.id !== picked.account.id));
    }
    async getTracks() {
        const accounts = await this.getAccounts();
        return accounts
            .flatMap(tracksFromAccount)
            .filter((item) => item.percentUsed != null || item.error);
    }
    async hasAccounts() {
        return (await this.getAccounts()).length > 0;
    }
    async fetchUsage(apiKey) {
        const version = String(this.context.extension.packageJSON.version ?? '0.0.0');
        let response;
        try {
            response = await fetch(OPENCODE_GO_USAGE_URL, {
                headers: {
                    Authorization: `Bearer ${apiKey}`,
                    Accept: 'application/json',
                    'User-Agent': `quota-vscode/${version}`,
                },
                // Never forward the key to a redirect target.
                redirect: 'error',
                signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
            });
        }
        catch {
            throw new Error('Could not reach OpenCode Go. Check your connection and try again.');
        }
        if (!response.ok)
            throw new Error((0, opencodeGoUsage_1.openCodeGoErrorMessage)(response.status));
        let body;
        try {
            body = await response.json();
        }
        catch {
            throw new Error('OpenCode Go usage response was not valid JSON.');
        }
        return (0, opencodeGoUsage_1.parseOpenCodeGoUsage)(body);
    }
    async getAccounts() {
        return this.context.globalState.get(OPENCODE_GO_ACCOUNTS_KEY, []);
    }
    async getAccount(accountId) {
        return (await this.getAccounts()).find((account) => account.id === accountId);
    }
    async saveAccount(account) {
        const accounts = await this.getAccounts();
        const next = [account, ...accounts.filter((item) => item.id !== account.id)];
        await this.context.globalState.update(OPENCODE_GO_ACCOUNTS_KEY, next);
        return account;
    }
    async getCredentialStore() {
        const raw = await this.context.secrets.get(OPENCODE_GO_SECRET_KEY);
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
        await this.context.secrets.store(OPENCODE_GO_SECRET_KEY, JSON.stringify(store));
    }
}
exports.OpenCodeGoProvider = OpenCodeGoProvider;
//# sourceMappingURL=opencodeGoProvider.js.map