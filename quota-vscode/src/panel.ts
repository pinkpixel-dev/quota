// @env node
import * as crypto from 'node:crypto';

import * as vscode from 'vscode';

import { isReauthenticationRequired } from './authError';
import { buildAccountCards, escapeHtml, renderAccountCard } from './panelCards';
import { PANEL_STYLES } from './panelStyles';
import type { ProviderId, QuotaConfiguration, QuotaSnapshot } from './types';

let panel: vscode.WebviewPanel | undefined;

interface PanelProvider {
  id: ProviderId;
  label: string;
  command: string;
}

const PANEL_PROVIDERS: PanelProvider[] = [
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

function nonce(): string {
  return crypto.randomBytes(16).toString('base64url');
}

function renderProviderGroup(provider: PanelProvider, accountCount: number, requiresReauthentication: boolean): string {
  const connect = `connect${provider.command}`;
  const label = escapeHtml(provider.label);

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

function renderEmpty(snapshot: QuotaSnapshot): string {
  const warning = snapshot.warnings[0] ?? 'No quota data found yet.';
  return `
    <section class="empty">
      <div class="empty-title">No quota tracks yet</div>
      <p>${escapeHtml(warning)}</p>
    </section>
  `;
}

function renderHtml(webview: vscode.Webview, snapshot: QuotaSnapshot, config: QuotaConfiguration): string {
  const scriptNonce = nonce();
  const cards = buildAccountCards(snapshot, config);
  const sourceLabel = snapshot.exportedAt
    ? `Safe summary exported ${snapshot.exportedAt}`
    : `Source: ${snapshot.sourcePath}`;
  const accountCountLabel = cards.length === 1 ? '1 account' : `${cards.length} accounts`;
  const reauthenticationProviders = new Set(
    snapshot.tracks
      .filter((track) => isReauthenticationRequired(track.error))
      .map((track) => track.providerId),
  );
  const providerGroups = PANEL_PROVIDERS
    .map((provider) => renderProviderGroup(
      provider,
      cards.filter((card) => card.providerId === provider.id).length,
      reauthenticationProviders.has(provider.id),
    ))
    .join('');

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${scriptNonce}';">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Quota</title>
  <style>
${PANEL_STYLES}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <div class="title-row">
          <h1>Quota</h1>
          <span class="count">${escapeHtml(accountCountLabel)}</span>
        </div>
        <div class="source">${escapeHtml(sourceLabel)}</div>
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
    ${cards.length > 0 ? `<section class="list">${cards.map(renderAccountCard).join('')}</section>` : renderEmpty(snapshot)}
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

async function runPanelCommand(command: string): Promise<void> {
  if (command === 'settings') {
    await vscode.commands.executeCommand('quota.openSettings');
    return;
  }
  if (command === 'chooseStatusBarItems') {
    await vscode.commands.executeCommand('quota.chooseStatusBarItems');
    return;
  }
  if (!PANEL_COMMANDS.has(command)) return;

  await vscode.commands.executeCommand(`quota.${command}`);
  await vscode.commands.executeCommand('quota.openPanel');
}

export async function showQuotaPanel(snapshot: QuotaSnapshot, config: QuotaConfiguration): Promise<void> {
  if (panel) {
    panel.reveal(panel.viewColumn);
  } else {
    panel = vscode.window.createWebviewPanel(
      'quota.panel',
      'Quota',
      vscode.ViewColumn.Beside,
      { enableScripts: true },
    );

    panel.onDidDispose(() => {
      panel = undefined;
    });

    panel.webview.onDidReceiveMessage(async (message: { command?: string }) => {
      if (typeof message.command === 'string') await runPanelCommand(message.command);
    });
  }

  panel.webview.html = renderHtml(panel.webview, snapshot, config);
}

export function updateQuotaPanel(snapshot: QuotaSnapshot, config: QuotaConfiguration): void {
  if (panel) panel.webview.html = renderHtml(panel.webview, snapshot, config);
}
