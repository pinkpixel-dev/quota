import { CANONICAL_TRACK_ORDER, PROVIDER_ORDER } from './constants';
import { displayPercent, formatPercent, formatReset, formatUpdated, meterPercent } from './format';
import type { ProviderId, QuotaConfiguration, QuotaSnapshot, QuotaTrack } from './types';

export interface PanelRow {
  id: string;
  label: string;
  percentLabel: string;
  meterPercent: number;
  tone: 'ok' | 'warn' | 'danger';
  resetLabel: string;
}

export interface PanelStat {
  id: string;
  label: string;
  value: string;
}

/** One connected account: its percentage windows as rows, value-only tracks as header stats. */
export interface AccountCard {
  key: string;
  providerId: ProviderId;
  providerLabel: string;
  accountLabel: string;
  stats: PanelStat[];
  rows: PanelRow[];
  updatedLabel: string;
  errors: string[];
}

export function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function displayTrackLabel(label: string): string {
  const lower = label.toLowerCase();
  if (lower === '5h window' || lower === '5h usage') return '5h';
  if (lower === 'weekly window' || lower === 'weekly usage') return 'Weekly';
  if (lower === 'monthly usage') return 'Monthly';
  if (lower === 'gemini models') return 'Gemini 5h';
  if (lower === 'gemini models weekly') return 'Gemini Weekly';
  if (lower === 'claude/gpt models') return 'Claude/GPT 5h';
  if (lower === 'claude/gpt models weekly') return 'Claude/GPT Weekly';
  if (lower === 'available ai credits') return 'AI credits';
  return label;
}

function trackPercentLabel(track: QuotaTrack, config: QuotaConfiguration): string {
  const percent = displayPercent(track, config.statusBarDisplay);
  const suffix = config.statusBarDisplay === 'percentRemaining' ? 'left' : 'used';
  return `${formatPercent(percent)} ${suffix}`;
}

function trackTone(track: QuotaTrack): PanelRow['tone'] {
  const used = track.percentUsed ?? (track.percentRemaining == null ? undefined : 100 - track.percentRemaining);
  if (used != null && used >= 90) return 'danger';
  if (used != null && used >= 70) return 'warn';
  return 'ok';
}

function trackOrder(a: QuotaTrack, b: QuotaTrack): number {
  // Keep a provider's windows in a fixed order, shortest first, instead of alphabetical.
  const trackSort = CANONICAL_TRACK_ORDER.indexOf(a.id) - CANONICAL_TRACK_ORDER.indexOf(b.id);
  return trackSort === 0 ? a.label.localeCompare(b.label) : trackSort;
}

export function buildAccountCards(snapshot: QuotaSnapshot, config: QuotaConfiguration): AccountCard[] {
  const enabled = new Set(config.enabledProviders);
  const groups = new Map<string, QuotaTrack[]>();

  for (const track of snapshot.tracks) {
    if (!enabled.has(track.providerId)) continue;
    const key = `${track.providerId}:${track.accountId}`;
    const group = groups.get(key);
    if (group) group.push(track);
    else groups.set(key, [track]);
  }

  const cards = [...groups.entries()].map(([key, tracks]): AccountCard => {
    const sorted = [...tracks].sort(trackOrder);
    const first = sorted[0];
    const updatedAt = Math.max(...sorted.map((track) => track.updatedAt ?? 0));

    return {
      key,
      providerId: first.providerId,
      providerLabel: first.providerLabel,
      accountLabel: first.accountLabel,
      stats: sorted
        .filter((track) => track.valueLabel)
        .map((track) => ({ id: track.id, label: displayTrackLabel(track.label), value: track.valueLabel! })),
      rows: sorted
        .filter((track) => !track.valueLabel)
        .map((track) => ({
          id: track.id,
          label: displayTrackLabel(track.label),
          percentLabel: trackPercentLabel(track, config),
          meterPercent: meterPercent(track, config.statusBarDisplay),
          tone: trackTone(track),
          resetLabel: track.resetLabel ?? formatReset(track.resetAt),
        })),
      updatedLabel: formatUpdated(updatedAt > 0 ? updatedAt : null),
      errors: [...new Set(sorted.map((track) => track.error).filter((error): error is string => !!error))],
    };
  });

  // Array.sort is stable, so accounts keep their provider's storage order.
  return cards.sort((a, b) => PROVIDER_ORDER.indexOf(a.providerId) - PROVIDER_ORDER.indexOf(b.providerId));
}

function renderRow(row: PanelRow): string {
  const toneClass = row.tone === 'ok' ? '' : row.tone;
  return `
        <li class="quota-row">
          <div class="row-top">
            <span class="row-label">${escapeHtml(row.label)}</span>
            <span class="quota-percent ${toneClass}">${escapeHtml(row.percentLabel)}</span>
          </div>
          <div class="meter" role="meter" aria-label="${escapeHtml(row.label)}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${row.meterPercent}">
            <div class="meter-fill ${toneClass}" style="width: ${row.meterPercent}%"></div>
          </div>
          <div class="row-meta">${escapeHtml(row.resetLabel)}</div>
        </li>`;
}

export function renderAccountCard(card: AccountCard): string {
  const stats = card.stats.map((stat) => `
        <div class="card-stat">
          <span class="card-stat-value">${escapeHtml(stat.value)}</span>
          <span class="card-stat-label">${escapeHtml(stat.label)}</span>
        </div>`).join('');

  return `
    <article class="account-card" aria-label="${escapeHtml(`${card.providerLabel}, ${card.accountLabel}`)}">
      <div class="card-head">
        <div class="card-heading">
          <div class="card-title">${escapeHtml(card.providerLabel)}</div>
          <div class="quota-account">${escapeHtml(card.accountLabel)}</div>
        </div>${stats}
      </div>
      ${card.rows.length > 0 ? `<ul class="quota-rows">${card.rows.map(renderRow).join('')}
      </ul>` : ''}
      <div class="card-meta">${escapeHtml(card.updatedLabel)}</div>
      ${card.errors.map((error) => `<div class="quota-error">${escapeHtml(error)}</div>`).join('')}
    </article>
  `;
}
