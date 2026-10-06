// @env node
import * as crypto from 'node:crypto';

import * as vscode from 'vscode';

import { PROVIDER_LABELS } from './constants';
import {
  maskOpenCodeGoKey,
  openCodeGoErrorMessage,
  parseOpenCodeGoUsage,
  type OpenCodeGoUsageSummary,
  type OpenCodeGoWindow,
} from './opencodeGoUsage';
import type { QuotaTrack, TrackId } from './types';

const OPENCODE_GO_USAGE_URL = 'https://opencode.ai/zen/go/v1/usage';
const OPENCODE_GO_SECRET_KEY = 'quota.opencodeGo.credentials';
const OPENCODE_GO_ACCOUNTS_KEY = 'quota.opencodeGo.accounts';
const REQUEST_TIMEOUT_MS = 15_000;

interface OpenCodeGoCredentialStore {
  accounts: Record<string, string>;
}

interface OpenCodeGoAccount {
  id: string;
  label: string;
  usage: OpenCodeGoUsageSummary;
  quotaQueryLastError?: string | null;
  quotaQueryLastErrorAt?: number | null;
  usageUpdatedAt?: number | null;
  createdAt: number;
  lastUsed: number;
}

function accountIdForKey(apiKey: string): string {
  return crypto.createHash('sha256').update(apiKey).digest('hex').slice(0, 16);
}

function track(account: OpenCodeGoAccount, id: TrackId, label: string, window: OpenCodeGoWindow | undefined): QuotaTrack {
  return {
    id,
    providerId: 'opencodeGo',
    providerLabel: PROVIDER_LABELS.opencodeGo,
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

function tracksFromAccount(account: OpenCodeGoAccount): QuotaTrack[] {
  return [
    track(account, 'opencodeGo.fiveHour', '5h usage', account.usage.rolling),
    track(account, 'opencodeGo.weekly', 'Weekly usage', account.usage.weekly),
    track(account, 'opencodeGo.monthly', 'Monthly usage', account.usage.monthly),
  ];
}

export class OpenCodeGoProvider {
  constructor(private readonly context: vscode.ExtensionContext) {}

  async connect(): Promise<OpenCodeGoAccount | undefined> {
    const apiKey = (await vscode.window.showInputBox({
      title: 'Connect OpenCode Go',
      prompt: 'Paste your OpenCode Go API key from the OpenCode console.',
      password: true,
      ignoreFocusOut: true,
      validateInput: (value) => (value.trim() ? undefined : 'Enter an API key.'),
    }))?.trim();
    if (!apiKey) return undefined;

    const name = await vscode.window.showInputBox({
      title: 'Connect OpenCode Go',
      prompt: 'Optional name for this account. Leave blank to show the last four characters of the key.',
      ignoreFocusOut: true,
    });
    if (name === undefined) return undefined;

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
      label: name.trim() || maskOpenCodeGoKey(apiKey),
      usage,
      quotaQueryLastError: null,
      quotaQueryLastErrorAt: null,
      usageUpdatedAt: timestamp,
      createdAt: existing?.createdAt ?? timestamp,
      lastUsed: timestamp,
    });
  }

  async refreshAll(): Promise<OpenCodeGoAccount[]> {
    const accounts = await this.getAccounts();
    const refreshed: OpenCodeGoAccount[] = [];
    for (const account of accounts) refreshed.push(await this.refreshAccount(account.id));
    return refreshed;
  }

  async refreshAccount(accountId: string): Promise<OpenCodeGoAccount> {
    const account = await this.getAccount(accountId);
    const apiKey = (await this.getCredentialStore()).accounts[accountId];
    if (!account || !apiKey) throw new Error('OpenCode Go account is not connected.');

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
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return this.saveAccount({ ...account, quotaQueryLastError: message, quotaQueryLastErrorAt: Date.now() });
    }
  }

  async disconnect(): Promise<void> {
    const accounts = await this.getAccounts();
    if (accounts.length === 0) {
      void vscode.window.showInformationMessage('No OpenCode Go accounts are connected.');
      return;
    }

    const picked = await vscode.window.showQuickPick(
      accounts.map((account) => ({ label: account.label, account })),
      { title: 'Disconnect OpenCode Go' },
    );
    if (!picked) return;

    const confirmed = await vscode.window.showWarningMessage(
      `Disconnect OpenCode Go account ${picked.account.label}? The stored API key and cached quota data will be deleted.`,
      { modal: true },
      'Disconnect',
    );
    if (confirmed !== 'Disconnect') return;

    const store = await this.getCredentialStore();
    delete store.accounts[picked.account.id];
    await this.saveCredentialStore(store);
    await this.context.globalState.update(
      OPENCODE_GO_ACCOUNTS_KEY,
      accounts.filter((account) => account.id !== picked.account.id),
    );
  }

  async getTracks(): Promise<QuotaTrack[]> {
    const accounts = await this.getAccounts();
    return accounts
      .flatMap(tracksFromAccount)
      .filter((item) => item.percentUsed != null || item.error);
  }

  async hasAccounts(): Promise<boolean> {
    return (await this.getAccounts()).length > 0;
  }

  private async fetchUsage(apiKey: string): Promise<OpenCodeGoUsageSummary> {
    const version = String(this.context.extension.packageJSON.version ?? '0.0.0');
    let response: Response;
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
    } catch {
      throw new Error('Could not reach OpenCode Go. Check your connection and try again.');
    }

    if (!response.ok) throw new Error(openCodeGoErrorMessage(response.status));

    let body: unknown;
    try {
      body = await response.json();
    } catch {
      throw new Error('OpenCode Go usage response was not valid JSON.');
    }
    return parseOpenCodeGoUsage(body);
  }

  private async getAccounts(): Promise<OpenCodeGoAccount[]> {
    return this.context.globalState.get<OpenCodeGoAccount[]>(OPENCODE_GO_ACCOUNTS_KEY, []);
  }

  private async getAccount(accountId: string): Promise<OpenCodeGoAccount | undefined> {
    return (await this.getAccounts()).find((account) => account.id === accountId);
  }

  private async saveAccount(account: OpenCodeGoAccount): Promise<OpenCodeGoAccount> {
    const accounts = await this.getAccounts();
    const next = [account, ...accounts.filter((item) => item.id !== account.id)];
    await this.context.globalState.update(OPENCODE_GO_ACCOUNTS_KEY, next);
    return account;
  }

  private async getCredentialStore(): Promise<OpenCodeGoCredentialStore> {
    const raw = await this.context.secrets.get(OPENCODE_GO_SECRET_KEY);
    if (!raw) return { accounts: {} };

    try {
      const parsed = JSON.parse(raw) as OpenCodeGoCredentialStore;
      return parsed && typeof parsed === 'object' && parsed.accounts ? parsed : { accounts: {} };
    } catch {
      return { accounts: {} };
    }
  }

  private async saveCredentialStore(store: OpenCodeGoCredentialStore): Promise<void> {
    await this.context.secrets.store(OPENCODE_GO_SECRET_KEY, JSON.stringify(store));
  }
}
