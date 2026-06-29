const assert = require('node:assert/strict');
const test = require('node:test');

const {
  buildAntigravityCreditsTrack,
  parseAntigravityLoadStatus,
} = require('../out/antigravityUsage');

test('parseAntigravityLoadStatus maps paid tier AI credits', () => {
  const status = parseAntigravityLoadStatus({
    cloudaicompanionProject: 'project-123',
    paidTier: {
      id: 'g1-pro-tier',
      name: 'Pro',
      availableCredits: [
        {
          creditType: 'GOOGLE_ONE_AI',
          creditAmount: '25,000',
          minimumCreditAmountForUsage: '50',
        },
        {
          creditType: 'IGNORED_WITHOUT_AMOUNT',
        },
      ],
    },
  });

  assert.equal(status.projectId, 'project-123');
  assert.equal(status.tierId, 'g1-pro-tier');
  assert.equal(status.tierName, 'Pro');
  assert.equal(status.credits.length, 1);
  assert.equal(status.credits[0].creditType, 'GOOGLE_ONE_AI');
  assert.equal(status.credits[0].creditAmount, '25,000');
  assert.equal(status.credits[0].minimumCreditAmountForUsage, '50');
});

test('buildAntigravityCreditsTrack returns a value label for valid credit amounts', () => {
  const track = buildAntigravityCreditsTrack(
    {
      id: 'antigravity_1',
      email: 'sizzlebop@example.com',
      name: 'Sizzle',
      credits: [
        {
          creditType: 'GOOGLE_ONE_AI',
          creditAmount: '25,000',
        },
        {
          creditType: 'GOOGLE_ONE_AI',
          creditAmount: '0.5',
        },
      ],
      quotaQueryLastError: null,
      usageUpdatedAt: 1782907200000,
    },
  );

  assert.ok(track);
  assert.equal(track.id, 'antigravity.credits');
  assert.equal(track.label, 'Available AI Credits');
  assert.equal(track.valueLabel, '25,000.5');
  assert.equal(track.accountLabel, 'Sizzle (sizzlebop@example.com)');
  assert.equal(track.updatedAt, 1782907200000);
});

test('buildAntigravityCreditsTrack is hidden without a valid amount', () => {
  const track = buildAntigravityCreditsTrack({
    id: 'antigravity_1',
    email: 'sizzlebop@example.com',
    credits: [
      {
        creditType: 'GOOGLE_ONE_AI',
      },
    ],
  });

  assert.equal(track, undefined);
});
