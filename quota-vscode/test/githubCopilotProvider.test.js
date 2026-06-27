const assert = require('node:assert/strict');
const test = require('node:test');

const {
  buildCopilotUsageSummary,
  parseCopilotResetDate,
} = require('../out/githubCopilotUsage');

test('buildCopilotUsageSummary maps quota snapshots and date-only resets', () => {
  const summary = buildCopilotUsageSummary({
    copilotToken: 'tid=abc;rd=1798761600:unused',
    quotaSnapshots: {
      premium_interactions: {
        entitlement: 300,
        remaining: 75,
        percent_remaining: 25,
      },
      chat: {
        entitlement: 100,
        percent_remaining: 40,
      },
      completions: {
        unlimited: true,
      },
    },
    quotaResetDate: '2026-07-01',
  });

  assert.equal(summary.premiumRequestsUsedPercent, 75);
  assert.equal(summary.chatMessagesUsedPercent, 60);
  assert.equal(summary.inlineSuggestionsUsedPercent, 0);
  assert.equal(summary.allowanceResetAt, 1782864000000);
  assert.equal(summary.remainingPremiumRequests, 75);
  assert.equal(summary.totalPremiumRequests, 300);
  assert.equal(summary.usedPremiumRequests, 225);
});

test('parseCopilotResetDate accepts RFC3339 timestamps', () => {
  assert.equal(parseCopilotResetDate('2026-07-01T12:30:00Z'), 1782909000000);
});
