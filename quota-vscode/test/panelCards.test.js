const assert = require('node:assert/strict');
const test = require('node:test');

const { buildAccountCards, renderAccountCard } = require('../out/panelCards');

const config = {
  enabledProviders: ['githubCopilot', 'codex', 'claude', 'antigravity', 'kiro', 'grok', 'opencodeGo'],
  statusBarDisplay: 'percentRemaining',
};

function track(id, accountId, extra = {}) {
  return {
    id,
    providerId: id.slice(0, id.indexOf('.')),
    providerLabel: id.startsWith('antigravity') ? 'Antigravity' : 'Codex',
    label: id,
    accountId,
    accountLabel: `${accountId}@example.com`,
    updatedAt: 1_000,
    ...extra,
  };
}

test('buildAccountCards groups every track for one account into a single card', () => {
  const cards = buildAccountCards({
    sourcePath: 'test',
    warnings: [],
    tracks: [
      track('antigravity.claudeWeekly', 'agy', { label: 'Claude/GPT models weekly', percentRemaining: 51 }),
      track('antigravity.gemini', 'agy', { label: 'Gemini Models', percentRemaining: 100 }),
      track('antigravity.credits', 'agy', { label: 'Available AI Credits', valueLabel: '1,250', updatedAt: 2_000 }),
      track('antigravity.geminiWeekly', 'agy', { label: 'Gemini Models weekly', percentRemaining: 82 }),
      track('antigravity.claude', 'agy', { label: 'Claude/GPT models', percentRemaining: 5 }),
      track('codex.primary', 'work', { percentRemaining: 40 }),
      track('codex.primary', 'personal', { percentRemaining: 90 }),
    ],
  }, config);

  assert.deepEqual(cards.map((card) => card.key), ['codex:work', 'codex:personal', 'antigravity:agy']);
  const agy = cards[2];
  assert.deepEqual(agy.rows.map((row) => row.label), ['Gemini 5h', 'Gemini Weekly', 'Claude/GPT 5h', 'Claude/GPT Weekly']);
  assert.deepEqual(agy.stats, [{ id: 'antigravity.credits', label: 'AI credits', value: '1,250' }]);
  assert.equal(agy.rows[2].tone, 'danger');
  assert.equal(agy.rows[0].percentLabel, '100% left');
});

test('account cards show each distinct account error once and escape HTML', () => {
  const [card] = buildAccountCards({
    sourcePath: 'test',
    warnings: [],
    tracks: [
      track('codex.primary', 'work', { accountLabel: '<b>x</b>', error: 'Token expired' }),
      track('codex.weekly', 'work', { accountLabel: '<b>x</b>', error: 'Token expired' }),
    ],
  }, config);

  assert.deepEqual(card.errors, ['Token expired']);
  const html = renderAccountCard(card);
  assert.ok(html.includes('&lt;b&gt;x&lt;/b&gt;'));
  assert.equal(html.match(/Token expired/g).length, 1);
});

test('disabled providers do not produce cards', () => {
  const cards = buildAccountCards({
    sourcePath: 'test',
    warnings: [],
    tracks: [track('codex.primary', 'work', { percentRemaining: 40 })],
  }, { ...config, enabledProviders: ['claude'] });
  assert.equal(cards.length, 0);
});
