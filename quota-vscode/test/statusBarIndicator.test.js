const assert = require('node:assert/strict');
const test = require('node:test');

const { statusBarIndicator } = require('../out/format');

function track(percentages = {}) {
  return {
    id: 'codex.primary',
    providerId: 'codex',
    providerLabel: 'Codex',
    label: 'Weekly usage',
    accountLabel: 'sizzlebop@example.com',
    ...percentages,
  };
}

test('statusBarIndicator uses the extension panel usage thresholds', () => {
  assert.equal(statusBarIndicator(track({ percentUsed: 0 })), '🟢');
  assert.equal(statusBarIndicator(track({ percentUsed: 69 })), '🟢');
  assert.equal(statusBarIndicator(track({ percentUsed: 70 })), '🟡');
  assert.equal(statusBarIndicator(track({ percentUsed: 89 })), '🟡');
  assert.equal(statusBarIndicator(track({ percentUsed: 90 })), '🔴');
  assert.equal(statusBarIndicator(track({ percentUsed: 100 })), '🔴');
});

test('statusBarIndicator classifies remaining percentages directly', () => {
  assert.equal(statusBarIndicator(track({ percentRemaining: 31 })), '🟢');
  assert.equal(statusBarIndicator(track({ percentRemaining: 30 })), '🟡');
  assert.equal(statusBarIndicator(track({ percentRemaining: 11 })), '🟡');
  assert.equal(statusBarIndicator(track({ percentRemaining: 10 })), '🔴');
});

test('statusBarIndicator stays neutral without percentage data', () => {
  assert.equal(statusBarIndicator(track()), '⚪');
  assert.equal(statusBarIndicator(track({ valueLabel: '25,000' })), '⚪');
});
