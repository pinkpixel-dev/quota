const assert = require('node:assert/strict');
const test = require('node:test');

const { shouldSuppressClaudeError } = require('../out/claudeUsage');

test('shouldSuppressClaudeError suppresses Claude usage rate limits', () => {
  assert.equal(shouldSuppressClaudeError('Claude usage returned 429 with body length 107.'), true);
});

test('shouldSuppressClaudeError keeps non-rate-limit errors visible', () => {
  assert.equal(shouldSuppressClaudeError('Claude token refresh failed.'), false);
});
