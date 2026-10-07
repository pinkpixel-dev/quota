// @env node
import * as vscode from 'vscode';

import { EXTENSION_NAME } from './constants';
import { statusBarIndicator, statusBarLabel } from './format';
import { resolveStatusBarTracks } from './statusBarSelection';
import type { QuotaConfiguration, QuotaSnapshot } from './types';

export class QuotaStatusBar {
  private readonly mainItem: vscode.StatusBarItem;
  private readonly trackItems = new Map<string, { item: vscode.StatusBarItem; priority: number }>();

  constructor() {
    this.mainItem = vscode.window.createStatusBarItem('quota.main', vscode.StatusBarAlignment.Right, 90);
    this.mainItem.name = EXTENSION_NAME;
    this.mainItem.command = 'quota.openPanel';
  }

  dispose(): void {
    this.mainItem.dispose();
    for (const { item } of this.trackItems.values()) item.dispose();
  }

  update(snapshot: QuotaSnapshot, config: QuotaConfiguration): void {
    if (!config.statusBarEnabled) {
      this.mainItem.hide();
      this.hideTrackItems();
      return;
    }

    this.mainItem.text = '$(pulse) Quota';
    this.mainItem.tooltip = snapshot.warnings.length > 0
      ? `${snapshot.warnings[0]}\n\nClick to open Quota.`
      : `Click to open Quota.\nSource: ${snapshot.sourcePath}`;
    this.mainItem.show();

    const visibleTracks = resolveStatusBarTracks(
      config.statusBarItems,
      snapshot.tracks,
      config.enabledProviders,
      config.statusBarMaxItems,
    );
    const visibleKeys = new Set(visibleTracks.map((item) => item.key));

    for (const [key, entry] of this.trackItems.entries()) {
      if (!visibleKeys.has(key)) entry.item.hide();
    }

    for (const { key, track, accountTag, priority } of visibleTracks) {
      const item = this.getTrackItem(key, priority);
      const label = statusBarLabel(track, config.statusBarDisplay);
      item.text = `${statusBarIndicator(track)} ${accountTag ? `${label} · ${accountTag}` : label}`;
      item.tooltip = [
        `${track.providerLabel}: ${track.label}`,
        track.accountLabel,
        track.error ? `Last error: ${track.error}` : undefined,
        snapshot.sourcePath,
      ].filter(Boolean).join('\n');
      item.backgroundColor = undefined;
      item.show();
    }
  }

  /** Priority is fixed at creation, so an item is recreated when its position changes. */
  private getTrackItem(key: string, priority: number): vscode.StatusBarItem {
    const existing = this.trackItems.get(key);
    if (existing?.priority === priority) return existing.item;
    existing?.item.dispose();

    const item = vscode.window.createStatusBarItem(`quota.${key}`, vscode.StatusBarAlignment.Right, priority);
    item.name = `${EXTENSION_NAME}: ${key.slice(0, key.indexOf('@'))}`;
    item.command = 'quota.openPanel';
    this.trackItems.set(key, { item, priority });
    return item;
  }

  private hideTrackItems(): void {
    for (const { item } of this.trackItems.values()) item.hide();
  }
}
