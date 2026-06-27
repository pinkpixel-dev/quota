// @env node
import * as vscode from 'vscode';

import { CANONICAL_TRACK_ORDER, EXTENSION_NAME } from './constants';
import { statusBarLabel } from './format';
import type { QuotaConfiguration, QuotaSnapshot, TrackId } from './types';

export class QuotaStatusBar {
  private readonly mainItem: vscode.StatusBarItem;
  private readonly trackItems = new Map<TrackId, vscode.StatusBarItem>();

  constructor() {
    this.mainItem = vscode.window.createStatusBarItem('quota.main', vscode.StatusBarAlignment.Right, 90);
    this.mainItem.name = EXTENSION_NAME;
    this.mainItem.command = 'quota.openPanel';
  }

  dispose(): void {
    this.mainItem.dispose();
    for (const item of this.trackItems.values()) item.dispose();
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

    const sortedTrackIds = [...config.statusBarItems].sort((a, b) => {
      const ai = CANONICAL_TRACK_ORDER.indexOf(a);
      const bi = CANONICAL_TRACK_ORDER.indexOf(b);
      return (ai === -1 ? 999 : ai) - (bi === -1 ? 999 : bi);
    });
    const selectedTrackIds = sortedTrackIds.slice(0, Math.max(0, config.statusBarMaxItems));
    const visibleTracks = selectedTrackIds
      .map((id) => snapshot.tracks.find((track) => track.id === id && config.enabledProviders.includes(track.providerId)))
      .filter((track) => track != null);
    const visibleIds = new Set(visibleTracks.map((track) => track.id));

    for (const [id, item] of this.trackItems.entries()) {
      if (!visibleIds.has(id)) item.hide();
    }

    for (const track of visibleTracks) {
      const item = this.getTrackItem(track.id);
      item.text = `$(circle-filled) ${statusBarLabel(track, config.statusBarDisplay)}`;
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

  private getTrackItem(id: TrackId): vscode.StatusBarItem {
    const existing = this.trackItems.get(id);
    if (existing) return existing;

    const canonicalIndex = CANONICAL_TRACK_ORDER.indexOf(id);
    const priority = 89 - (canonicalIndex === -1 ? 50 : canonicalIndex);
    const item = vscode.window.createStatusBarItem(`quota.${id}`, vscode.StatusBarAlignment.Right, priority);
    item.name = `${EXTENSION_NAME}: ${id}`;
    item.command = 'quota.openPanel';
    this.trackItems.set(id, item);
    return item;
  }

  private hideTrackItems(): void {
    for (const item of this.trackItems.values()) item.hide();
  }
}
