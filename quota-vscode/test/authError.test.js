const assert = require('node:assert/strict');
const test = require('node:test');

const {
  isReauthenticationRequired,
  reauthenticationMessage,
  tokenRefreshErrorMessage,
} = require('../out/authError');

test('maps Claude invalid_grant refresh failures to reauthentication', () => {
  const body = JSON.stringify({
    error: 'invalid_grant',
    error_description: 'Refresh token expired',
  });
  const message = tokenRefreshErrorMessage('Claude Code', 400, body);

  assert.equal(message, reauthenticationMessage('Claude Code'));
  assert.equal(isReauthenticationRequired(message), true);
});

test('maps Codex unauthorized refresh failures to reauthentication without exposing the body', () => {
  const body = JSON.stringify({ error: 'invalid_token', secret: 'must-not-leak' });
  const message = tokenRefreshErrorMessage('Codex', 401, body);

  assert.equal(message, reauthenticationMessage('Codex'));
  assert.equal(message.includes('must-not-leak'), false);
});

test('keeps temporary refresh failures diagnostic and redacted', () => {
  const body = '<html>temporary upstream failure</html>';
  const message = tokenRefreshErrorMessage('Codex', 503, body);

  assert.equal(message, `Codex token refresh returned 503 with body length ${body.length}.`);
  assert.equal(isReauthenticationRequired(message), false);
  assert.equal(message.includes(body), false);
});
