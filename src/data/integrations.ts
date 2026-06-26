export type IntegrationStatus = 'planned' | 'reference' | 'blocked';

export interface IntegrationSummary {
  name: string;
  description: string;
  status: IntegrationStatus;
  referenceHint: string;
  iconPath: string;
}

export const integrations: IntegrationSummary[] = [
  {
    name: 'GitHub Copilot',
    description: 'Developer account status for Copilot-capable environments.',
    status: 'reference',
    referenceHint: 'First port target because it is testable locally and has a well-defined OAuth flow.',
    iconPath: '/brand-icons/githubcopilot.svg',
  },
  {
    name: 'Codex',
    description: 'OpenAI account state, local config awareness, and future model tooling.',
    status: 'reference',
    referenceHint: 'Study OAuth, account files, quota parsing, and local access boundaries.',
    iconPath: '/brand-icons/openai.svg',
  },
  {
    name: 'Antigravity',
    description: 'Google Antigravity account connection, local import, and quota windows.',
    status: 'reference',
    referenceHint: 'Connects through Google OAuth, with local Google/Gemini import kept as a fallback.',
    iconPath: '/brand-icons/antigravity.svg',
  },
  {
    name: 'Kiro',
    description: 'Kiro account connection, local import, and prompt credit usage.',
    status: 'reference',
    referenceHint: 'PKCE OAuth with browser redirect callback; usage from AWS runtime endpoint.',
    iconPath: '/brand-icons/kiro.svg',
  },
  {
    name: 'Cursor',
    description: 'Cursor account connection via browser OAuth or local SQLite import.',
    status: 'reference',
    referenceHint: 'Deep-link polling OAuth + local state.vscdb import; usage from cursor.com/api/usage-summary.',
    iconPath: '/brand-icons/cursor.svg',
  },

  {
    name: 'Claude',
    description: 'Claude Code OAuth account connection, profile details, and usage windows.',
    status: 'reference',
    referenceHint: 'Connects through Claude Code OAuth; Claude Desktop cookie import stays deferred.',
    iconPath: '/brand-icons/claude.svg',
  },
];
