export function shouldSuppressClaudeError(message: string): boolean {
  const lower = message.toLowerCase();
  return lower.includes('claude usage returned 429')
    || lower.includes('claude usage is rate limited')
    || lower.includes('rate_limit');
}
