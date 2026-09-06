import assert from 'node:assert/strict';
import test from 'node:test';
import { buildTrayUsageRows } from '../src/data/trayRows.ts';

const order = ['githubCopilot', 'codex', 'antigravity', 'claude', 'kiro', 'cursor', 'grok'];

function emptyAccounts() {
  return {
    githubCopilot: [],
    codex: [],
    antigravity: [],
    claude: [],
    kiro: [],
    cursor: [],
    grok: [],
  };
}

test('builds compact rows in the saved provider order', () => {
  const accounts = emptyAccounts();
  accounts.codex.push({
    id: 'codex-1',
    email: 'dev@example.com',
    authMode: 'oauth',
    quota: { hourlyRemainingPercent: 82.4, weeklyRemainingPercent: 63.6 },
    requiresReauthentication: false,
    createdAt: 1,
    lastUsed: 1,
  });
  accounts.cursor.push({
    id: 'cursor-1',
    email: 'cursor@example.com',
    source: 'oauth',
    totalPercent: 41.2,
    autoPercent: 30,
    apiPercent: 11,
    onDemandEnabled: false,
    createdAt: 1,
    lastUsed: 1,
  });

  assert.deepEqual(buildTrayUsageRows(accounts, ['cursor', 'codex']), [
    'Cursor · cursor@example.com · Total 41% used · Auto 30% used · API 11% used · On-demand off',
    'Codex · dev@example.com · 5h 82% left · Week 64% left',
  ]);
});

test('clamps percentages and keeps count-based Copilot usage', () => {
  const accounts = emptyAccounts();
  accounts.githubCopilot.push({
    id: 'copilot-1',
    githubLogin: 'octocat',
    usage: {
      inlineSuggestionsUsedPercent: -5,
      chatMessagesUsedPercent: 104,
      premiumRequestsUsedPercent: null,
      inlineIncluded: false,
      chatIncluded: false,
      premiumIncluded: false,
      usedPremiumRequests: 12,
      totalPremiumRequests: 300,
    },
    createdAt: 1,
    lastUsed: 1,
  });

  assert.deepEqual(buildTrayUsageRows(accounts, order), [
    'Copilot · @octocat · Premium 12/300 used · Chat 100% used · Inline 0% used',
  ]);
});

test('keeps connected accounts visible when usage has not loaded', () => {
  const accounts = emptyAccounts();
  accounts.claude.push({
    id: 'claude-1',
    email: 'claude@example.com',
    authMode: 'oauth',
    quota: {},
    requiresReauthentication: true,
    createdAt: 1,
    lastUsed: 1,
  });

  assert.deepEqual(buildTrayUsageRows(accounts, order), [
    'Claude · claude@example.com · No usage data yet',
  ]);
});
