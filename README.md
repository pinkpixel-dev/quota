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
- Theme Modes: System, Dark, Light
- Safe JSON Export for account summaries
- Account Pages for providers with refresh and delete buttons
- Tauri desktop shell

## Installation

Download for your platform at https://github.com/pinkpixel-dev/quota/releases

Or install from source:

```bash
git clone https://github.com/pinkpixel-dev/quota.git
cd quota
npm install
npm run tauri dev
```

## Tech Stack

- Tauri 2
- React
- TypeScript
- Rust
- Vite
- CSS variables

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for details.

## License

Quota is licensed under Apache-2.0.

Made with 💖 by Pink Pixel.
