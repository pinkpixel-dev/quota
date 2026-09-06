# Changelog

All notable changes to this project will be documented here.

## 1.4.0 - September 6, 2026

### ✨ Tray Usage

- The desktop tray menu now lists compact usage for every connected account in the saved provider order.
- Each account stays visible when usage has not loaded yet, and selecting an account row opens Quota.
- Tray rows use safe account summaries. Raw tokens, refresh tokens, and API keys never enter the tray display model.
- The tray limits unusually large account lists to 30 rows and links back to Quota for the remaining accounts.

### 🔄 Refresh

- Added a `Refresh usage` tray action that refreshes every connected provider without opening the main window.
- The refresh action changes to `Refreshing usage...` and blocks duplicate refresh requests until the current refresh finishes.
- Manual and timed refreshes update the tray from the same in-memory account summaries used by the dashboard.

### 🧪 Testing

- Added focused frontend tests for provider ordering, percentage normalization, count-based Copilot usage, and accounts without loaded usage.
- Added Rust tests for tray-row cleanup, Unicode-safe length limits, menu mnemonic escaping, and large account lists.

### 🏷️ Versioning

- Bumped the desktop app to `1.4.0`. The VS Code/OpenVSX extension stays at `1.2.0`.

## 1.3.0 - September 2, 2026

### ✨ Grok Provider

