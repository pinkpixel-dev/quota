# Quota VS Code Extension

Quota is a small IDE AI quota monitor for VS Code and OpenVSX-compatible IDEs. It focuses on glanceable quota status while you are coding.

## Supported Providers

The VSIX focuses on tools that are useful inside VS Code or VS Code-derived IDEs:

- GitHub Copilot
- Codex
- Claude Code
- Antigravity
- Kiro

## Settings

```json
{
  "quota.dataSource": "extensionAccounts",
  "quota.refresh.intervalSeconds": 120,
  "quota.summaryPath": "",
  "quota.providers.enabled": ["githubCopilot", "codex", "claude"],
  "quota.statusBar.enabled": true,
  "quota.statusBar.items": ["codex.primary"],
  "quota.statusBar.display": "percentUsed",
  "quota.statusBar.maxItems": 3
}
```

## Commands

- `Quota: Connect GitHub Copilot`
- `Quota: Refresh GitHub Copilot`
- `Quota: Disconnect GitHub Copilot`
- `Quota: Connect Codex`
- `Quota: Refresh Codex`
- `Quota: Disconnect Codex`
- `Quota: Connect Claude Code`
- `Quota: Refresh Claude Code`
- `Quota: Disconnect Claude Code`
- `Quota: Connect Antigravity`
- `Quota: Refresh Antigravity`
- `Quota: Disconnect Antigravity`
- `Quota: Connect Kiro`
- `Quota: Refresh Kiro`
- `Quota: Disconnect Kiro`
- `Quota: Refresh Summary`
- `Quota: Open Quota Panel`
- `Quota: Open Settings`

## Quota Panel

Click the status bar `Quota` button or run `Quota: Open Quota Panel` to open a compact in-editor panel. It shows enabled quota tracks with percent used/remaining, reset timing, last-updated timing, and quick actions for refresh, provider connect/disconnect, and settings.

## Status Bar Tracks

Available track IDs:

- `githubCopilot.premium`
- `githubCopilot.chat`
- `githubCopilot.inline`
- `codex.primary`
- `codex.weekly`
- `claude.fiveHour`
- `claude.weekly`
- `claude.weeklySonnet`
- `claude.extraUsage`
- `antigravity.gemini`
- `antigravity.geminiWeekly`
- `antigravity.claude`
- `antigravity.claudeWeekly`
- `kiro.promptCredits`
