// @env node
import * as vscode from 'vscode';

import { DEFAULT_SUMMARY_PATH, PROVIDER_ORDER } from './constants';
import type { ProviderId, QuotaConfiguration, QuotaDataSource, StatusBarDisplayMode, TrackId } from './types';

const TRACK_IDS: TrackId[] = [
  'githubCopilot.premium',
  'githubCopilot.chat',
  'githubCopilot.inline',
  'codex.primary',
  'codex.weekly',
  'claude.fiveHour',
  'claude.weekly',
  'claude.weeklySonnet',
  'claude.extraUsage',
  'antigravity.gemini',
  'antigravity.geminiWeekly',
  'antigravity.claude',
  'antigravity.claudeWeekly',
  'kiro.promptCredits',
];

function isProviderId(value: string): value is ProviderId {
  return PROVIDER_ORDER.includes(value as ProviderId);
}

function isTrackId(value: string): value is TrackId {
  return TRACK_IDS.includes(value as TrackId);
}

function readStringArray(section: vscode.WorkspaceConfiguration, key: string): string[] {
  const value = section.get<unknown>(key);
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
}

export function readConfiguration(): QuotaConfiguration {
  const section = vscode.workspace.getConfiguration('quota');
  const dataSource = section.get<QuotaDataSource>('dataSource', 'extensionAccounts');
  const summaryPath = section.get<string>('summaryPath', '').trim() || DEFAULT_SUMMARY_PATH;
  const enabledProviders = readStringArray(section, 'providers.enabled').filter(isProviderId);
  const statusBarItems = readStringArray(section, 'statusBar.items').filter(isTrackId);
  const statusBarDisplay = section.get<StatusBarDisplayMode>('statusBar.display', 'percentUsed');

  return {
    dataSource,
    summaryPath,
    enabledProviders: enabledProviders.length > 0 ? enabledProviders : PROVIDER_ORDER,
    statusBarEnabled: section.get<boolean>('statusBar.enabled', true),
    statusBarItems,
    statusBarDisplay,
    statusBarMaxItems: section.get<number>('statusBar.maxItems', 3),
    refreshIntervalSeconds: section.get<number>('refresh.intervalSeconds', 120),
  };
}
