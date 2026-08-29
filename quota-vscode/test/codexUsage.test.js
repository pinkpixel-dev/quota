const assert = require('node:assert/strict');
const test = require('node:test');

const { quotaFromUsage, tracksFromCodexAccount } = require('../out/codexUsage');

test('quotaFromUsage maps Codex primary and secondary windows', () => {
  const currentTime = 1771000000000;
  const result = quotaFromUsage({
    plan_type: 'plus',
    rate_limit: {
      primary_window: {
        used_percent: 28,
        limit_window_seconds: 18_000,
        reset_after_seconds: 900,
      },
      secondary_window: {
        used_percent: 89,
        limit_window_seconds: 604_800,
        reset_at: 1772341200,
      },
    },
  }, currentTime);

  assert.equal(result.plan, 'plus');
  assert.deepEqual(result.quota, {
    hourlyRemainingPercent: 72,
    hourlyResetAt: 1771000900000,
    hourlyWindowMinutes: 300,
    weeklyRemainingPercent: 11,
    weeklyResetAt: 1772341200000,
    weeklyWindowMinutes: 10_080,
  });
});

test('tracksFromCodexAccount emits five-hour and weekly tracks', () => {
  const tracks = tracksFromCodexAccount({
    email: 'sizzlebop@example.com',
    usageUpdatedAt: 1771000000000,
    quota: {
      hourlyRemainingPercent: 72,
      hourlyResetAt: 1771000900000,
      weeklyRemainingPercent: 11,
      weeklyResetAt: 1772341200000,
    },
  });

  assert.deepEqual(tracks.map((track) => ({
    id: track.id,
    label: track.label,
    percentUsed: track.percentUsed,
    percentRemaining: track.percentRemaining,
    resetAt: track.resetAt,
  })), [
    {
      id: 'codex.primary',
      label: '5h usage',
      percentUsed: 28,
      percentRemaining: 72,
      resetAt: 1771000900000,
    },
    {
      id: 'codex.weekly',
      label: 'Weekly usage',
      percentUsed: 89,
      percentRemaining: 11,
      resetAt: 1772341200000,
    },
  ]);
});
