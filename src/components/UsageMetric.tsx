import { createContext, useContext } from 'react';
import type { ReactNode } from 'react';
import {
  DEFAULT_USAGE_DISPLAY_MODE,
  LOW_REMAINING_PERCENT,
  clampPercent,
  displayPercentFromUsed,
  usageSuffix,
  type UsageDisplayMode,
} from '../data/usageDisplay';

export const UsageDisplayContext = createContext<UsageDisplayMode>(DEFAULT_USAGE_DISPLAY_MODE);

interface UsageMetricProps {
  label: string;
  /** Percent of the limit used. Drives the bar, the percent text, and the low tone. */
  usedPercent?: number | null;
  /** Replaces the percent text, for rows that read better as counts or dollars. */
  value?: string;
  /** Unlimited on this plan: a full bar when showing remaining, an empty one when showing used. */
  included?: boolean;
  meta?: ReactNode[];
  emphasized?: boolean;
  inactive?: boolean;
}

export function UsageMetric({ label, usedPercent, value, included = false, meta = [], emphasized = false, inactive = false }: UsageMetricProps) {
  const mode = useContext(UsageDisplayContext);
  const used = included ? 0 : usedPercent == null ? null : clampPercent(usedPercent);
  const shown = used == null ? null : displayPercentFromUsed(used, mode);
  const low = !included && used != null && 100 - used <= LOW_REMAINING_PERCENT;
  const text = value ?? (included ? 'Included' : shown == null ? '-' : `${shown}% ${usageSuffix(mode)}`);
  const classes = [
    'usage-metric',
    emphasized ? 'usage-metric--emphasized' : '',
    low ? 'usage-metric--remaining-low' : '',
    inactive ? 'usage-metric--inactive' : '',
  ].filter(Boolean).join(' ');

  return (
    <div className={classes}>
      <div className="usage-metric__line">
        <span>{label}</span>
        <strong>{text}</strong>
      </div>
      <div className="usage-metric__bar" aria-hidden="true">
        <span style={{ width: `${shown ?? 0}%` }} />
      </div>
      {meta.map((line, index) => (
        <div key={index} className="usage-metric__meta">{line}</div>
      ))}
    </div>
  );
}

/** Percent used out of a total, or null when there is no total to measure against. */
export function usedPercentOf(used: number | null | undefined, total: number | null | undefined): number | null {
  if (used == null || total == null || total <= 0) return null;
  return (used / total) * 100;
}
