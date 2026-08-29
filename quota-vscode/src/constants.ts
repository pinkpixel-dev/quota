import type { ProviderId, TrackId } from './types';

export const EXTENSION_NAME = 'Quota';

export const DEFAULT_SUMMARY_PATH = '~/.quota/safe-summary.json';

export const PROVIDER_LABELS: Record<ProviderId, string> = {
  githubCopilot: 'GitHub Copilot',
  codex: 'Codex',
  claude: 'Claude Code',
  antigravity: 'Antigravity',
  kiro: 'Kiro',
};

export const PROVIDER_ORDER: ProviderId[] = [
  'githubCopilot',
  'codex',
  'claude',
  'antigravity',
  'kiro',
];

export const CANONICAL_TRACK_ORDER: TrackId[] = [
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
  'antigravity.credits',
  'kiro.promptCredits',
];

export const TRACK_LABELS: Record<TrackId, string> = {
  'githubCopilot.premium': 'Premium requests',
  'githubCopilot.chat': 'Chat messages',
  'githubCopilot.inline': 'Inline suggestions',
  'codex.primary': '5h window',
  'codex.weekly': 'Weekly window',
  'claude.fiveHour': '5h usage',
  'claude.weekly': 'Weekly usage',
  'claude.weeklySonnet': 'Weekly Sonnet',
  'claude.extraUsage': 'Extra usage',
  'antigravity.gemini': 'Gemini models',
  'antigravity.geminiWeekly': 'Gemini models weekly',
  'antigravity.claude': 'Claude/GPT models',
  'antigravity.claudeWeekly': 'Claude/GPT models weekly',
  'antigravity.credits': 'Available AI Credits',
  'kiro.promptCredits': 'Prompt credits',
};

export const TRACK_STATUS_BAR_LABEL: Record<TrackId, string> = {
  'githubCopilot.premium': 'Copilot:Premium',
  'githubCopilot.chat': 'Copilot:Chat',
  'githubCopilot.inline': 'Copilot:Inline',
  'codex.primary': 'Codex:5h',
  'codex.weekly': 'Codex:Wk',
  'claude.fiveHour': 'Claude:5h',
  'claude.weekly': 'Claude:Wk',
  'claude.weeklySonnet': 'Claude:WkS',
  'claude.extraUsage': 'Claude:Xtra',
  'antigravity.gemini': 'Agy:Gemini:5h',
  'antigravity.geminiWeekly': 'Agy:Gemini:Wk',
  'antigravity.claude': 'Agy:Claude:5h',
  'antigravity.claudeWeekly': 'Agy:Claude:Wk',
  'antigravity.credits': 'Agy:Credits',
  'kiro.promptCredits': 'Kiro:Credits',
};
