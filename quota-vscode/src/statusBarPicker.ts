// @env node
import * as vscode from 'vscode';

import { CANONICAL_TRACK_ORDER, PROVIDER_LABELS, PROVIDER_ORDER, TRACK_LABELS } from './constants';
import { isTrackSelected, parseStatusBarEntry, statusBarEntryKey } from './statusBarSelection';
import type { ProviderId, QuotaConfiguration, QuotaSnapshot, QuotaTrack } from './types';

const MAX_STATUS_BAR_ITEMS = 8;

interface TrackPickItem extends vscode.QuickPickItem {
  track?: QuotaTrack;
}

function buildPickItems(snapshot: QuotaSnapshot, config: QuotaConfiguration): TrackPickItem[] {
  const items: TrackPickItem[] = [];

  for (const providerId of PROVIDER_ORDER) {
    if (!config.enabledProviders.includes(providerId)) continue;
    const providerTracks = snapshot.tracks
      .filter((track) => track.providerId === providerId)
      .sort((a, b) => CANONICAL_TRACK_ORDER.indexOf(a.id) - CANONICAL_TRACK_ORDER.indexOf(b.id));
    const accountIds = [...new Set(providerTracks.map((track) => track.accountId))];

    for (const accountId of accountIds) {
      const accountTracks = providerTracks.filter((track) => track.accountId === accountId);
      items.push({
        label: `${PROVIDER_LABELS[providerId]} · ${accountTracks[0].accountLabel}`,
        kind: vscode.QuickPickItemKind.Separator,
      });
      for (const track of accountTracks) {
        items.push({
          label: TRACK_LABELS[track.id] ?? track.label,
          description: track.accountLabel,
          picked: isTrackSelected(config.statusBarItems, track, snapshot.tracks),
          track,
        });
      }
    }
  }

  return items;
}

/** Shows every connected account's tracks and saves the chosen ones as `trackId@accountId` entries. */
export async function chooseStatusBarItems(snapshot: QuotaSnapshot, config: QuotaConfiguration): Promise<boolean> {
  const items = buildPickItems(snapshot, config);
  if (!items.some((item) => item.track)) {
    void vscode.window.showInformationMessage('Connect an account first, then choose which quotas show in the status bar.');
    return false;
  }

  const picked = await vscode.window.showQuickPick(items, {
    title: 'Quota: Status Bar Items',
    placeHolder: 'Pick the quotas to show in the status bar',
    canPickMany: true,
    matchOnDescription: true,
  });
  if (!picked) return false;

  const offeredProviders = new Set(items.filter((item) => item.track).map((item) => item.track!.providerId));
  // Keep entries for providers this list did not show (disabled or no data yet). Entries for
  // shown providers are replaced by the selection, which also drops disconnected accounts.
  const kept = config.statusBarItems.filter((value) => {
    const entry = parseStatusBarEntry(value);
    return entry != null && !offeredProviders.has(entry.trackId.slice(0, entry.trackId.indexOf('.')) as ProviderId);
  });
  const next = [...kept, ...picked.filter((item) => item.track).map((item) => statusBarEntryKey(item.track!))];

  const section = vscode.workspace.getConfiguration('quota');
  const target = section.inspect('statusBar.items')?.workspaceValue !== undefined
    ? vscode.ConfigurationTarget.Workspace
    : vscode.ConfigurationTarget.Global;
  await section.update('statusBar.items', next, target);

  if (next.length > config.statusBarMaxItems) {
    const raise = Math.min(MAX_STATUS_BAR_ITEMS, next.length);
    const action = raise > config.statusBarMaxItems ? `Show ${raise}` : undefined;
    const message = `Only the first ${config.statusBarMaxItems} of ${next.length} selected quotas fit the status bar limit.`;
    const selected = action
      ? await vscode.window.showWarningMessage(message, action)
      : await vscode.window.showWarningMessage(message);
    if (action && selected === action) await section.update('statusBar.maxItems', raise, vscode.ConfigurationTarget.Global);
  }

  return true;
}
