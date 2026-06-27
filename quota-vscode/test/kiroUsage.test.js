const assert = require('node:assert/strict');
const test = require('node:test');

const {
  buildKiroUsageSummary,
  normalizeKiroPlan,
  parseKiroTimestamp,
} = require('../out/kiroUsage');

test('buildKiroUsageSummary maps nested Kiro usage state', () => {
  const summary = buildKiroUsageSummary({
    userInfo: {
      email: 'sizzlebop@example.com',
    },
    'kiro.resourceNotifications.usageState': {
      subscriptionInfo: {
        subscriptionName: 'pro',
      },
      usageBreakdownList: [
        {
          type: 'credit',
          usageLimitWithPrecision: 500,
          currentUsageWithPrecision: 125,
          resetDate: '2026-07-01T12:00:00Z',
          freeTrialUsage: {
            usageLimitWithPrecision: 100,
            currentUsageWithPrecision: 25,
            daysRemaining: 6,
          },
        },
      ],
    },
  });

  assert.equal(summary.email, 'sizzlebop@example.com');
  assert.equal(summary.planName, 'pro');
  assert.equal(summary.creditsTotal, 500);
  assert.equal(summary.creditsUsed, 125);
  assert.equal(summary.bonusTotal, 100);
  assert.equal(summary.bonusUsed, 25);
  assert.equal(summary.bonusExpireDays, 6);
  assert.equal(summary.usageResetAt, 1782907200000);
});

test('normalizeKiroPlan returns compact plan labels', () => {
  assert.equal(normalizeKiroPlan('Kiro Pro+ monthly'), 'PRO');
  assert.equal(normalizeKiroPlan('standalone free'), 'FREE');
});

test('parseKiroTimestamp accepts seconds, milliseconds, and RFC3339', () => {
  assert.equal(parseKiroTimestamp(1782907200), 1782907200000);
  assert.equal(parseKiroTimestamp(1782907200000), 1782907200000);
  assert.equal(parseKiroTimestamp('2026-07-01T12:00:00Z'), 1782907200000);
});
