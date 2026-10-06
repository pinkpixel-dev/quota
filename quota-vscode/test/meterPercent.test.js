const assert = require('node:assert/strict');
const test = require('node:test');

const { meterPercent } = require('../out/format');

function track(percentages = {}) {
  return {
    id: 'opencodeGo.weekly',
    providerId: 'opencodeGo',
    providerLabel: 'OpenCode Go',
    label: 'Weekly usage',
    accountLabel: 'Personal',
    ...percentages,
  };
}

test('fills the panel meter by percent used or remaining, following the setting', () => {
  const weekly = track({ percentUsed: 30, percentRemaining: 70 });

  assert.equal(meterPercent(weekly, 'percentUsed'), 30);
  assert.equal(meterPercent(weekly, 'percentRemaining'), 70);
});

test('derives the missing side and clamps out-of-range values', () => {
  assert.equal(meterPercent(track({ percentUsed: 12.6 }), 'percentRemaining'), 87);
  assert.equal(meterPercent(track({ percentRemaining: -4 }), 'percentRemaining'), 0);
  assert.equal(meterPercent(track({ percentUsed: 104 }), 'percentUsed'), 100);
});

test('leaves the meter empty when a track has no percent', () => {
  assert.equal(meterPercent(track(), 'percentUsed'), 0);
  assert.equal(meterPercent(track(), 'percentRemaining'), 0);
});
