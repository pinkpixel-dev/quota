<p align="center">
  <img src="icon.png" alt="Quota logo" width="300" height="300">
</p>

# Quota

Monitor your AI & dev tool usage in one place.

Quota is a desktop app for monitoring AI usage across accounts. Connect your accounts for each provider, refresh usage and pin accounts to the dashboard for easy tracking.

## Supported Providers

Currently Quota supports the following providers:

- GitHub Copilot
- Codex
- Antigravity
- Claude Code
- Kiro
- Cursor
- Grok

Every provider saves its raw auth info in the Rust backend and only secure account & usage information are sent to the React frontend.

## Screenshots

### Dashboard

![Default dashboard](screenshots/default_dashboard.png)

### Compact Dashboard

![Compact dashboard](screenshots/compact_dashboard.png)

### List Dashboard

![List dashboard](screenshots/list_dashboard.png)

### Accounts Page Example

![Accounts page](screenshots/accounts_page.png)

### Settings

![Settings page](screenshots/settings_page.png)

## Features

- Dashboard for connected accounts
- Ordering of providers in Settings
- Account pinning in Dashboard
- Toggle provider visibility without disconnection of accounts
- Dashboard Layouts: Default, Compact, List
- List-mode account actions stay anchored to the far-right edge for easier removal
- Theme Modes: System, Dark, Light
- Safe JSON Export for account summaries
- Opt-in auto refresh with a configurable interval
- System tray with compact usage for every connected account, background refresh, and click-to-restore behavior
- Opt-in desktop notifications when a quota drops below a configurable threshold
- Account Pages for providers with refresh and delete buttons
- Clear Codex, Claude Code, and Grok reauthentication prompts when saved authorization expires
- Antigravity AI credit display when credits are available
- Tauri desktop shell
- Separate VS Code/OpenVSX extension scaffold in `quota-vscode/`

## Installation

Download the latest published desktop release, [Quota v1.1.1](https://github.com/pinkpixel-dev/quota/releases/tag/v1.1.1), for your platform. Desktop `v1.5.0` is currently prepared locally and has not been published yet.

Or install from source:

```bash
git clone https://github.com/pinkpixel-dev/quota.git
cd quota
npm install
npm run tauri dev
```

## VS Code Extension

The Quota VSIX is located in `quota-vscode/`. It is a separate TypeScript extension package with a small status bar button, optional configured quota percentages, and a compact webview panel for enabled providers. See the VSIX [README.md](https://github.com/pinkpixel-dev/quota/blob/main/quota-vscode/README.md) for details.

### Extension Installation Options

1. Install from within VSCode, Antigravity or Kiro through the marketplace

2. Download and install from VSIX

- [Open VSX Registry](https://open-vsx.org/extension/pinkpixel/quota-ai-usage-tracker/)

- [GitHub v1.0.5](https://github.com/pinkpixel-dev/quota/blob/main/quota-vscode/versions/quota-ai-usage-tracker-1.0.5.vsix) (latest published VSIX; `v1.2.0` is prepared locally)

Once the .vsix file is downloaded, open your ide (VSCode, Antigravity, Kiro), press F1, and type in "Extensions: Install from VSIX".

If Codex, Claude Code, or Grok authorization expires, Quota keeps the account and its last safe quota data visible. Use the Reauthenticate action in the desktop account card or extension panel to renew access without disconnecting the account first.

## Quota CLI

`quota-cli` is a separate Rust binary that prints the same usage numbers in your terminal. It reads each provider's credentials from wherever that provider's own CLI already stored them, so there's no sign-in step and no account to connect. It doesn't need the desktop app installed or running.

```bash
cargo install quota-cli
```

Kiro's credentials live in a SQLite database that's compiled from bundled C source, so the build needs a working C compiler. If you'd rather not build anything, prebuilt binaries for Linux, macOS, and Windows are attached to the `quota-cli-v*` [releases](https://github.com/pinkpixel-dev/quota/releases). Download the archive for your platform, unpack it, and put `quota-cli` somewhere on your `PATH`.

Then:

```bash
quota-cli usage
```

```text
claude       5h 98% (resets in 4h 58m) · Wk 35% (resets in 3d 9h)
codex        5h 100% (resets in 4h 59m) · Wk 95% (resets in 4d 6h)
cursor       Plan 100% (resets in 8d 3h)
antigravity  5h 100% (resets in 4h 59m) · Wk 97% (resets in 6d 17h)
grok         no credit allocation
kiro         Credits 100% · Bonus 100%
```

Percentages are what you have left. Reset times show when a provider reports one. Anything you're not signed into says so instead of showing a number. Add `--json` for scripting.

It covers Claude, Codex, Cursor, Antigravity, Grok, and Kiro. GitHub Copilot is desktop-only for now. Every reader is strictly read-only: nothing is refreshed, rewritten, or rotated, because those tokens back your own agent sessions.

See [`quota-cli/README.md`](quota-cli/README.md) for the full command reference and where each provider's credentials come from. The shared provider logic is published separately as [`quota-core`](https://crates.io/crates/quota-core).

## Herdr Plugin

If you use Herdr, `herdr-plugin/` shows your AI usage right in the sidebar, next to the panes and workspaces where you're using it. Each pane shows the numbers for the agent it's actually running, so a Claude pane and a Codex pane each show their own.

The plugin is a thin wrapper around `quota-cli`, so install that first, then:

```bash
herdr plugin install pinkpixel-dev/quota/herdr-plugin
```

See [`herdr-plugin/README.md`](herdr-plugin/README.md) for setup and troubleshooting.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for details.

## License

Quota is licensed under Apache-2.0.

Made with 💖 by Pink Pixel.
