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
exports.activate = activate;
exports.deactivate = deactivate;
// @env node
const vscode = __importStar(require("vscode"));
const antigravityProvider_1 = require("./antigravityProvider");
const claudeProvider_1 = require("./claudeProvider");
const codexProvider_1 = require("./codexProvider");
const configuration_1 = require("./configuration");
const githubCopilotProvider_1 = require("./githubCopilotProvider");
const kiroProvider_1 = require("./kiroProvider");
const panel_1 = require("./panel");
const summary_1 = require("./summary");
const statusBar_1 = require("./statusBar");
let config;
let snapshot;
let statusBar;
let githubCopilotProvider;
let codexProvider;
let claudeProvider;
let antigravityProvider;
let kiroProvider;
let refreshTimer;
let lastManualRefreshAt = 0;
const MANUAL_REFRESH_COOLDOWN_MS = 10_000;
async function loadSnapshot() {
    if (config.dataSource === 'desktopSummary')
        return (0, summary_1.loadQuotaSnapshot)(config.summaryPath);
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
async function refresh(showToast = false, options = {}) {
    config = (0, configuration_1.readConfiguration)();
    if (config.dataSource === 'extensionAccounts' && options.refreshProviders) {
        if (await githubCopilotProvider.hasAccounts())
            await githubCopilotProvider.refreshAll();
        if (await codexProvider.hasAccounts())
            await codexProvider.refreshAll();
        if (await claudeProvider.hasAccounts())
            await claudeProvider.refreshAll();
        if (await antigravityProvider.hasAccounts())
            await antigravityProvider.refreshAll();
        if (await kiroProvider.hasAccounts())
            await kiroProvider.refreshAll();
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
function checkManualRefreshCooldown() {
    const elapsed = Date.now() - lastManualRefreshAt;
    if (elapsed < MANUAL_REFRESH_COOLDOWN_MS) {
        const seconds = Math.ceil((MANUAL_REFRESH_COOLDOWN_MS - elapsed) / 1000);
        void vscode.window.showWarningMessage(`Please wait ${seconds}s before refreshing again.`);
        return false;
    }
    lastManualRefreshAt = Date.now();
    return true;
}
function scheduleAutoRefresh() {
    if (refreshTimer) {
        clearInterval(refreshTimer);
        refreshTimer = undefined;
    }
    if (config.dataSource !== 'extensionAccounts')
        return;
    const seconds = Math.min(3600, Math.max(30, config.refreshIntervalSeconds));
    refreshTimer = setInterval(() => {
        void refresh(false, { refreshProviders: true });
    }, seconds * 1000);
}
async function activate(context) {
    statusBar = new statusBar_1.QuotaStatusBar();
    githubCopilotProvider = new githubCopilotProvider_1.GitHubCopilotProvider(context);
    codexProvider = new codexProvider_1.CodexProvider(context);
    claudeProvider = new claudeProvider_1.ClaudeProvider(context);
    antigravityProvider = new antigravityProvider_1.AntigravityProvider(context);
    kiroProvider = new kiroProvider_1.KiroProvider(context);
    config = (0, configuration_1.readConfiguration)();
    snapshot = await loadSnapshot();
    statusBar.update(snapshot, config);
    scheduleAutoRefresh();
    context.subscriptions.push(statusBar, {
        dispose: () => {
            if (refreshTimer)
                clearInterval(refreshTimer);
            refreshTimer = undefined;
        },
    }, vscode.commands.registerCommand('quota.openPanel', async () => {
        await refresh(false);
        await (0, panel_1.showQuotaPanel)(snapshot, config);
    }), vscode.commands.registerCommand('quota.refresh', async () => {
        if (!checkManualRefreshCooldown())
            return;
        await refresh(true, { refreshProviders: true });
    }), vscode.commands.registerCommand('quota.openSettings', async () => {
        await vscode.commands.executeCommand('workbench.action.openSettings', `@ext:${context.extension.id}`);
    }), vscode.commands.registerCommand('quota.connectGitHubCopilot', async () => {
        try {
            await githubCopilotProvider.connect();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`GitHub Copilot connection failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.refreshGitHubCopilot', async () => {
        try {
            if (!checkManualRefreshCooldown())
                return;
            await githubCopilotProvider.refreshAll();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`GitHub Copilot refresh failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.disconnectGitHubCopilot', async () => {
        await githubCopilotProvider.disconnect();
        await refresh(false, { refreshProviders: false });
    }), vscode.commands.registerCommand('quota.connectCodex', async () => {
        try {
            await codexProvider.connect();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`Codex connection failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.refreshCodex', async () => {
        try {
            if (!checkManualRefreshCooldown())
                return;
            await codexProvider.refreshAll();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`Codex refresh failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.disconnectCodex', async () => {
        await codexProvider.disconnect();
        await refresh(false, { refreshProviders: false });
    }), vscode.commands.registerCommand('quota.connectClaude', async () => {
        try {
            await claudeProvider.connect();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`Claude Code connection failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.refreshClaude', async () => {
        try {
            if (!checkManualRefreshCooldown())
                return;
            await claudeProvider.refreshAll();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`Claude Code refresh failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.disconnectClaude', async () => {
        await claudeProvider.disconnect();
        await refresh(false, { refreshProviders: false });
    }), vscode.commands.registerCommand('quota.connectAntigravity', async () => {
        try {
            await antigravityProvider.connect();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`Antigravity connection failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.refreshAntigravity', async () => {
        try {
            if (!checkManualRefreshCooldown())
                return;
            await antigravityProvider.refreshAll();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`Antigravity refresh failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.disconnectAntigravity', async () => {
        await antigravityProvider.disconnect();
        await refresh(false, { refreshProviders: false });
    }), vscode.commands.registerCommand('quota.connectKiro', async () => {
        try {
            await kiroProvider.connect();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`Kiro connection failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.refreshKiro', async () => {
        try {
            if (!checkManualRefreshCooldown())
                return;
            await kiroProvider.refreshAll();
            await refresh(true, { refreshProviders: false });
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(`Kiro refresh failed: ${message}`);
        }
    }), vscode.commands.registerCommand('quota.disconnectKiro', async () => {
        await kiroProvider.disconnect();
        await refresh(false, { refreshProviders: false });
    }), vscode.workspace.onDidChangeConfiguration(async (event) => {
        if (event.affectsConfiguration('quota')) {
            await refresh(false);
            scheduleAutoRefresh();
        }
    }));
}
function deactivate() {
    if (refreshTimer)
        clearInterval(refreshTimer);
    statusBar?.dispose();
}
//# sourceMappingURL=extension.js.map