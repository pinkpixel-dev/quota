const assert = require('node:assert/strict');
const test = require('node:test');

const {
  maskOpenCodeGoKey,
  openCodeGoErrorMessage,
  parseOpenCodeGoUsage,
} = require('../out/opencodeGoUsage');

test('parseOpenCodeGoUsage maps the rolling, weekly, and monthly windows', () => {
  const summary = parseOpenCodeGoUsage({
    usage: {
      rolling: { status: 'ok', percent: 12, resetsAt: '2026-10-02T16:18:57.800Z' },
      weekly: { status: 'ok', percent: 78, resetsAt: '2026-10-05T00:00:00.000Z' },
      monthly: { status: 'ok', percent: 94, resetsAt: '2026-10-11T12:22:02.000Z' },
    },
  });

  assert.deepEqual(summary.rolling, { status: 'ok', percentUsed: 12, resetAt: Date.parse('2026-10-02T16:18:57.800Z') });
  assert.equal(summary.weekly.percentUsed, 78);
  assert.equal(summary.weekly.resetAt, Date.parse('2026-10-05T00:00:00.000Z'));
  assert.equal(summary.monthly.percentUsed, 94);
});

test('parseOpenCodeGoUsage keeps a window with no reset time and clamps percents', () => {
  const summary = parseOpenCodeGoUsage({
    usage: {
      rolling: { status: 'ok', percent: 0, resetsAt: null },
      weekly: { percent: 104 },
      monthly: { percent: -3, resetsAt: 'not a date' },
    },
  });

  assert.equal(summary.rolling.percentUsed, 0);
  assert.equal(summary.rolling.resetAt, undefined);
  assert.equal(summary.weekly.percentUsed, 100);
  assert.equal(summary.monthly.percentUsed, 0);
  assert.equal(summary.monthly.resetAt, undefined);
});

test('parseOpenCodeGoUsage throws instead of reading a bad body as 0% used', () => {
  assert.throws(() => parseOpenCodeGoUsage({}), /did not include usage windows/);
  assert.throws(() => parseOpenCodeGoUsage({ usage: {} }), /did not include usage windows/);
  assert.throws(() => parseOpenCodeGoUsage(null), /did not include usage windows/);
});

test('openCodeGoErrorMessage explains rejected keys and missing subscriptions', () => {
  assert.match(openCodeGoErrorMessage(401), /rejected this API key/);
  assert.match(openCodeGoErrorMessage(403), /no OpenCode Go subscription/);
  assert.equal(openCodeGoErrorMessage(500), 'OpenCode Go usage returned 500.');
});

test('maskOpenCodeGoKey shows only the last four characters', () => {
  assert.equal(maskOpenCodeGoKey('  sk-test-abcd1234  '), 'Go key ••••1234');
});

test('parseOpenCodeGoUsage treats a full unused rolling window as not started', () => {
  const now = Date.parse('2026-10-02T12:00:00.000Z');
  const summary = parseOpenCodeGoUsage({
    usage: { rolling: { status: 'ok', percent: 0, resetsAt: '2026-10-02T17:00:00.000Z' } },
  }, now);

  assert.equal(summary.rolling.startsOnFirstUse, true);
  assert.equal(summary.rolling.resetAt, undefined);
  assert.equal(summary.rolling.percentUsed, 0);
});

test('parseOpenCodeGoUsage keeps the reset for a rolling window that has started', () => {
  const now = Date.parse('2026-10-02T12:00:00.000Z');
  const used = parseOpenCodeGoUsage({
    usage: { rolling: { percent: 3, resetsAt: '2026-10-02T17:00:00.000Z' } },
  }, now);
  assert.equal(used.rolling.startsOnFirstUse, undefined);
  assert.equal(used.rolling.resetAt, Date.parse('2026-10-02T17:00:00.000Z'));

  const partial = parseOpenCodeGoUsage({
    usage: { rolling: { percent: 0, resetsAt: '2026-10-02T14:00:00.000Z' } },
  }, now);
  assert.equal(partial.rolling.startsOnFirstUse, undefined);
  assert.equal(partial.rolling.resetAt, Date.parse('2026-10-02T14:00:00.000Z'));
});
