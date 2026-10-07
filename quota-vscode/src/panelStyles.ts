// Webview CSS for the Quota panel. Colors come from VS Code theme variables.
export const PANEL_STYLES = `
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
  width: 100%;
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

.provider-group {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 2px 2px 2px 8px;
}

.provider-name {
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}

button {
  appearance: none;
  white-space: nowrap;
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

button:focus-visible {
  outline: 1px solid var(--vscode-focusBorder);
  outline-offset: 2px;
}

.list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  align-items: stretch;
  gap: 8px;
  padding-top: 12px;
}

.account-card {
  display: flex;
  flex-direction: column;
}

.account-card,
.empty {
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-raised);
  padding: 10px;
}

.card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.card-heading {
  min-width: 0;
}

.card-title {
  font-weight: 650;
}

.quota-account,
.card-meta,
.row-meta,
.card-stat-label,
.empty p {
  color: var(--muted);
}

.quota-account {
  margin-top: 2px;
  font-size: 12px;
  line-height: 1.3;
  word-break: break-word;
}

.card-stat {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  align-items: flex-end;
  line-height: 1.15;
}

.card-stat-value {
  font-size: 15px;
  font-weight: 650;
  white-space: nowrap;
}

.card-stat-label {
  font-size: 11px;
  white-space: nowrap;
}

.quota-rows {
  display: grid;
  gap: 10px;
  margin: 10px 0 0;
  padding: 10px 0 0;
  border-top: 1px solid var(--border);
  list-style: none;
}

.row-top {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.row-label {
  min-width: 0;
  font-size: 12px;
  font-weight: 600;
}

.quota-percent {
  flex: 0 0 auto;
  font-size: 13px;
  font-weight: 650;
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
  margin: 5px 0 4px;
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

.row-meta,
.card-meta {
  font-size: 11px;
}

.card-meta {
  margin-top: auto;
  padding-top: 10px;
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

  .list {
    grid-template-columns: 1fr;
  }
}
`;
