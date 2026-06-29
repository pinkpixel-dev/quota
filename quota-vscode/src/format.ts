import { TRACK_STATUS_BAR_LABEL } from './constants';
import type { QuotaTrack, StatusBarDisplayMode, TrackId } from './types';

export function formatPercent(value: number | undefined): string {
  return value == null ? 'No data' : `${Math.round(value)}%`;
}

export function displayPercent(track: QuotaTrack, mode: StatusBarDisplayMode): number | undefined {
  if (mode === 'percentRemaining') return track.percentRemaining ?? (track.percentUsed == null ? undefined : 100 - track.percentUsed);
  return track.percentUsed ?? (track.percentRemaining == null ? undefined : 100 - track.percentRemaining);
}

export function statusBarLabel(track: QuotaTrack, mode: StatusBarDisplayMode): string {
  const label = TRACK_STATUS_BAR_LABEL[track.id as TrackId] ?? track.providerLabel;
  if (track.valueLabel) return `${label} ${track.valueLabel}`;
  const suffix = mode === 'percentRemaining' ? 'left' : 'used';
  return `${label} ${formatPercent(displayPercent(track, mode))} ${suffix}`;
}

export function formatReset(resetAt: number | null | undefined): string {
  if (resetAt == null) return 'Reset unknown';

  const date = new Date(resetAt);
  if (Number.isNaN(date.getTime())) return 'Reset unknown';

  return `Resets ${date.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  })}`;
}

export function formatUpdated(updatedAt: number | null | undefined): string {
  if (updatedAt == null) return 'Not refreshed yet';

  const date = new Date(updatedAt);
  if (Number.isNaN(date.getTime())) return 'Not refreshed yet';

  return `Updated ${date.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  })}`;
}
