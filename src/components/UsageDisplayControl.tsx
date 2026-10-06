import { USAGE_DISPLAY_MODES, formatUsageDisplayMode, type UsageDisplayMode } from '../data/usageDisplay';

interface UsageDisplayControlProps {
  value: UsageDisplayMode;
  onChange: (mode: UsageDisplayMode) => void;
}

export function UsageDisplayControl({ value, onChange }: UsageDisplayControlProps) {
  return (
    <div className="segmented-control" role="group" aria-label="Show usage as">
      {USAGE_DISPLAY_MODES.map((mode) => (
        <button
          key={mode}
          type="button"
          className={mode === value ? 'segmented-control__item segmented-control__item--active' : 'segmented-control__item'}
          onClick={() => onChange(mode)}
          aria-pressed={mode === value}
        >
          {formatUsageDisplayMode(mode)}
        </button>
      ))}
    </div>
  );
}
