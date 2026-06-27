// @env node
import * as crypto from 'node:crypto';

import * as vscode from 'vscode';

import { PROVIDER_ORDER } from './constants';
import { displayPercent, formatPercent, formatReset, formatUpdated } from './format';
import type { ProviderId, QuotaConfiguration, QuotaSnapshot, QuotaTrack } from './types';

let panel: vscode.WebviewPanel | undefined;

interface PanelTrack {
  id: string;
  providerId: ProviderId;
  providerLabel: string;
  label: string;
  accountLabel: string;
  percentLabel: string;
  percentUsed: number | undefined;
  percentRemaining: number | undefined;
  resetLabel: string;
  updatedLabel: string;
  error?: string | null;
}

function nonce(): string {
  return crypto.randomBytes(16).toString('base64url');
}

function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function trackPercentLabel(track: QuotaTrack, config: QuotaConfiguration): string {
  const percent = displayPercent(track, config.statusBarDisplay);
  const suffix = config.statusBarDisplay === 'percentRemaining' ? 'left' : 'used';
  return `${formatPercent(percent)} ${suffix}`;
}

function toPanelTrack(track: QuotaTrack, config: QuotaConfiguration): PanelTrack {
  return {
    id: track.id,
    providerId: track.providerId,
    providerLabel: track.providerLabel,
    label: displayTrackLabel(track.label),
    accountLabel: track.accountLabel,
    percentLabel: trackPercentLabel(track, config),
    percentUsed: track.percentUsed,
    percentRemaining: track.percentRemaining,
    resetLabel: formatReset(track.resetAt),
    updatedLabel: formatUpdated(track.updatedAt),
    error: track.error,
  };
}

function renderProviderAction(provider: 'githubCopilot' | 'codex' | 'claude' | 'antigravity' | 'kiro', label: string, connected: boolean): string {
  const command = connected
    ? provider === 'githubCopilot' ? 'disconnectGitHubCopilot' : provider === 'codex' ? 'disconnectCodex' : provider === 'claude' ? 'disconnectClaude' : provider === 'antigravity' ? 'disconnectAntigravity' : 'disconnectKiro'
    : provider === 'githubCopilot' ? 'connectGitHubCopilot' : provider === 'codex' ? 'connectCodex' : provider === 'claude' ? 'connectClaude' : provider === 'antigravity' ? 'connectAntigravity' : 'connectKiro';
  const verb = connected ? 'Disconnect' : 'Connect';

  return `<button type="button" class="secondary" data-command="${command}">${verb} ${label}</button>`;
}

function displayTrackLabel(label: string): string {
  const lower = label.toLowerCase();
  if (lower === '5h window' || lower === '5h usage') return '5h';
  if (lower === 'weekly window' || lower === 'weekly usage') return 'Weekly';
  if (lower === 'gemini models') return 'Gemini 5h';
  if (lower === 'gemini models weekly') return 'Gemini Weekly';
  if (lower === 'claude/gpt models') return 'Claude/GPT 5h';
  if (lower === 'claude/gpt models weekly') return 'Claude/GPT Weekly';
  return label;
}

function visibleTracks(snapshot: QuotaSnapshot, config: QuotaConfiguration): PanelTrack[] {
  const enabled = new Set(config.enabledProviders);
  return snapshot.tracks
    .filter((track) => enabled.has(track.providerId))
    .sort((a, b) => {
      const providerSort = PROVIDER_ORDER.indexOf(a.providerId) - PROVIDER_ORDER.indexOf(b.providerId);
      return providerSort === 0 ? a.label.localeCompare(b.label) : providerSort;
    })
    .map((track) => toPanelTrack(track, config));
}

function renderTrack(track: PanelTrack): string {
  const used = track.percentUsed ?? (track.percentRemaining == null ? undefined : 100 - track.percentRemaining);
  const width = used == null ? 0 : Math.min(100, Math.max(0, Math.round(used)));
  const isHot = used != null && used >= 90;
  const isWarn = used != null && used >= 70 && used < 90;

  return `
    <article class="quota-row">
      <div class="quota-main">
        <div class="quota-heading">
          <div class="quota-title">${escapeHtml(track.providerLabel)} <span>${escapeHtml(track.label)}</span></div>
          <div class="quota-account">${escapeHtml(track.accountLabel)}</div>
        </div>
        <div class="quota-percent ${isHot ? 'danger' : isWarn ? 'warn' : ''}">${escapeHtml(track.percentLabel)}</div>
      </div>
      <div class="meter" aria-hidden="true">
        <div class="meter-fill ${isHot ? 'danger' : isWarn ? 'warn' : ''}" style="width: ${width}%"></div>
      </div>
      <div class="quota-meta">
        <span>${escapeHtml(track.resetLabel)}</span>
        <span>${escapeHtml(track.updatedLabel)}</span>
      </div>
      ${track.error ? `<div class="quota-error">${escapeHtml(track.error)}</div>` : ''}
    </article>
  `;
}

