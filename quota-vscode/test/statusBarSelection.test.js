const assert = require('node:assert/strict');
const test = require('node:test');

const {
  isTrackSelected,
  parseStatusBarEntry,
  resolveStatusBarTracks,
  shortAccountTag,
} = require('../out/statusBarSelection');

const ALL_PROVIDERS = ['githubCopilot', 'codex', 'claude', 'antigravity', 'kiro', 'grok', 'opencodeGo'];

function track(id, accountId, accountLabel, extra = {}) {
  return {
    id,
    providerId: id.slice(0, id.indexOf('.')),
    providerLabel: 'Provider',
    label: id,
    accountId,
    accountLabel,
    percentRemaining: 50,
    ...extra,
  };
}

const work = track('codex.weekly', 'codex_work', 'work@example.com');
const personal = track('codex.weekly', 'codex_personal', 'me@example.com');
const workFiveHour = track('codex.primary', 'codex_work', 'work@example.com');
const claude = track('claude.fiveHour', 'claude_a', 'Sizzle (sizzle@example.com)');

test('parseStatusBarEntry accepts bare and account-pinned track IDs', () => {
  assert.deepEqual(parseStatusBarEntry('codex.weekly'), { trackId: 'codex.weekly' });
  assert.deepEqual(parseStatusBarEntry('codex.weekly@codex_work'), { trackId: 'codex.weekly', accountId: 'codex_work' });
  assert.equal(parseStatusBarEntry('codex.nope'), undefined);
  assert.equal(parseStatusBarEntry('codex.weekly@'), undefined);
});

test('bare entries keep following the first connected account', () => {
  const resolved = resolveStatusBarTracks(['codex.weekly'], [work, personal], ALL_PROVIDERS, 3);
  assert.equal(resolved.length, 1);
  assert.equal(resolved[0].track, work);
});

test('pinned entries resolve each account and tag accounts when a provider has several', () => {
  const resolved = resolveStatusBarTracks(
    ['codex.weekly@codex_personal', 'codex.weekly@codex_work', 'claude.fiveHour@claude_a'],
    [work, workFiveHour, personal, claude],
    ALL_PROVIDERS,
    5,
  );
  assert.deepEqual(resolved.map((item) => item.key), [
    'codex.weekly@codex_work',
    'codex.weekly@codex_personal',
    'claude.fiveHour@claude_a',
  ]);
  assert.deepEqual(resolved.map((item) => item.accountTag), ['work', 'me', undefined]);
  assert.deepEqual(resolved.map((item) => item.priority), [89, 88, 87]);
});

test('missing accounts, disabled providers, duplicates, and the max item limit are respected', () => {
  const tracks = [work, personal, claude];
  assert.equal(resolveStatusBarTracks(['codex.weekly@gone'], tracks, ALL_PROVIDERS, 3).length, 0);
  assert.equal(resolveStatusBarTracks(['claude.fiveHour'], tracks, ['codex'], 3).length, 0);
  assert.equal(resolveStatusBarTracks(['codex.weekly', 'codex.weekly@codex_work'], tracks, ALL_PROVIDERS, 3).length, 1);
  assert.equal(resolveStatusBarTracks(['codex.weekly@codex_work', 'codex.weekly@codex_personal'], tracks, ALL_PROVIDERS, 1).length, 1);
});

test('isTrackSelected matches pinned entries and legacy bare entries', () => {
  const tracks = [work, personal];
  assert.equal(isTrackSelected(['codex.weekly'], work, tracks), true);
  assert.equal(isTrackSelected(['codex.weekly'], personal, tracks), false);
  assert.equal(isTrackSelected(['codex.weekly@codex_personal'], personal, tracks), true);
});

test('shortAccountTag prefers the email local part and truncates long names', () => {
  assert.equal(shortAccountTag('work@example.com'), 'work');
  assert.equal(shortAccountTag('Sizzle (sizzle@example.com)'), 'sizzle');
  assert.equal(shortAccountTag('a-really-long-account-name'), 'a-really-lo…');
});
