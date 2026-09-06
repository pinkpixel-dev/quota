import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

interface TrayMenuSnapshot {
  rows: string[];
  refreshing: boolean;
}

export function updateTrayMenu(snapshot: TrayMenuSnapshot): Promise<void> {
  return invoke('update_tray_menu', { snapshot });
}

export function listenForTrayRefresh(handler: () => void): Promise<UnlistenFn> {
  return listen('tray-refresh-requested', handler);
}
