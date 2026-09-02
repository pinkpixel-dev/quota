import { invoke } from '@tauri-apps/api/core';

/**
 * Opens a link in the user's default browser.
 *
 * This goes through a Rust command instead of the opener plugin because the
 * plugin hands the URL to a child process that inherits the AppImage
 * environment, which makes the browser fail to start. The Rust side strips
 * those variables and reports a real error when no launcher worked.
 */
export async function openExternalUrl(url: string): Promise<void> {
  await invoke('open_external_url', { url });
}
