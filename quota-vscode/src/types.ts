export type ProviderId = 'githubCopilot' | 'codex' | 'claude' | 'antigravity' | 'kiro';

export type StatusBarDisplayMode = 'percentUsed' | 'percentRemaining';

export type QuotaDataSource = 'extensionAccounts' | 'desktopSummary';

export type TrackId =
  | 'githubCopilot.premium'
  | 'githubCopilot.chat'
  | 'githubCopilot.inline'
  | 'codex.primary'
  | 'codex.weekly'
  | 'claude.fiveHour'
  | 'claude.weekly'
  | 'claude.weeklySonnet'
  | 'claude.extraUsage'
  | 'antigravity.gemini'
  | 'antigravity.geminiWeekly'
  | 'antigravity.claude'
  | 'antigravity.claudeWeekly'
  | 'kiro.promptCredits';

export interface QuotaConfiguration {
  dataSource: QuotaDataSource;
  summaryPath: string;
  enabledProviders: ProviderId[];
  statusBarEnabled: boolean;
  statusBarItems: TrackId[];
  statusBarDisplay: StatusBarDisplayMode;
  statusBarMaxItems: number;
  refreshIntervalSeconds: number;
}

export interface QuotaTrack {
  id: TrackId;
  providerId: ProviderId;
  providerLabel: string;
  label: string;
  accountLabel: string;
  percentUsed?: number;
  percentRemaining?: number;
  resetAt?: number | null;
  updatedAt?: number | null;
  error?: string | null;
}

export interface QuotaSnapshot {
  sourcePath: string;
  exportedAt?: string;
  tracks: QuotaTrack[];
  warnings: string[];
}
