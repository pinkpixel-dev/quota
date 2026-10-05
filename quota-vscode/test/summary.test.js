const assert = require('node:assert/strict');
const test = require('node:test');

const { payloadToTracks } = require('../out/summary');

test('payloadToTracks emits Codex five-hour and weekly usage windows', () => {
  const tracks = payloadToTracks({
    providers: {
      codex: [
        {
          email: 'sizzlebop@example.com',
          quota: {
            hourlyRemainingPercent: 72,
            hourlyResetAt: 1771736400,
            weeklyRemainingPercent: 11,
            weeklyResetAt: 1772341200,
          },
        },
      ],
    },
  });

  assert.equal(tracks.length, 2);
  assert.equal(tracks[0].id, 'codex.primary');
  assert.equal(tracks[0].label, '5h usage');
  assert.equal(tracks[0].percentUsed, 28);
  assert.equal(tracks[0].percentRemaining, 72);
  assert.equal(tracks[0].resetAt, 1771736400000);
  assert.equal(tracks[1].id, 'codex.weekly');
  assert.equal(tracks[1].label, 'Weekly usage');
  assert.equal(tracks[1].percentUsed, 89);
  assert.equal(tracks[1].percentRemaining, 11);
  assert.equal(tracks[1].resetAt, 1772341200000);
});

test('payloadToTracks reads OpenCode Go windows from the desktop export', () => {
  const tracks = payloadToTracks({
    providers: {
      opencodeGo: [
        {
          label: 'Work',
          usage: {
            fiveHour: { usedPercent: 12, remainingPercent: 88, resetAt: 1790957937 },
            weekly: { usedPercent: 78, remainingPercent: 22, resetAt: null },
            monthly: { usedPercent: null, remainingPercent: null, resetAt: null },
          },
        },
      ],
    },
  });

  assert.equal(tracks.length, 2);
  assert.equal(tracks[0].id, 'opencodeGo.fiveHour');
  assert.equal(tracks[0].accountLabel, 'Work');
  assert.equal(tracks[0].percentUsed, 12);
  assert.equal(tracks[0].resetAt, 1790957937000);
  assert.equal(tracks[1].id, 'opencodeGo.weekly');
  assert.equal(tracks[1].percentRemaining, 22);
});
