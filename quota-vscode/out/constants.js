"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.TRACK_STATUS_BAR_LABEL = exports.TRACK_LABELS = exports.CANONICAL_TRACK_ORDER = exports.PROVIDER_ORDER = exports.PROVIDER_LABELS = exports.DEFAULT_SUMMARY_PATH = exports.EXTENSION_NAME = void 0;
exports.EXTENSION_NAME = 'Quota';
exports.DEFAULT_SUMMARY_PATH = '~/.quota/safe-summary.json';
exports.PROVIDER_LABELS = {
    githubCopilot: 'GitHub Copilot',
    codex: 'Codex',
    claude: 'Claude Code',
    antigravity: 'Antigravity',
    kiro: 'Kiro',
};
exports.PROVIDER_ORDER = [
    'githubCopilot',
    'codex',
    'claude',
    'antigravity',
    'kiro',
];
exports.CANONICAL_TRACK_ORDER = [
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
exports.TRACK_LABELS = {
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
exports.TRACK_STATUS_BAR_LABEL = {
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
//# sourceMappingURL=constants.js.map