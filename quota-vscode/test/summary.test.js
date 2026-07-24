const assert = require('node:assert/strict');
const test = require('node:test');

const { payloadToTracks } = require('../out/summary');

test('payloadToTracks emits only the current Codex primary window as weekly usage', () => {
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

  assert.equal(tracks.length, 1);
  assert.equal(tracks[0].id, 'codex.primary');
  assert.equal(tracks[0].label, 'Weekly usage');
  assert.equal(tracks[0].percentUsed, 28);
  assert.equal(tracks[0].percentRemaining, 72);
  assert.equal(tracks[0].resetAt, 1771736400000);
});
