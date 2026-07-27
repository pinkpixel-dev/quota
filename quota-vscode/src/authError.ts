export type ReauthenticationProvider = 'Codex' | 'Claude Code';

const REAUTHENTICATION_MESSAGES: Record<ReauthenticationProvider, string> = {
  Codex: 'Codex authorization expired. Reauthenticate to continue.',
  'Claude Code': 'Claude Code authorization expired. Reauthenticate to continue.',
};

function bodySignalsRejectedRefreshToken(body: string): boolean {
  const normalized = body.toLowerCase();
  return [
    'invalid_grant',
    'refresh token expired',
    'refresh_token_expired',
    'refresh token revoked',
    'refresh_token_revoked',
  ].some((signal) => normalized.includes(signal));
}

export function reauthenticationMessage(provider: ReauthenticationProvider): string {
  return REAUTHENTICATION_MESSAGES[provider];
}

export function isReauthenticationRequired(message: string | null | undefined): boolean {
  return message != null && Object.values(REAUTHENTICATION_MESSAGES).includes(message);
}

export function tokenRefreshErrorMessage(
  provider: ReauthenticationProvider,
  status: number,
  body: string,
): string {
  const isRejectedCredential =
    status === 401
    || status === 403
    || (status === 400 && bodySignalsRejectedRefreshToken(body));

  if (isRejectedCredential) return reauthenticationMessage(provider);
  return `${provider} token refresh returned ${status} with body length ${body.length}.`;
}