function renderEmpty(snapshot: QuotaSnapshot): string {
  const warning = snapshot.warnings[0] ?? 'No quota data found yet.';
  return `
    <section class="empty">
      <div class="empty-title">No quota tracks yet</div>
      <p>${escapeHtml(warning)}</p>
      <div class="empty-actions">
        <button type="button" data-command="connectGitHubCopilot">Connect Copilot</button>
        <button type="button" class="secondary" data-command="connectCodex">Connect Codex</button>
        <button type="button" class="secondary" data-command="connectClaude">Connect Claude</button>
        <button type="button" class="secondary" data-command="connectAntigravity">Connect Antigravity</button>
        <button type="button" class="secondary" data-command="connectKiro">Connect Kiro</button>
      </div>
    </section>
  `;
}

function renderHtml(webview: vscode.Webview, snapshot: QuotaSnapshot, config: QuotaConfiguration): string {
  const scriptNonce = nonce();
  const tracks = visibleTracks(snapshot, config);
  const sourceLabel = snapshot.exportedAt
    ? `Safe summary exported ${snapshot.exportedAt}`
    : `Source: ${snapshot.sourcePath}`;
  const trackCountLabel = tracks.length === 1 ? '1 track' : `${tracks.length} tracks`;
  const connectedProviders = new Set(tracks.map((track) => track.providerId));

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${scriptNonce}';">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Quota</title>
  <style>
    :root {
      color-scheme: light dark;
      --surface: var(--vscode-editor-background);
      --surface-raised: var(--vscode-sideBar-background);
      --border: var(--vscode-panel-border);
      --text: var(--vscode-foreground);
      --muted: var(--vscode-descriptionForeground);
      --accent: var(--vscode-button-background);
      --accent-text: var(--vscode-button-foreground);
      --danger: var(--vscode-errorForeground);
      --warn: var(--vscode-editorWarning-foreground);
    }

    * {
      box-sizing: border-box;
    }

    body {
      margin: 0;
      min-width: 320px;
      background: var(--surface);
      color: var(--text);
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
      line-height: 1.45;
    }

    main {
      width: min(100%, 1120px);
      margin: 0 auto;
      padding: 14px;
    }

    header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding-bottom: 12px;
      border-bottom: 1px solid var(--border);
    }

    h1 {
      margin: 0;
      font-size: 18px;
      font-weight: 650;
      letter-spacing: 0;
    }

    .title-row {
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .count {
      border: 1px solid var(--border);
      border-radius: 999px;
      color: var(--muted);
      font-size: 11px;
      line-height: 1;
      padding: 4px 7px;
      white-space: nowrap;
    }

    .source {
      margin-top: 3px;
      color: var(--muted);
      font-size: 12px;
      max-width: 360px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .utility-actions,
    .provider-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }

    .utility-actions {
      justify-content: flex-end;
    }

    .provider-actions {
      justify-content: flex-start;
      padding-top: 10px;
      border-bottom: 1px solid var(--border);
      padding-bottom: 12px;
    }

    button {
      appearance: none;
      border: 1px solid var(--vscode-button-border, transparent);
      border-radius: 4px;
      background: var(--accent);
      color: var(--accent-text);
      cursor: pointer;
      font: inherit;
      font-size: 11px;
      min-height: 26px;
      padding: 3px 9px;
    }

    button.secondary {
      background: var(--vscode-button-secondaryBackground);
      color: var(--vscode-button-secondaryForeground);
    }

    button:hover {
      background: var(--vscode-button-hoverBackground);
    }

    button.secondary:hover {
      background: var(--vscode-button-secondaryHoverBackground);
    }

    .list {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
      gap: 8px;
      padding-top: 12px;
    }

    .quota-row,
    .empty {
      border: 1px solid var(--border);
      border-radius: 6px;
      background: var(--surface-raised);
      padding: 10px;
    }

    .quota-main {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 12px;
    }

    .quota-heading {
      min-width: 0;
    }

    .quota-title {
      display: flex;
      align-items: center;
      gap: 7px;
      font-weight: 650;
    }

    .quota-account,
    .quota-meta,
    .empty p {
      color: var(--muted);
    }

    .quota-title span {
      border: 1px solid var(--border);
      border-radius: 999px;
      color: var(--muted);
      font-size: 11px;
      font-weight: 500;
      line-height: 1;
      padding: 4px 7px;
      white-space: nowrap;
    }

    .quota-account {
      margin-top: 5px;
      font-size: 12px;
      line-height: 1.3;
      word-break: break-word;
    }

    .quota-percent {
      flex: 0 0 auto;
      font-size: 17px;
      font-weight: 650;
      line-height: 1.1;
      text-align: right;
      white-space: nowrap;
    }

    .quota-percent.warn {
      color: var(--warn);
    }

    .quota-percent.danger {
      color: var(--danger);
    }

    .meter {
      height: 4px;
      margin: 10px 0 8px;
      overflow: hidden;
      border-radius: 999px;
      background: var(--vscode-input-background);
    }

    .meter-fill {
      height: 100%;
      border-radius: inherit;
      background: var(--accent);
    }

    .meter-fill.warn {
      background: var(--warn);
    }

    .meter-fill.danger {
      background: var(--danger);
    }

    .quota-meta {
      display: flex;
      flex-wrap: wrap;
      gap: 6px 12px;
      font-size: 12px;
    }

    .quota-error {
      margin-top: 8px;
      color: var(--danger);
      font-size: 12px;
    }

    .empty {
      margin-top: 12px;
    }

    .empty-title {
      font-weight: 650;
    }

    .empty p {
      margin: 4px 0 12px;
    }

    .empty-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }

    @media (max-width: 520px) {
      header {
        align-items: stretch;
        flex-direction: column;
      }

      .utility-actions,
      .provider-actions {
        justify-content: flex-start;
      }

      .source {
        max-width: none;
      }

      .quota-main {
        flex-direction: column;
        gap: 8px;
      }

      .quota-percent {
        text-align: left;
      }

      .list {
        grid-template-columns: 1fr;
      }
    }
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <div class="title-row">
          <h1>Quota</h1>
          <span class="count">${escapeHtml(trackCountLabel)}</span>
        </div>
        <div class="source">${escapeHtml(sourceLabel)}</div>
      </div>
      <div class="utility-actions">
        <button type="button" data-command="refresh">Refresh</button>
        <button type="button" class="secondary" data-command="settings">Settings</button>
      </div>
    </header>
    <nav class="provider-actions" aria-label="Provider actions">
      ${renderProviderAction('githubCopilot', 'Copilot', connectedProviders.has('githubCopilot'))}
      ${renderProviderAction('codex', 'Codex', connectedProviders.has('codex'))}
      ${renderProviderAction('claude', 'Claude', connectedProviders.has('claude'))}
      ${renderProviderAction('antigravity', 'Antigravity', connectedProviders.has('antigravity'))}
      ${renderProviderAction('kiro', 'Kiro', connectedProviders.has('kiro'))}
    </nav>
    ${tracks.length > 0 ? `<section class="list">${tracks.map(renderTrack).join('')}</section>` : renderEmpty(snapshot)}
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
  switch (command) {
    case 'refresh':
      await vscode.commands.executeCommand('quota.refresh');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'connectGitHubCopilot':
      await vscode.commands.executeCommand('quota.connectGitHubCopilot');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'disconnectGitHubCopilot':
      await vscode.commands.executeCommand('quota.disconnectGitHubCopilot');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'connectCodex':
      await vscode.commands.executeCommand('quota.connectCodex');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'disconnectCodex':
      await vscode.commands.executeCommand('quota.disconnectCodex');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'connectClaude':
      await vscode.commands.executeCommand('quota.connectClaude');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'disconnectClaude':
      await vscode.commands.executeCommand('quota.disconnectClaude');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'connectAntigravity':
      await vscode.commands.executeCommand('quota.connectAntigravity');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'disconnectAntigravity':
      await vscode.commands.executeCommand('quota.disconnectAntigravity');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'connectKiro':
      await vscode.commands.executeCommand('quota.connectKiro');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'disconnectKiro':
      await vscode.commands.executeCommand('quota.disconnectKiro');
      await vscode.commands.executeCommand('quota.openPanel');
      return;
    case 'settings':
      await vscode.commands.executeCommand('quota.openSettings');
      return;
    default:
      return;
  }
}

export async function showQuotaPanel(snapshot: QuotaSnapshot, config: QuotaConfiguration): Promise<void> {
  if (panel) {
    panel.reveal(vscode.ViewColumn.Beside);
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
