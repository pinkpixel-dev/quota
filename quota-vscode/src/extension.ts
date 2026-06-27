// @env node
import * as vscode from 'vscode';

import { AntigravityProvider } from './antigravityProvider';
import { ClaudeProvider } from './claudeProvider';
import { CodexProvider } from './codexProvider';
import { readConfiguration } from './configuration';
import { GitHubCopilotProvider } from './githubCopilotProvider';
import { KiroProvider } from './kiroProvider';
import { showQuotaPanel } from './panel';
import { loadQuotaSnapshot } from './summary';
import { QuotaStatusBar } from './statusBar';
import type { QuotaConfiguration, QuotaSnapshot } from './types';

let config: QuotaConfiguration;
let snapshot: QuotaSnapshot;
let statusBar: QuotaStatusBar;
let githubCopilotProvider: GitHubCopilotProvider;
let codexProvider: CodexProvider;
let claudeProvider: ClaudeProvider;
let antigravityProvider: AntigravityProvider;
let kiroProvider: KiroProvider;
let refreshTimer: NodeJS.Timeout | undefined;
let lastManualRefreshAt = 0;

const MANUAL_REFRESH_COOLDOWN_MS = 10_000;

async function loadSnapshot(): Promise<QuotaSnapshot> {
  if (config.dataSource === 'desktopSummary') return loadQuotaSnapshot(config.summaryPath);

  const tracks = [
    ...(await githubCopilotProvider.getTracks()),
    ...(await codexProvider.getTracks()),
    ...(await claudeProvider.getTracks()),
    ...(await antigravityProvider.getTracks()),
    ...(await kiroProvider.getTracks()),
  ];
  return {
    sourcePath: 'VS Code extension accounts',
    tracks,
    warnings: tracks.length === 0 ? ['No extension-owned accounts are connected yet.'] : [],
  };
}

async function refresh(showToast = false, options: { refreshProviders?: boolean } = {}): Promise<void> {
  config = readConfiguration();
  if (config.dataSource === 'extensionAccounts' && options.refreshProviders) {
    if (await githubCopilotProvider.hasAccounts()) await githubCopilotProvider.refreshAll();
    if (await codexProvider.hasAccounts()) await codexProvider.refreshAll();
    if (await claudeProvider.hasAccounts()) await claudeProvider.refreshAll();
    if (await antigravityProvider.hasAccounts()) await antigravityProvider.refreshAll();
    if (await kiroProvider.hasAccounts()) await kiroProvider.refreshAll();
  }
  snapshot = await loadSnapshot();
  statusBar.update(snapshot, config);

  if (showToast) {
    const message = snapshot.tracks.length > 0
      ? `Quota refreshed ${snapshot.tracks.length} quota tracks.`
      : snapshot.warnings[0] ?? 'Quota refreshed, but no tracks were found.';
    void vscode.window.showInformationMessage(message);
  }
}

function checkManualRefreshCooldown(): boolean {
  const elapsed = Date.now() - lastManualRefreshAt;
  if (elapsed < MANUAL_REFRESH_COOLDOWN_MS) {
    const seconds = Math.ceil((MANUAL_REFRESH_COOLDOWN_MS - elapsed) / 1000);
    void vscode.window.showWarningMessage(`Please wait ${seconds}s before refreshing again.`);
    return false;
  }

  lastManualRefreshAt = Date.now();
  return true;
}

function scheduleAutoRefresh(): void {
  if (refreshTimer) {
    clearInterval(refreshTimer);
    refreshTimer = undefined;
  }

  if (config.dataSource !== 'extensionAccounts') return;

  const seconds = Math.min(3600, Math.max(30, config.refreshIntervalSeconds));
  refreshTimer = setInterval(() => {
    void refresh(false, { refreshProviders: true });
  }, seconds * 1000);

}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  statusBar = new QuotaStatusBar();
  githubCopilotProvider = new GitHubCopilotProvider(context);
  codexProvider = new CodexProvider(context);
  claudeProvider = new ClaudeProvider(context);
  antigravityProvider = new AntigravityProvider(context);
  kiroProvider = new KiroProvider(context);
  config = readConfiguration();
  snapshot = await loadSnapshot();
  statusBar.update(snapshot, config);
  scheduleAutoRefresh();

  context.subscriptions.push(
    statusBar,
    {
      dispose: () => {
        if (refreshTimer) clearInterval(refreshTimer);
        refreshTimer = undefined;
      },
    },
    vscode.commands.registerCommand('quota.openPanel', async () => {
      await refresh(false);
      await showQuotaPanel(snapshot, config);
    }),
    vscode.commands.registerCommand('quota.refresh', async () => {
      if (!checkManualRefreshCooldown()) return;
      await refresh(true, { refreshProviders: true });
    }),
    vscode.commands.registerCommand('quota.openSettings', async () => {
      await vscode.commands.executeCommand('workbench.action.openSettings', `@ext:${context.extension.id}`);
    }),
    vscode.commands.registerCommand('quota.connectGitHubCopilot', async () => {
      try {
        await githubCopilotProvider.connect();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`GitHub Copilot connection failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.refreshGitHubCopilot', async () => {
      try {
        if (!checkManualRefreshCooldown()) return;
        await githubCopilotProvider.refreshAll();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`GitHub Copilot refresh failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.disconnectGitHubCopilot', async () => {
      await githubCopilotProvider.disconnect();
      await refresh(false, { refreshProviders: false });
    }),
    vscode.commands.registerCommand('quota.connectCodex', async () => {
      try {
        await codexProvider.connect();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Codex connection failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.refreshCodex', async () => {
      try {
        if (!checkManualRefreshCooldown()) return;
        await codexProvider.refreshAll();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Codex refresh failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.disconnectCodex', async () => {
      await codexProvider.disconnect();
      await refresh(false, { refreshProviders: false });
    }),
    vscode.commands.registerCommand('quota.connectClaude', async () => {
      try {
        await claudeProvider.connect();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Claude Code connection failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.refreshClaude', async () => {
      try {
        if (!checkManualRefreshCooldown()) return;
        await claudeProvider.refreshAll();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Claude Code refresh failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.disconnectClaude', async () => {
      await claudeProvider.disconnect();
      await refresh(false, { refreshProviders: false });
    }),
    vscode.commands.registerCommand('quota.connectAntigravity', async () => {
      try {
        await antigravityProvider.connect();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Antigravity connection failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.refreshAntigravity', async () => {
      try {
        if (!checkManualRefreshCooldown()) return;
        await antigravityProvider.refreshAll();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Antigravity refresh failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.disconnectAntigravity', async () => {
      await antigravityProvider.disconnect();
      await refresh(false, { refreshProviders: false });
    }),
    vscode.commands.registerCommand('quota.connectKiro', async () => {
      try {
        await kiroProvider.connect();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Kiro connection failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.refreshKiro', async () => {
      try {
        if (!checkManualRefreshCooldown()) return;
        await kiroProvider.refreshAll();
        await refresh(true, { refreshProviders: false });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        void vscode.window.showErrorMessage(`Kiro refresh failed: ${message}`);
      }
    }),
    vscode.commands.registerCommand('quota.disconnectKiro', async () => {
      await kiroProvider.disconnect();
      await refresh(false, { refreshProviders: false });
    }),
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (event.affectsConfiguration('quota')) {
        await refresh(false);
        scheduleAutoRefresh();
      }
    }),
  );
}

export function deactivate(): void {
  if (refreshTimer) clearInterval(refreshTimer);
  statusBar?.dispose();
}