- Added Grok (xAI Grok Build) as a tracked provider in the desktop app and the VS Code extension, contributed by [@ozgur-d](https://github.com/ozgur-d).
- Connect an account two ways: an xAI device-code sign-in that opens `accounts.x.ai` and polls for the token, or a local import of the credentials the Grok CLI already keeps in `~/.grok/auth.json`. The import honours `GROK_HOME`.
- Usage comes from the Grok CLI billing endpoint: credit window with its reset time, monthly spend, on-demand spend, prepaid balance, and a per-product breakdown. Plan tier is filled in from the subscriptions endpoint when billing does not report it.
- The desktop app gets a dashboard card, an account page, provider ordering, a visibility toggle, pinning, notifications, JSON export, and the reauthenticate flow. A saved provider order from an older build picks up Grok on its own.
- The extension tracks `grok.credits`, `grok.monthlySpend`, and `grok.onDemand`, with connect, import-local, refresh, and disconnect commands.
- Raw tokens stay in the Rust backend on desktop and in `SecretStorage` in the extension. Only display-safe data crosses the boundary.

### 🔒 Security

- Added `accounts.x.ai` and `auth.x.ai` to the Rust sign-in host allow list, so Grok sign-in works through the sanitized-environment opener added in 1.2.2 rather than the old opener plugin.

### 🐛 Fixes

- A Grok token missing the `grok-cli:access` and `api:access` scopes is answered `403` by the billing endpoint rather than `401`. Because refreshing reissues the same scopes, a `403` now asks you to reconnect instead of retrying a refresh that cannot widen them. A `401` still refreshes normally.

### ⚠️ Upgrading

- If you connected Grok from a pre-release build, disconnect and reconnect it once, or re-import from the Grok CLI. A token saved before this change carries the narrow scopes and refreshing will not widen them.

### 🧪 Testing

- Added 6 Rust integration tests for Grok covering single and multi-account import, credential-leak assertions, quota parsing, and refresh-failure classification.
- Added 7 extension unit tests for Grok usage parsing.
- Extended the sign-in host test to cover both xAI hosts.

### 🏷️ Versioning

- Desktop app to `1.3.0`. VS Code extension to `1.2.0`, packaged into `quota-vscode/versions/`.

## 1.2.2 - September 2, 2026

### 🐛 Fixes

- Fixed sign-in links not opening a browser in the Linux AppImage. Clicking "Open Claude", "Open OpenAI", or any other provider's connect button appeared to do nothing.
- The AppImage runtime points `LD_LIBRARY_PATH` at its own bundled libraries. The old `tauri-plugin-opener` path passed that environment straight to `xdg-open`, so the browser started against the wrong system libraries and crashed. `xdg-open` still exits with a success code after that crash, which is why no error was ever shown.
- Sign-in links now open through a new Rust `open_external_url` command that removes the AppImage variables from the launcher's environment, keeps the host entries in `PATH` and `XDG_DATA_DIRS`, and falls back to `gio open` and `$BROWSER` when `xdg-open` fails.
- Failed launches now show a real error telling you to copy the link, instead of a misleading "Opened with browser fallback" message that never appeared.

### 🔒 Security

- Moved the sign-in host allow list into Rust. Only `https` URLs on the six known provider hosts can be opened, which still covers device-flow URLs that arrive from a provider's API response.

### 🧪 Testing

- Added unit coverage for URL validation, host parsing, and AppDir path stripping.
- Added an integration test that runs a real child process to confirm the AppImage environment never reaches a browser launcher, and that a normal deb or rpm install keeps its own environment untouched.

### 📦 Release Tooling

- Added `npm run build:release`, a single command that checks the version fields agree, type checks, runs the Rust tests, bundles every installer with the AppImage build environment applied, lists what was produced, and rewrites `SHA256SUMS.txt`.
- Added `npm run checksums` for regenerating `SHA256SUMS.txt` from existing installers, which is what you want after copying Windows artifacts in from CI.
- The build stops early when `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` disagree on the version, instead of producing installers with mixed version numbers.
- Replaced `SHA256SUMS.txt` with verified checksums for the three Linux desktop `v1.2.2` installers. The Windows installers still need a run of the Windows workflow.

### 🏷️ Versioning

- Bumped the desktop app to `1.2.2`. The VS Code/OpenVSX extension stays at `1.1.1`.

## 1.2.1 - August 29, 2026

### 🔄 Codex Quota Windows

- Restored the Codex five-hour limit from the primary quota window in the desktop app and VS Code/OpenVSX extension.
- Restored the separate Codex weekly limit from the secondary quota window in both products.
- Kept the extension track ID `codex.primary` for the five-hour limit and restored `codex.weekly` for the weekly limit.
- Preserved the existing desktop `hourly*` cache fields, so saved accounts and extension settings do not need a migration.

### 🧪 Testing

- Updated desktop and extension coverage for Codex responses that contain primary five-hour and secondary weekly windows.

### 📦 Release Preparation

- Replaced `SHA256SUMS.txt` with verified checksums for the five desktop `v1.2.1` installers.

### 🏷️ Versioning

- Bumped the desktop app to `1.2.1` and the VS Code/OpenVSX extension to `1.1.1`.

## 1.2.0 - July 27, 2026

### 🔐 Authentication

- Added clear reauthentication-required states for expired or rejected Codex and Claude Code refresh tokens in the desktop app.
- Added account-level Reauthenticate actions that reuse the existing secure OAuth flows without deleting saved accounts first.
- Updated Claude Code refresh handling to retry once with a refreshed token after an unexpected authorization failure.
- Added extension reauthentication errors, manual-refresh alerts, and Codex/Claude Code Reauthenticate actions in the quota panel.
- Replaced raw or opaque token-refresh responses with safe, provider-specific messages while retaining redacted diagnostics for temporary failures.

### 🎨 Dashboard

- Anchored pin and remove controls to the far-right edge of list-mode account rows so destructive actions remain easy to find.
- Preserved the existing top-right action placement in default and compact card layouts.
- Removed the fixed 320-pixel document minimum that caused a narrow horizontal scrollbar at the smallest supported viewport.

### 🧪 Testing

- Added Rust coverage for rejected Codex and Claude Code refresh-token classification without response-body leakage.
- Added extension coverage for authentication-required and temporary refresh failures.
- Verified the desktop list layout and reauthentication state across 320, 375, 414, 768, 1024, 1440, and 1920-pixel widths.

### 📦 Release Preparation

- Built the desktop `v1.2.0` AppImage, `.deb`, and `.rpm` packages and the extension `v1.1.0` VSIX.
- Launch-tested the `v1.2.0` AppImage on CachyOS for 25 seconds with no new coredump.
- Verified the native package metadata, extension test suite, VSIX contents, and archive integrity.
- Added the Windows `v1.2.0` MSI and NSIS installers and replaced `SHA256SUMS.txt` with a verified five-installer `v1.2.0` manifest.

### 🏷️ Versioning

- Bumped the desktop app to `1.2.0` and the VS Code/OpenVSX extension to `1.1.0`.

## 1.1.1 - July 24, 2026

### 🔄 Changes

- Updated the desktop app and VS Code/OpenVSX extension to show Codex's current primary seven-day window as the single Weekly Limit.
- Removed the obsolete Codex secondary weekly row from desktop cards, low-quota notifications, extension tracks, status-bar settings, and Marketplace documentation.
- Kept the existing `codex.primary` extension track ID and legacy desktop-summary cache keys so current user settings and cached accounts remain compatible.
- Added desktop and extension coverage for the current primary-only Codex usage payload and its weekly reset timestamp.
- Changed pinned extension status-bar tracks from white dots to usage-aware indicators: green above 30% remaining, yellow from 11–30%, red at 10% or less, and neutral when percentage data is unavailable.
- Prepared patch release metadata for desktop `v1.1.1` and extension `v1.0.5` without generating release artifacts.
- Reworked `APPIMAGE_FIX.md` into a cross-distribution Tauri AppImage build and troubleshooting guide, with stable-baseline builds as the default and symptom-specific fallbacks for Arch/CachyOS issues.
- Expanded the `v1.1.1` release notes into combined publish-ready copy covering desktop `v1.1.1`, extension `v1.0.5`, compatibility details, and the deferred Codex reset display.
- Updated the README with direct links to desktop `v1.1.1` and extension `v1.0.5`.
- Replaced the checksum manifest with verified SHA-256 checksums for the four desktop `v1.1.1` installers.

### 🐛 Fixes

- Fixed an immediate `SIGSEGV` when launching the `v1.1.1` AppImage on current CachyOS by disabling incompatible GIO module scanning inside AppImage runs before Tauri initializes.

## 2026-06-28

### Added

- Added opt-in desktop notifications for low or exhausted quota. Enable in Settings → Notifications with a configurable threshold (default 20% remaining). Fires once per quota drop and clears when the quota recovers above the threshold. Covers all providers: GitHub Copilot, Codex, Antigravity, Claude, Kiro, and Cursor.
- Added system tray icon to the desktop app. Closing the window now hides it to the tray rather than quitting; left-clicking the tray icon or choosing "Show Quota" from the right-click menu restores the window. "Quit Quota" in the tray menu fully exits the app. Auto-refresh continues running in the background while the window is hidden.

### Fixed

- Fixed broken `label` prop strings on `CodexMetricRow`, `ClaudeMetricRow`, and `AntigravityMetricRow` components that were missing "5 Hour" from the label text.

---

- Added desktop Antigravity AI credit display by parsing safe `paidTier.availableCredits` data from the existing Cloud Code `loadCodeAssist` response and showing a Model Credits row only when a valid credit amount is available.
- Added Rust coverage for parsing Antigravity AI credits from `loadCodeAssist`.
- Added VSIX Antigravity AI credit display for direct-auth and desktop-summary data, with a value-only `antigravity.credits` track that can appear in the quota panel or be pinned to the status bar.
- Added VSIX tests for Antigravity credit parsing and track formatting.
- Changed the VSIX Claude Code refresh path to suppress expected usage-rate-limit (`429`) errors, keeping the last good quota visible instead of repeating a noisy provider backoff message on quota cards.
- Clarified that auto-refresh should default to 120 seconds to avoid unnecessary provider rate limits.
- Added opt-in desktop auto refresh with a persisted Settings toggle and configurable interval. The default interval is 120 seconds, with 30 seconds as the minimum.

## 2026-06-27

### Added (VSIX)

- Added the Kiro direct-auth VSIX slice with local callback OAuth, SecretStorage token storage, safe metadata/cache state, direct AWS runtime quota refresh, refresh-token retry, manual refresh, disconnect, auto-refresh support, and a prompt credits quota track.
- Added VSIX tests for Kiro usage parsing, including nested usage-state payloads, plan normalization, and timestamp normalization.
- Added the GitHub Copilot direct-auth VSIX slice with GitHub device login, SecretStorage token storage, safe metadata/cache state, direct Copilot quota refresh, manual refresh, disconnect, auto-refresh support, and premium/chat/inline quota tracks.
- Added VSIX tests for GitHub Copilot quota parsing, including date-only reset handling and RFC3339 reset handling.

### Changed (VSIX)

- Fixed the quota panel Settings button so it opens settings for the installed extension ID instead of the old hardcoded Marketplace ID.
- Added the Quota app icon to the VSIX package manifest so the extension has a Marketplace listing icon.
- Rewrote `quota-vscode/README.md` as a marketplace-facing details page with a stronger product intro, screenshots, feature list, supported-provider table, usage guidance, settings, commands, privacy notes, and desktop app link.
- Updated the quota panel track list to use a responsive grid: narrow panels stay single-column, while wider panels can show tracks in multiple columns to reduce scrolling now that all providers are connected.

## 2026-06-26 (evening)

### Changed (VSIX)

- Renamed `antigravity.thirdParty` and `antigravity.thirdPartyWeekly` track IDs to `antigravity.claude` and `antigravity.claudeWeekly` throughout the extension. The status bar settings dropdown now shows `antigravity.claude` and `antigravity.claudeWeekly` instead of the old `thirdParty` names.
- Renamed `codex.primary` track label from "Primary window" to "5h window" and updated all display paths so the window type reads "5h" instead of "Immediate" everywhere.
- Added `TRACK_STATUS_BAR_LABEL` map giving each quota track a concise abbreviated label for the status bar (e.g. `Codex:5h`, `Codex:Wk`, `Claude:5h`, `Agy:Gemini:5h`, `Agy:Claude:Wk`) so tracks are clearly identifiable when multiple appear at once.
- Status bar items now sort by canonical provider and track order regardless of the order they were added to `quota.statusBar.items`, so same-provider tracks always group together.
- Status bar track items are now assigned fixed priority values based on canonical track position so their visual order in the status bar stays stable.
- Updated quota panel track labels so Antigravity rows read "Gemini 5h", "Gemini Weekly", "Claude/GPT 5h", and "Claude/GPT Weekly" instead of the longer raw label strings.
- Removed the `quota.summaryPath` setting from the VS Code settings UI since all providers now use direct auth.

## 2026-06-26

### Added

- Added the Antigravity direct-auth VSIX slice with Google OAuth, local callback capture, SecretStorage token storage, safe metadata/cache state, Cloud Code quota refresh, token refresh before expiry, manual refresh, disconnect, auto-refresh support, and Gemini Models / Claude-GPT five-hour and weekly quota tracks.
- Added the Claude Code direct-auth VSIX slice with browser OAuth, hosted callback/code paste, SecretStorage token storage, safe metadata/cache state, direct usage refresh, token refresh before expiry, manual refresh, disconnect, auto-refresh support, and five-hour/weekly/optional Sonnet/extra quota tracks.
- Added a compact VSIX webview quota panel with quota rows, percent bars, reset timing, last-updated timing, refresh, Codex connect/disconnect, and settings actions.
- Added the first direct-auth Codex VSIX slice with OAuth connection, local callback capture, SecretStorage token storage, safe metadata/cache state, direct quota refresh, token refresh retry, manual refresh, disconnect, auto-refresh support, and status bar/quota panel tracks.
- Added `quota-vscode/VSIX_DIRECT_AUTH_PLAN.md` documenting the direct-OAuth extension direction, Codex-first provider slice, auto-refresh requirements, storage boundary, and lessons from `ORIGINAL_VSIX/`.
- Added the initial self-contained `quota-vscode/` extension scaffold with a TypeScript VS Code package, `Quota` status bar button, configurable status bar quota tracks, compact Quick Pick panel, safe-summary parser, and extension planning docs.
- Added nested VS Code run/debug configuration for the extension so opening `quota-vscode/` and pressing F5 launches an Extension Development Host.
- Added `SHA256SUMS.txt` with verified checksums for the `v1.0.0` AppImage, `.deb`, `.rpm`, and Windows `.exe` release artifacts.
- Added `RELEASE_NOTES.md` with `v1.0.0` release notes, a suggested GitHub repository description, and suggested repository tags.
- Added a manually triggered GitHub Actions workflow for building Windows Tauri installers and uploading them as workflow artifacts.
- Added the generated Tauri desktop icon set from the root `icon.png` source and configured the bundle icon paths for Linux, macOS, and Windows packaging.
- Added a pin button (Pin/PinOff icons) to every account card on provider account pages. Pinned accounts are shown on the dashboard instead of the default first-two; multiple pinned accounts per provider all show; unpinning all reverts to the default first-two fallback. Dashboard cards for pinned accounts show a PinOff icon to unpin; default first-two cards show no pin button. Pin state persists in localStorage.
- Added eye/eye-off toggle per provider in the Settings Dashboard order list, letting users hide any connected provider from the dashboard without disconnecting it. Hidden state persists in localStorage.
- Added a Settings hint under Dashboard order informing users they can pin accounts from each provider's accounts page.

### Changed

- Added Antigravity to the VSIX quota panel provider action row and status bar track settings.
- Rearranged the VSIX quota panel actions so Refresh and Settings sit together in the header, while provider connect/disconnect actions sit on a separate row above the quota list.
- Made VSIX status bar quota tracks render as regular text without warning-colored backgrounds, and made quota panel provider buttons switch between Connect and Disconnect based on visible connected provider tracks.
- Updated the VSIX quota panel actions so Codex and Claude Code can both connect/disconnect from the panel.
- Polished the VSIX quota panel with tighter spacing, slimmer action buttons, clearer Immediate/Weekly labels, a track count pill, and better narrow-column layout.
- Replaced the VSIX Quick Pick quota list with the compact webview panel so enabled quota tracks are easier to scan.
- Verified the Codex direct-auth VSIX slice in the F5 Extension Development Host: Codex connection succeeds and the status bar display updates.
- Updated VSIX planning docs to make direct OAuth the preferred long-term path, with the safe-summary bridge kept as a fallback/testing path.
- Documented the VSIX MVP direction: GitHub Copilot, Codex, Claude Code, Antigravity, and Kiro are in scope; Cursor is intentionally excluded; weekly/additional limits live in the quota panel; Antigravity Gemini is not duplicated while model rows share one quota.
- Confirmed the Linux release binaries were built, the Windows installer was collected with the Linux artifacts, and the AppImage was tested successfully for `1.0.0`.
- Stopped ignoring generated Tauri `.ico` and `.icns` icon assets so Windows and macOS bundle icon files are available in CI.
- Changed the Tauri npm wrapper to invoke the local `@tauri-apps/cli` JavaScript entrypoint through Node instead of spawning the Windows `tauri.cmd` shim, fixing the GitHub Actions `spawnSync tauri.cmd EINVAL` failure.
- Decided to skip Docker packaging for `1.0.0` and focus release distribution on native Tauri bundles.
- Updated the app release version to `1.0.0` across npm, Cargo, and Tauri metadata.
- Enabled Tauri bundling and routed `npm run tauri ...` through a small Node wrapper that sets `NO_STRIP=true` for Linux AppImage builds.
- Rewrote the README as user-facing project documentation with a centered logo, screenshot gallery, feature overview, integration notes, privacy model, and development commands.
- Added persisted provider ordering in Settings and applied it to the dashboard provider summary cards and connected provider sections.
- Made the dashboard provider summary cards clickable so each provider count opens that provider's all-accounts view.
- Added a persisted System/Dark/Light theme setting with a cool off-white light theme and `#f59e0b` light-mode accent.
- Rearranged the Settings page layout: Appearance section now spans the full width at the top, with Data and Privacy side by side below it.
- Updated compact dashboard layout to remove forced equal row heights across provider sections; sections are now their natural height, eliminating dead space from uneven account counts.

## 2026-06-25

### Added

- Added a Settings navigation item and Settings page with Appearance, Data, and Privacy sections.
- Added separate persisted view options for the Dashboard and provider account pages, each supporting Default, Compact, and List layouts.
- Added safe JSON export for frontend account summaries, including connected account totals and provider summary objects without raw tokens, refresh tokens, or API keys.
- Added the Cursor integration with deep-link polling browser OAuth, local SQLite import from `~/.config/Cursor/User/globalStorage/state.vscdb`, user meta lookups, Stripe subscription profile mapping, and usage-summary parsing (Total Usage, Auto + Composer, API Usage, On-Demand disabled status).
- Added `CursorUsageCard` component to match the reference design screenshot exactly, displaying plan badges, Auth IDs, Total Usage dollars/percentages, reset dates, Auto + Composer, API Usage, and On-Demand states.
- Added `CursorAccountsView` view to allow full account management (list, refresh, refresh all, delete, back to dashboard).
- Added the Kiro integration with PKCE browser OAuth, local file import from `~/.aws/sso/cache/kiro-auth-token.json` and optional `~/.config/Kiro/User/globalStorage/kiro.kiroagent/profile.json`, AWS runtime usage fetch, safe account summaries, Quota-owned storage, account deletion, dashboard cards, all-accounts view, and Integrations row with Connect and Import local actions.
- Added `tiny_http` as a Cargo dependency for the Kiro local OAuth callback server.
- Added `plan-badge` CSS class for the `KIRO FREE` / `KIRO PRO` plan badge on usage cards.

### Changed

- Tightened the default Dashboard card spacing, made Compact mode substantially denser with a three-column provider grid, side-by-side account cards, full-width single-account provider cards, and row-style List mode cards.
- Changed the Cursor integration status from `planned` to `reference` in the integrations list.
- Updated the dashboard, summary grid, and Integrations view to support Cursor.
- Changed the Kiro integration status from `planned` to `reference` in the integrations list.
- Updated the dashboard, summary grid, and Integrations view to include Kiro alongside existing providers.

### Fixed

- Fixed Cursor local import failing to compile due to rusqlite connection/statement non-`Send` futures across await boundaries. Root cause: SQLite Connection and Statement objects were held across the `.await` point in `import_cursor_from_local`. Resolution: wrapped the synchronous rusqlite database querying inside `tokio::task::spawn_blocking` to offload blocking DB queries to a background thread pool and drop rusqlite types before the await.
- Fixed Kiro OAuth browser login failing immediately with a Cognito `redirect_mismatch` error. Root cause: the callback URL passed as `redirect_uri` to `app.kiro.dev/signin` included a path suffix (`/oauth/callback`). Kiro forwards this to Amazon Cognito, which has `http://localhost` (no port, no path) registered and only matches bare localhost URLs. Removing the path from `callback_url` so it is just `http://localhost:PORT` fixes the mismatch for Google and GitHub login.
- Fixed Kiro OAuth callback server returning `Not Found` for some login providers (AWS Builder ID, and any provider whose callback path differs from `/oauth/callback`). The server now accepts any path that carries a `code` or `error` parameter instead of restricting to specific paths.
- Fixed Kiro OAuth callback server returning a `302` redirect to `app.kiro.dev` after success, which caused browser error dialogs on some systems. The server now returns a plain HTML page with a `window.close()` script instead.
- Fixed second (and later) Kiro accounts overwriting the first connected account. Root cause: the account ID was computed from `email + profileArn` before the usage fetch, but Kiro's token exchange endpoint often returns no email field. Both accounts then hashed to the same empty-email ID. The ID is now computed after the usage API response is applied, which reliably provides `userInfo.email`. If email is still unavailable after usage, the ID falls back to a hash of the access token to prevent collision.

### Removed

- Removed unused Windsurf and Zed planned integrations from the frontend integrations list and deleted their branding assets.
- Removed redundant individual account refresh buttons from usage cards (GitHub Copilot, Codex, Claude, Antigravity, Kiro, Cursor), relying on the provider-level refresh buttons.
- Removed clutter/explanatory text from the dashboard page header and the sidebar bottom footer.

---

- Added the first Codex integration slice with local `~/.codex/auth.json` import, safe account summaries, Quota-owned storage, quota refresh, account deletion, dashboard cards, and an all-accounts view.
- Added Rust tests for Codex local import safety and Codex quota-window parsing.
- Added Codex OAuth login through OpenAI with PKCE, local callback capture, and multiple-account support.
- Added Codex refresh-token handling so expired OAuth access tokens can recover during quota refresh.
- Added the first Antigravity integration slice with local Google/Gemini credential import, safe account summaries, Quota-owned storage, quota parsing, refresh commands, account deletion, dashboard cards, and an all-accounts view.
- Added Rust tests for Antigravity local import safety and Antigravity quota-bucket parsing.
- Added Antigravity Google OAuth login with local callback capture, Cloud Code Assist scopes, safe account summaries, and multiple-account support.
- Added the first Claude Code integration slice with OAuth login, manual callback/code paste, profile lookup, usage refresh, safe account summaries, Quota-owned storage, account deletion, dashboard cards, and an all-accounts view.
- Added Rust tests for Claude OAuth URL generation, callback parsing, token-summary safety, and Claude usage-window parsing.

### Changed

- Updated the dashboard and Integrations view so Codex can be imported and managed alongside GitHub Copilot.
- Changed the Codex integration row to offer both direct OAuth connection and local auth import.
- Changed Codex quota rows to display remaining percentage as the filled bar, matching the `left` label and avoiding healthy quota appearing depleted.
- Updated the dashboard and Integrations view so Antigravity can be imported and managed alongside GitHub Copilot and Codex.
- Changed the Antigravity quota refresh request to include the Gemini metadata payload used by the original app, and updated the Antigravity usage card labels to match the native "Gemini Models" and "Claude and GPT models" grouping.
- Changed the Antigravity integration row to offer direct Google OAuth connection while keeping local import as a fallback.
- Changed Antigravity OAuth to use the original Antigravity client and expanded Google scopes instead of the Gemini client.
- Changed the Antigravity remote quota flow to send Antigravity-style Cloud Code metadata, user-agent headers, `x-goog-api-client`, and a `fetchAvailableModels` preflight before requesting quota summary data.
- Changed Antigravity quota refresh to tolerate empty successful Cloud Code responses, preserve parse diagnostics with response context, and store early refresh errors on the account card instead of only showing a global dashboard error.
- Changed Antigravity Cloud Code requests to stop advertising unsupported compressed response encodings so compressed bytes are not parsed as JSON.
- Changed Antigravity account labels so OAuth-connected accounts display as OAuth accounts until plan/tier data is available.
- Changed Antigravity quota rows to display remaining percentage as the filled bar, matching the original app's usage semantics more closely.
- Changed the Tauri/Vite development server binding to use `127.0.0.1:5173` consistently so the desktop webview does not split between IPv4 and IPv6 dev servers.
- Documented the original app's Claude Code OAuth/profile/usage endpoints as the preferred next integration path.
- Updated the dashboard, summary cards, account views, and Integrations view so Claude Code can be connected and managed alongside GitHub Copilot, Codex, and Antigravity.
- Updated the roadmap to prioritize Claude Code after the main Antigravity fix and to track a future cleaner Quota extension.

## 2026-06-18

### Added

- Started the fresh Pink Pixel rebuild under the placeholder name Panel.
- Added a clean Tauri 2, React, TypeScript, Rust, and Vite scaffold.
- Added English project documentation for setup, contribution, roadmap, security, and architecture direction.
- Added an Apache-2.0 license for new root-level Pink Pixel code.
- Adopted the app name Quota and tagline "Track your AI usage in one place."
- Added the first GitHub Copilot integration slice with device-login start, login completion, account listing, usage refresh, account deletion, and safe frontend summaries.
- Added the Tauri opener plugin for the GitHub Copilot device-login page.
- Added a clean Dashboard view that shows only connected accounts.
- Added a separate Integrations view for connecting providers.
- Added original-style GitHub Copilot usage detail rows with included status, premium request counts, and reset timing.
- Added dashboard and account-view planning notes based on the new reference screenshots and collected provider icons.
- Added dashboard provider summary cards, including total connected accounts and per-provider counts.
- Added a GitHub Copilot provider section that shows up to two visible account cards.
- Added a GitHub Copilot all-accounts view with refresh, add, and per-account actions.
- Added provider brand SVGs to public app assets and wired them into dashboard, integration, and Copilot usage UI.

### Changed

- Moved the previous codebase into `ORIGINAL/` as reference-only source material.
- Updated the initial integration order to prioritize GitHub Copilot, Codex, and Antigravity before the harder-to-test Claude flow.
- Changed the copied GitHub Copilot data boundary so raw tokens remain in Rust-owned storage instead of being returned to React.
- Changed the GitHub auth button from a plain browser anchor to a Tauri native opener call.
- Removed the scaffold-style top status/hero/reference sections from the main app UI.
- Kept GitHub Copilot OAuth on the Copilot-compatible VS Code client ID because custom GitHub OAuth app client IDs do not complete the Copilot token exchange.
- Matched the original reset-time behavior by reading the Copilot token `rd` fallback when explicit reset fields are missing.
- Fixed GitHub Copilot date-only reset values like `2026-07-01` so they display with local reset timing instead of `Reset unknown`.
- Cleaned stale Tauri/Cargo build artifacts after the root directory rename so generated permission files point at the current `quota` path.
- Expanded the UI roadmap to cover provider summary cards, two visible account cards per provider, full account views, notification settings, and light mode.
- Changed the dashboard content area to use the full available width instead of centering a narrow account column.
- Increased sidebar logo, app name, nav text, and spacing to better match the wider dashboard layout.
- Changed monochrome provider brand icons to render white in dark mode using a theme-ready CSS filter variable.

### Removed

- Excluded the old Go sidecar and Electron helper from the new app direction.
- Excluded legacy ads, sponsors, relay surfaces, remote announcements, and remote behavior configuration from the new scaffold.
