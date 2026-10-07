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
exports.showQuotaPanel = showQuotaPanel;
exports.updateQuotaPanel = updateQuotaPanel;
// @env node
const crypto = __importStar(require("node:crypto"));
const vscode = __importStar(require("vscode"));
const authError_1 = require("./authError");
const panelCards_1 = require("./panelCards");
const panelStyles_1 = require("./panelStyles");
let panel;
const PANEL_PROVIDERS = [
    { id: 'githubCopilot', label: 'Copilot', command: 'GitHubCopilot' },
    { id: 'codex', label: 'Codex', command: 'Codex' },
    { id: 'claude', label: 'Claude', command: 'Claude' },
    { id: 'antigravity', label: 'Antigravity', command: 'Antigravity' },
    { id: 'kiro', label: 'Kiro', command: 'Kiro' },
    { id: 'grok', label: 'Grok', command: 'Grok' },
    { id: 'opencodeGo', label: 'OpenCode Go', command: 'OpenCodeGo' },
];
/** Panel buttons that run `quota.<command>` and then re-render the panel. */
const PANEL_COMMANDS = new Set([
    'refresh',
    ...PANEL_PROVIDERS.flatMap((provider) => [`connect${provider.command}`, `disconnect${provider.command}`]),
]);
function nonce() {
    return crypto.randomBytes(16).toString('base64url');
}
function renderProviderGroup(provider, accountCount, requiresReauthentication) {
    const connect = `connect${provider.command}`;
    const label = (0, panelCards_1.escapeHtml)(provider.label);
    if (accountCount === 0) {
        return `
      <div class="provider-group" role="group" aria-label="${label}">
        <span class="provider-name">${label}</span>
        <button type="button" class="secondary" data-command="${connect}" title="Connect a ${label} account">Connect</button>
      </div>`;
    }
    const accountsLabel = accountCount === 1 ? '1 account' : `${accountCount} accounts`;
    const reauthenticate = requiresReauthentication
        ? `<button type="button" data-command="${connect}" title="Reauthenticate ${label}">Reauthenticate</button>`
        : '';
    return `
      <div class="provider-group" role="group" aria-label="${label}, ${accountsLabel}">
        <span class="provider-name" title="${accountsLabel}">${label} · ${accountCount}</span>
        ${reauthenticate}
        <button type="button" class="secondary" data-command="${connect}" title="Add another ${label} account">Add</button>
        <button type="button" class="secondary" data-command="disconnect${provider.command}" title="Choose a ${label} account to disconnect">Disconnect</button>
      </div>`;
}
function renderEmpty(snapshot) {
    const warning = snapshot.warnings[0] ?? 'No quota data found yet.';
    return `
    <section class="empty">
      <div class="empty-title">No quota tracks yet</div>
      <p>${(0, panelCards_1.escapeHtml)(warning)}</p>
    </section>
  `;
}
function renderHtml(webview, snapshot, config) {
    const scriptNonce = nonce();
    const cards = (0, panelCards_1.buildAccountCards)(snapshot, config);
    const sourceLabel = snapshot.exportedAt
        ? `Safe summary exported ${snapshot.exportedAt}`
        : `Source: ${snapshot.sourcePath}`;
    const accountCountLabel = cards.length === 1 ? '1 account' : `${cards.length} accounts`;
    const reauthenticationProviders = new Set(snapshot.tracks
        .filter((track) => (0, authError_1.isReauthenticationRequired)(track.error))
        .map((track) => track.providerId));
    const providerGroups = PANEL_PROVIDERS
        .map((provider) => renderProviderGroup(provider, cards.filter((card) => card.providerId === provider.id).length, reauthenticationProviders.has(provider.id)))
        .join('');
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${scriptNonce}';">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Quota</title>
  <style>
${panelStyles_1.PANEL_STYLES}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <div class="title-row">
          <h1>Quota</h1>
          <span class="count">${(0, panelCards_1.escapeHtml)(accountCountLabel)}</span>
        </div>
        <div class="source">${(0, panelCards_1.escapeHtml)(sourceLabel)}</div>
      </div>
      <div class="utility-actions">
        <button type="button" data-command="refresh">Refresh</button>
        <button type="button" class="secondary" data-command="chooseStatusBarItems" title="Choose which quotas show in the status bar">Status bar</button>
        <button type="button" class="secondary" data-command="settings">Settings</button>
      </div>
    </header>
    <nav class="provider-actions" aria-label="Provider accounts">
      ${providerGroups}
    </nav>
    ${cards.length > 0 ? `<section class="list">${cards.map(panelCards_1.renderAccountCard).join('')}</section>` : renderEmpty(snapshot)}
  </main>
  <script nonce="${scriptNonce}">
    const vscode = acquireVsCodeApi();
    document.addEventListener('click', (event) => {
      const button = event.target.closest('button[data-command]');
      if (!button) return;
      vscode.postMessage({ command: button.dataset.command });
    });
  </script>
</body>
</html>`;
}
async function runPanelCommand(command) {
    if (command === 'settings') {
        await vscode.commands.executeCommand('quota.openSettings');
        return;
    }
    if (command === 'chooseStatusBarItems') {
        await vscode.commands.executeCommand('quota.chooseStatusBarItems');
        return;
    }
    if (!PANEL_COMMANDS.has(command))
        return;
    await vscode.commands.executeCommand(`quota.${command}`);
    await vscode.commands.executeCommand('quota.openPanel');
}
async function showQuotaPanel(snapshot, config) {
    if (panel) {
        panel.reveal(panel.viewColumn);
    }
    else {
        panel = vscode.window.createWebviewPanel('quota.panel', 'Quota', vscode.ViewColumn.Beside, { enableScripts: true });
        panel.onDidDispose(() => {
            panel = undefined;
        });
        panel.webview.onDidReceiveMessage(async (message) => {
            if (typeof message.command === 'string')
                await runPanelCommand(message.command);
        });
    }
    panel.webview.html = renderHtml(panel.webview, snapshot, config);
}
function updateQuotaPanel(snapshot, config) {
    if (panel)
        panel.webview.html = renderHtml(panel.webview, snapshot, config);
}
//# sourceMappingURL=panel.js.map