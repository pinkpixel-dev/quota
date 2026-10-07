// @env node
import * as vscode from 'vscode';

import { DEFAULT_SUMMARY_PATH, PROVIDER_ORDER } from './constants';
import { parseStatusBarEntry } from './statusBarSelection';
import type { ProviderId, QuotaConfiguration, QuotaDataSource, StatusBarDisplayMode } from './types';

function isProviderId(value: string): value is ProviderId {
  return PROVIDER_ORDER.includes(value as ProviderId);
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
  const statusBarItems = readStringArray(section, 'statusBar.items').filter((item) => parseStatusBarEntry(item) != null);
  const statusBarDisplay = section.get<StatusBarDisplayMode>('statusBar.display', 'percentRemaining');

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
