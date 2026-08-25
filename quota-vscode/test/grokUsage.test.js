const assert = require('node:assert/strict');
const test = require('node:test');

const {
  buildGrokUsageSummary,
  humanizeGrokPeriod,
  humanizeGrokTier,
  parseGrokTimestamp,
} = require('../out/grokUsage');

test('buildGrokUsageSummary maps the credit window and monthly history', () => {
  const summary = buildGrokUsageSummary(
    {
      config: {
        currentPeriod: {
          type: 'USAGE_PERIOD_TYPE_WEEKLY',
          start: '2026-08-24T21:44:00Z',
          end: '2026-08-31T21:44:00Z',
        },
        creditUsagePercent: 8,
        productUsage: [
          { product: 'grok-code', usagePercent: 8 },
          { product: 'grok-4', usagePercent: 2 },
        ],
        onDemandCap: { val: 50 },
        onDemandUsed: { val: 12.5 },
        prepaidBalance: { val: 3.25 },
        subscriptionTier: 'SUBSCRIPTION_TIER_X_PREMIUM_PLUS',
      },
    },
    {
      config: {
        used: { val: 21.75 },
        monthlyLimit: { val: 100 },
        billingPeriodStart: '2026-08-01T00:00:00Z',
        billingPeriodEnd: '2026-09-01T00:00:00Z',
      },
    },
  );

  assert.equal(summary.plan, 'X Premium Plus');
  assert.equal(summary.creditUsedPercent, 8);
  assert.equal(summary.creditRemainingPercent, 92);
  assert.equal(summary.periodLabel, 'Weekly');
  assert.equal(summary.periodStartAt, Date.parse('2026-08-24T21:44:00Z'));
  assert.equal(summary.periodResetAt, Date.parse('2026-08-31T21:44:00Z'));
  assert.equal(summary.monthlyUsed, 21.75);
  assert.equal(summary.monthlyLimit, 100);
  assert.equal(summary.monthlyPeriodEndAt, Date.parse('2026-09-01T00:00:00Z'));
  assert.equal(summary.onDemandUsed, 12.5);
  assert.equal(summary.onDemandCap, 50);
  assert.equal(summary.prepaidBalance, 3.25);
  assert.deepEqual(summary.productUsage, [
    { product: 'grok-code', usedPercent: 8, remainingPercent: 92 },
    { product: 'grok-4', usedPercent: 2, remainingPercent: 98 },
  ]);
});

test('buildGrokUsageSummary derives credit usage from the highest product usage', () => {
  const summary = buildGrokUsageSummary({
    config: {
      productUsage: [
        { product: 'grok-code', usagePercent: 15 },
        { product: 'grok-4', usagePercent: 42 },
      ],
    },
  });

  assert.equal(summary.creditUsedPercent, 42);
  assert.equal(summary.creditRemainingPercent, 58);
  assert.equal(summary.monthlyLimit, undefined);
  assert.equal(summary.onDemandCap, undefined);
});

test('buildGrokUsageSummary drops zero monthly limits and on-demand caps', () => {
  const summary = buildGrokUsageSummary(
    { config: { onDemandCap: { val: 0 }, onDemandUsed: { val: 0 } } },
    { config: { monthlyLimit: { val: 0 }, used: { val: 0 } } },
  );

  assert.equal(summary.monthlyLimit, undefined);
  assert.equal(summary.onDemandCap, undefined);
  assert.deepEqual(summary.productUsage, []);
});

test('buildGrokUsageSummary accepts unwrapped billing configs', () => {
  const summary = buildGrokUsageSummary({ creditUsagePercent: 30 });
  assert.equal(summary.creditRemainingPercent, 70);
});

test('humanizeGrokTier strips the enum prefix and keeps the X brand casing', () => {
  assert.equal(humanizeGrokTier('SUBSCRIPTION_TIER_X_PREMIUM_PLUS'), 'X Premium Plus');
  assert.equal(humanizeGrokTier('SUBSCRIPTION_TIER_BASIC'), 'Basic');
  assert.equal(humanizeGrokTier('  '), undefined);
});

test('humanizeGrokPeriod strips the usage period prefix', () => {
  assert.equal(humanizeGrokPeriod('USAGE_PERIOD_TYPE_WEEKLY'), 'Weekly');
  assert.equal(humanizeGrokPeriod('monthly'), 'Monthly');
});

test('parseGrokTimestamp accepts seconds, milliseconds, and RFC3339', () => {
  assert.equal(parseGrokTimestamp(1782907200), 1782907200000);
  assert.equal(parseGrokTimestamp(1782907200000), 1782907200000);
  assert.equal(parseGrokTimestamp('2026-07-01T12:00:00Z'), 1782907200000);
  assert.equal(parseGrokTimestamp(0), undefined);
  assert.equal(parseGrokTimestamp('not a date'), undefined);
});
