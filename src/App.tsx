import { openUrl } from '@tauri-apps/plugin-opener';
import {
  ArrowLeft,
  ArrowDown,
  ArrowUp,
  Database,
  Download,
  Eye,
  EyeOff,
  ExternalLink,
  Github,
  Grid2X2,
  LockKeyhole,
  Palette,
  Pin,
  PinOff,
  Plus,
  RefreshCcw,
  Settings,
  Trash2,
  Users,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import {
  cancelAntigravityOAuthLogin,
  completeAntigravityOAuthLogin,
  deleteAntigravityAccount,
  importAntigravityFromLocal,
  listAntigravityAccounts,
  refreshAllAntigravityAccounts,
  refreshAntigravityAccount,
  startAntigravityOAuthLogin,
  type AntigravityAccountSummary,
  type AntigravityCreditInfo,
  type AntigravityOAuthStartResponse,
  type AntigravityQuotaWindow,
} from './data/antigravity';
import {
  cancelCodexOAuthLogin,
  completeCodexOAuthLogin,
  deleteCodexAccount,
  importCodexFromLocal,
  listCodexAccounts,
  refreshAllCodexAccounts,
  refreshCodexAccount,
  startCodexOAuthLogin,
  type CodexAccountSummary,
  type CodexOAuthStartResponse,
} from './data/codex';
import {
  cancelClaudeOAuthLogin,
  completeClaudeOAuthLogin,
  deleteClaudeAccount,
  listClaudeAccounts,
  refreshAllClaudeAccounts,
  refreshClaudeAccount,
  startClaudeOAuthLogin,
  type ClaudeAccountSummary,
  type ClaudeOAuthStartResponse,
  type ClaudeQuotaSummary,
} from './data/claude';
import {
  cancelKiroOAuthLogin,
  completeKiroOAuthLogin,
  deleteKiroAccount,
  getKiroPlanDisplayName,
  importKiroFromLocal,
  listKiroAccounts,
  refreshAllKiroAccounts,
  refreshKiroAccount,
  startKiroOAuthLogin,
  type KiroAccountSummary,
  type KiroOAuthStartResponse,
} from './data/kiro';
import {
  cancelCursorOAuthLogin,
  completeCursorOAuthLogin,
  deleteCursorAccount,
  getCursorPlanBadge,
  importCursorFromLocal,
  listCursorAccounts,
  refreshAllCursorAccounts,
  refreshCursorAccount,
  startCursorOAuthLogin,
  type CursorAccountSummary,
  type CursorOAuthStartResponse,
} from './data/cursor';
import {
  cancelGitHubCopilotLogin,
  completeGitHubCopilotLogin,
  deleteGitHubCopilotAccount,
  listGitHubCopilotAccounts,
  refreshAllGitHubCopilotAccounts,
  refreshGitHubCopilotAccount,
  startGitHubCopilotLogin,
  type GitHubCopilotAccountSummary,
  type GitHubCopilotOAuthStartResponse,
} from './data/githubCopilot';
import { integrations } from './data/integrations';

type AppView =
  | 'dashboard'
  | 'settings'
  | 'integrations'
  | 'github-copilot-accounts'
  | 'codex-accounts'
  | 'antigravity-accounts'
  | 'claude-accounts'
  | 'kiro-accounts'
  | 'cursor-accounts';

export type ViewMode = 'default' | 'compact' | 'list';
type ThemeMode = 'system' | 'dark' | 'light';
type ProviderKey = 'githubCopilot' | 'codex' | 'antigravity' | 'claude' | 'kiro' | 'cursor';

const GITHUB_COPILOT_ICON = '/brand-icons/githubcopilot.svg';
const CODEX_ICON = '/brand-icons/openai.svg';
const ANTIGRAVITY_ICON = '/brand-icons/antigravity.svg';
const CLAUDE_ICON = '/brand-icons/claude.svg';
const KIRO_ICON = '/brand-icons/kiro.svg';
const CURSOR_ICON = '/brand-icons/cursor.svg';
const DASHBOARD_VIEW_MODE_KEY = 'quota.dashboardViewMode';
const ACCOUNT_PAGES_VIEW_MODE_KEY = 'quota.accountPagesViewMode';
const THEME_MODE_KEY = 'quota.themeMode';
const PROVIDER_ORDER_KEY = 'quota.providerOrder';
const PINNED_ACCOUNTS_KEY = 'quota.pinnedAccounts';
const HIDDEN_PROVIDERS_KEY = 'quota.hiddenProviders';
const VIEW_MODES: ViewMode[] = ['default', 'compact', 'list'];
const THEME_MODES: ThemeMode[] = ['system', 'dark', 'light'];
const DEFAULT_PROVIDER_ORDER: ProviderKey[] = ['githubCopilot', 'codex', 'antigravity', 'claude', 'kiro', 'cursor'];
const PROVIDERS: Array<{ key: ProviderKey; name: string; iconPath: string }> = [
  { key: 'githubCopilot', name: 'GitHub Copilot', iconPath: GITHUB_COPILOT_ICON },
  { key: 'codex', name: 'Codex', iconPath: CODEX_ICON },
  { key: 'antigravity', name: 'Antigravity', iconPath: ANTIGRAVITY_ICON },
  { key: 'claude', name: 'Claude', iconPath: CLAUDE_ICON },
  { key: 'kiro', name: 'Kiro', iconPath: KIRO_ICON },
  { key: 'cursor', name: 'Cursor', iconPath: CURSOR_ICON },
];

function readStoredViewMode(key: string): ViewMode {
  try {
    const stored = window.localStorage.getItem(key);
    return VIEW_MODES.includes(stored as ViewMode) ? (stored as ViewMode) : 'default';
  } catch {
    return 'default';
  }
}

function storeViewMode(key: string, value: ViewMode) {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // Preference persistence is best-effort so private browsing/storage errors do not break the UI.
  }
}

function readStoredThemeMode(): ThemeMode {
  try {
    const stored = window.localStorage.getItem(THEME_MODE_KEY);
    return THEME_MODES.includes(stored as ThemeMode) ? (stored as ThemeMode) : 'system';
  } catch {
    return 'system';
  }
}

function storeThemeMode(value: ThemeMode) {
  try {
    window.localStorage.setItem(THEME_MODE_KEY, value);
  } catch {
    // Preference persistence is best-effort so private browsing/storage errors do not break the UI.
  }
}

function resolveThemeMode(mode: ThemeMode, prefersDark: boolean) {
  return mode === 'system' ? (prefersDark ? 'dark' : 'light') : mode;
}

function readStoredProviderOrder(): ProviderKey[] {
  try {
    const stored = JSON.parse(window.localStorage.getItem(PROVIDER_ORDER_KEY) ?? '[]') as unknown;
    if (!Array.isArray(stored)) return DEFAULT_PROVIDER_ORDER;
    const validStored = stored.filter((item): item is ProviderKey => DEFAULT_PROVIDER_ORDER.includes(item as ProviderKey));
    const missing = DEFAULT_PROVIDER_ORDER.filter((item) => !validStored.includes(item));
    return [...validStored, ...missing];
  } catch {
    return DEFAULT_PROVIDER_ORDER;
  }
}

function storeProviderOrder(value: ProviderKey[]) {
  try {
    window.localStorage.setItem(PROVIDER_ORDER_KEY, JSON.stringify(value));
  } catch {
    // Preference persistence is best-effort so private browsing/storage errors do not break the UI.
  }
}

function moveProvider(order: ProviderKey[], provider: ProviderKey, direction: -1 | 1) {
  const currentIndex = order.indexOf(provider);
  const nextIndex = currentIndex + direction;
  if (currentIndex < 0 || nextIndex < 0 || nextIndex >= order.length) return order;

  const nextOrder = [...order];
  const [item] = nextOrder.splice(currentIndex, 1);
  nextOrder.splice(nextIndex, 0, item);
  return nextOrder;
}

function getProviderOrderIndex(order: ProviderKey[], provider: ProviderKey) {
  const index = order.indexOf(provider);
  return index === -1 ? DEFAULT_PROVIDER_ORDER.indexOf(provider) : index;
}

function readStoredPinnedAccounts(): Set<string> {
  try {
    const stored = JSON.parse(window.localStorage.getItem(PINNED_ACCOUNTS_KEY) ?? '[]') as unknown;
    if (!Array.isArray(stored)) return new Set();
    return new Set(stored.filter((item): item is string => typeof item === 'string'));
  } catch {
    return new Set();
  }
}

function storePinnedAccounts(value: Set<string>) {
  try {
    window.localStorage.setItem(PINNED_ACCOUNTS_KEY, JSON.stringify([...value]));
  } catch {
    // best-effort
  }
}

function readStoredHiddenProviders(): Set<ProviderKey> {
  try {
    const stored = JSON.parse(window.localStorage.getItem(HIDDEN_PROVIDERS_KEY) ?? '[]') as unknown;
    if (!Array.isArray(stored)) return new Set();
    return new Set(stored.filter((item): item is ProviderKey => DEFAULT_PROVIDER_ORDER.includes(item as ProviderKey)));
  } catch {
    return new Set();
  }
}

function storeHiddenProviders(value: Set<ProviderKey>) {
  try {
    window.localStorage.setItem(HIDDEN_PROVIDERS_KEY, JSON.stringify([...value]));
  } catch {
    // best-effort
  }
}

function getVisibleAccounts<T extends { id: string }>(accounts: T[], pinnedAccounts: Set<string>): T[] {
  const pinned = accounts.filter((a) => pinnedAccounts.has(a.id));
  return pinned.length > 0 ? pinned : accounts.slice(0, 2);
}

export function App() {
  const [view, setView] = useState<AppView>('dashboard');
  const [themeMode, setThemeMode] = useState<ThemeMode>(() => readStoredThemeMode());
  const [dashboardViewMode, setDashboardViewMode] = useState<ViewMode>(() => readStoredViewMode(DASHBOARD_VIEW_MODE_KEY));
  const [accountPagesViewMode, setAccountPagesViewMode] = useState<ViewMode>(() => readStoredViewMode(ACCOUNT_PAGES_VIEW_MODE_KEY));
  const [providerOrder, setProviderOrder] = useState<ProviderKey[]>(() => readStoredProviderOrder());
  const [pinnedAccounts, setPinnedAccounts] = useState<Set<string>>(() => readStoredPinnedAccounts());
  const [hiddenProviders, setHiddenProviders] = useState<Set<ProviderKey>>(() => readStoredHiddenProviders());
  const [copilotAccounts, setCopilotAccounts] = useState<GitHubCopilotAccountSummary[]>([]);
  const [codexAccounts, setCodexAccounts] = useState<CodexAccountSummary[]>([]);
  const [antigravityAccounts, setAntigravityAccounts] = useState<AntigravityAccountSummary[]>([]);
  const [claudeAccounts, setClaudeAccounts] = useState<ClaudeAccountSummary[]>([]);
  const [kiroAccounts, setKiroAccounts] = useState<KiroAccountSummary[]>([]);
  const [cursorAccounts, setCursorAccounts] = useState<CursorAccountSummary[]>([]);
  const [copilotLogin, setCopilotLogin] = useState<GitHubCopilotOAuthStartResponse | null>(null);
  const [codexLogin, setCodexLogin] = useState<CodexOAuthStartResponse | null>(null);
  const [antigravityLogin, setAntigravityLogin] = useState<AntigravityOAuthStartResponse | null>(null);
  const [claudeLogin, setClaudeLogin] = useState<ClaudeOAuthStartResponse | null>(null);
  const [kiroLogin, setKiroLogin] = useState<KiroOAuthStartResponse | null>(null);
  const [cursorLogin, setCursorLogin] = useState<CursorOAuthStartResponse | null>(null);
  const [claudeCallbackInput, setClaudeCallbackInput] = useState('');
  const [claudeEmailHint, setClaudeEmailHint] = useState('');
  const [copilotBusy, setCopilotBusy] = useState(false);
  const [codexBusy, setCodexBusy] = useState(false);
  const [antigravityBusy, setAntigravityBusy] = useState(false);
  const [claudeBusy, setClaudeBusy] = useState(false);
  const [kiroBusy, setKiroBusy] = useState(false);
  const [cursorBusy, setCursorBusy] = useState(false);
  const [copilotError, setCopilotError] = useState<string | null>(null);
  const [codexError, setCodexError] = useState<string | null>(null);
  const [antigravityError, setAntigravityError] = useState<string | null>(null);
  const [claudeError, setClaudeError] = useState<string | null>(null);
  const [kiroError, setKiroError] = useState<string | null>(null);
  const [cursorError, setCursorError] = useState<string | null>(null);

  const connectedCount =
    copilotAccounts.length + codexAccounts.length + antigravityAccounts.length + claudeAccounts.length + kiroAccounts.length + cursorAccounts.length;

  useEffect(() => {
    void loadCopilotAccounts();
    void loadCodexAccounts();
    void loadAntigravityAccounts();
    void loadClaudeAccounts();
    void loadKiroAccounts();
    void loadCursorAccounts();
  }, []);

  useEffect(() => {
    storeViewMode(DASHBOARD_VIEW_MODE_KEY, dashboardViewMode);
  }, [dashboardViewMode]);

  useEffect(() => {
    storeViewMode(ACCOUNT_PAGES_VIEW_MODE_KEY, accountPagesViewMode);
  }, [accountPagesViewMode]);

  useEffect(() => {
    storeProviderOrder(providerOrder);
  }, [providerOrder]);

  useEffect(() => {
    storePinnedAccounts(pinnedAccounts);
  }, [pinnedAccounts]);

  useEffect(() => {
    storeHiddenProviders(hiddenProviders);
  }, [hiddenProviders]);

  useEffect(() => {
    storeThemeMode(themeMode);

    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const applyTheme = () => {
      document.documentElement.dataset.theme = resolveThemeMode(themeMode, media.matches);
    };

    applyTheme();

    if (themeMode !== 'system') return undefined;

    media.addEventListener('change', applyTheme);
    return () => media.removeEventListener('change', applyTheme);
  }, [themeMode]);

  async function loadCopilotAccounts() {
    try {
      setCopilotAccounts(await listGitHubCopilotAccounts());
      setCopilotError(null);
    } catch (error) {
      setCopilotError(error instanceof Error ? error.message : String(error));
    }
  }

  async function loadCodexAccounts() {
    try {
      setCodexAccounts(await listCodexAccounts());
      setCodexError(null);
    } catch (error) {
      setCodexError(error instanceof Error ? error.message : String(error));
    }
  }

  async function loadAntigravityAccounts() {
    try {
      setAntigravityAccounts(await listAntigravityAccounts());
      setAntigravityError(null);
    } catch (error) {
      setAntigravityError(error instanceof Error ? error.message : String(error));
    }
  }

  async function loadClaudeAccounts() {
    try {
      setClaudeAccounts(await listClaudeAccounts());
      setClaudeError(null);
    } catch (error) {
      setClaudeError(error instanceof Error ? error.message : String(error));
    }
  }

  async function loadKiroAccounts() {
    try {
      setKiroAccounts(await listKiroAccounts());
      setKiroError(null);
    } catch (error) {
      setKiroError(error instanceof Error ? error.message : String(error));
    }
  }

  async function loadCursorAccounts() {
    try {
      setCursorAccounts(await listCursorAccounts());
      setCursorError(null);
    } catch (error) {
      setCursorError(error instanceof Error ? error.message : String(error));
    }
  }

  async function startCopilotAuth() {
    setCopilotBusy(true);
    try {
      setCopilotLogin(await startGitHubCopilotLogin());
      setView('integrations');
      setCopilotError(null);
    } catch (error) {
      setCopilotError(error instanceof Error ? error.message : String(error));
    } finally {
      setCopilotBusy(false);
    }
  }

  async function completeCopilotAuth() {
    if (!copilotLogin) return;
    setCopilotBusy(true);
    try {
      const account = await completeGitHubCopilotLogin(copilotLogin.loginId);
      setCopilotAccounts((accounts) => [account, ...accounts.filter((item) => item.id !== account.id)]);
      setCopilotLogin(null);
      setView('dashboard');
      setCopilotError(null);
    } catch (error) {
      setCopilotError(error instanceof Error ? error.message : String(error));
    } finally {
      setCopilotBusy(false);
    }
  }

  async function cancelCopilotAuth() {
    setCopilotBusy(true);
    try {
      await cancelGitHubCopilotLogin(copilotLogin?.loginId);
      setCopilotLogin(null);
      setCopilotError(null);
    } catch (error) {
      setCopilotError(error instanceof Error ? error.message : String(error));
    } finally {
      setCopilotBusy(false);
    }
  }

  async function openCopilotAuthUrl() {
    if (!copilotLogin) return;
    const url = copilotLogin.verificationUriComplete ?? copilotLogin.verificationUri;

    try {
      await openUrl(url);
      setCopilotError(null);
    } catch (error) {
      window.open(url, '_blank', 'noopener,noreferrer');
      setCopilotError(error instanceof Error ? `Opened with browser fallback. Tauri opener said: ${error.message}` : null);
    }
  }

  async function refreshCopilotAccount(accountId: string) {
    setCopilotBusy(true);
    try {
      const account = await refreshGitHubCopilotAccount(accountId);
      setCopilotAccounts((accounts) => accounts.map((item) => (item.id === account.id ? account : item)));
      setCopilotError(null);
    } catch (error) {
      setCopilotError(error instanceof Error ? error.message : String(error));
    } finally {
      setCopilotBusy(false);
    }
  }

  async function refreshAllCopilot() {
    setCopilotBusy(true);
    try {
      const accounts = await refreshAllGitHubCopilotAccounts();
      setCopilotAccounts(accounts);
      setCopilotError(null);
    } catch (error) {
      setCopilotError(error instanceof Error ? error.message : String(error));
    } finally {
      setCopilotBusy(false);
    }
  }

  async function removeCopilotAccount(accountId: string) {
    setCopilotBusy(true);
    try {
      await deleteGitHubCopilotAccount(accountId);
      setCopilotAccounts((accounts) => accounts.filter((item) => item.id !== accountId));
      setCopilotError(null);
    } catch (error) {
      setCopilotError(error instanceof Error ? error.message : String(error));
    } finally {
      setCopilotBusy(false);
    }
  }

  async function importLocalCodex() {
    setCodexBusy(true);
    try {
      const account = await importCodexFromLocal();
      setCodexAccounts((accounts) => [account, ...accounts.filter((item) => item.id !== account.id)]);
      setView('dashboard');
      setCodexError(null);
    } catch (error) {
      setCodexError(error instanceof Error ? error.message : String(error));
    } finally {
      setCodexBusy(false);
    }
  }

  async function startCodexAuth() {
    setCodexBusy(true);
    try {
      setCodexLogin(await startCodexOAuthLogin());
      setView('integrations');
      setCodexError(null);
    } catch (error) {
      setCodexError(error instanceof Error ? error.message : String(error));
    } finally {
      setCodexBusy(false);
    }
  }

  async function openCodexAuthUrl() {
    if (!codexLogin) return;

    try {
      await openUrl(codexLogin.authUrl);
      setCodexError(null);
    } catch (error) {
      window.open(codexLogin.authUrl, '_blank', 'noopener,noreferrer');
      setCodexError(error instanceof Error ? `Opened with browser fallback. Tauri opener said: ${error.message}` : null);
    }
  }

  async function completeCodexAuth() {
    if (!codexLogin) return;
    setCodexBusy(true);
    try {
      const account = await completeCodexOAuthLogin(codexLogin.loginId);
      setCodexAccounts((accounts) => [account, ...accounts.filter((item) => item.id !== account.id)]);
      setCodexLogin(null);
      setView('dashboard');
      setCodexError(null);
    } catch (error) {
      setCodexError(error instanceof Error ? error.message : String(error));
    } finally {
      setCodexBusy(false);
    }
  }

  async function cancelCodexAuth() {
    setCodexBusy(true);
    try {
      await cancelCodexOAuthLogin(codexLogin?.loginId);
      setCodexLogin(null);
      setCodexError(null);
    } catch (error) {
      setCodexError(error instanceof Error ? error.message : String(error));
    } finally {
      setCodexBusy(false);
    }
  }

  async function refreshCodex(accountId: string) {
    setCodexBusy(true);
    try {
      const account = await refreshCodexAccount(accountId);
      setCodexAccounts((accounts) => accounts.map((item) => (item.id === account.id ? account : item)));
      setCodexError(null);
    } catch (error) {
      setCodexError(error instanceof Error ? error.message : String(error));
    } finally {
      setCodexBusy(false);
    }
  }

  async function refreshAllCodex() {
    setCodexBusy(true);
    try {
      const accounts = await refreshAllCodexAccounts();
      setCodexAccounts(accounts);
      setCodexError(null);
    } catch (error) {
      setCodexError(error instanceof Error ? error.message : String(error));
    } finally {
      setCodexBusy(false);
    }
  }

  async function removeCodexAccount(accountId: string) {
    setCodexBusy(true);
    try {
      await deleteCodexAccount(accountId);
      setCodexAccounts((accounts) => accounts.filter((item) => item.id !== accountId));
      setCodexError(null);
    } catch (error) {
      setCodexError(error instanceof Error ? error.message : String(error));
    } finally {
      setCodexBusy(false);
    }
  }

  async function importLocalAntigravity() {
    setAntigravityBusy(true);
    try {
      const account = await importAntigravityFromLocal();
      setAntigravityAccounts((accounts) => [account, ...accounts.filter((item) => item.id !== account.id)]);
      setView('dashboard');
      setAntigravityError(null);
    } catch (error) {
      setAntigravityError(error instanceof Error ? error.message : String(error));
    } finally {
      setAntigravityBusy(false);
    }
  }

  async function startAntigravityAuth() {
    setAntigravityBusy(true);
    try {
      setAntigravityLogin(await startAntigravityOAuthLogin());
      setView('integrations');
      setAntigravityError(null);
    } catch (error) {
      setAntigravityError(error instanceof Error ? error.message : String(error));
    } finally {
      setAntigravityBusy(false);
    }
  }

  async function openAntigravityAuthUrl() {
    if (!antigravityLogin) return;

    try {
      await openUrl(antigravityLogin.authUrl);
      setAntigravityError(null);
    } catch (error) {
      window.open(antigravityLogin.authUrl, '_blank', 'noopener,noreferrer');
      setAntigravityError(error instanceof Error ? `Opened with browser fallback. Tauri opener said: ${error.message}` : null);
    }
  }

  async function completeAntigravityAuth() {
    if (!antigravityLogin) return;
    setAntigravityBusy(true);
    try {
      const account = await completeAntigravityOAuthLogin(antigravityLogin.loginId);
      setAntigravityAccounts((accounts) => [account, ...accounts.filter((item) => item.id !== account.id)]);
      setAntigravityLogin(null);
      setView('dashboard');
      setAntigravityError(null);
    } catch (error) {
      setAntigravityError(error instanceof Error ? error.message : String(error));
    } finally {
      setAntigravityBusy(false);
    }
  }

  async function cancelAntigravityAuth() {
    setAntigravityBusy(true);
    try {
      await cancelAntigravityOAuthLogin(antigravityLogin?.loginId);
      setAntigravityLogin(null);
      setAntigravityError(null);
    } catch (error) {
      setAntigravityError(error instanceof Error ? error.message : String(error));
    } finally {
      setAntigravityBusy(false);
    }
  }

  async function refreshAntigravity(accountId: string) {
    setAntigravityBusy(true);
    try {
      const account = await refreshAntigravityAccount(accountId);
      setAntigravityAccounts((accounts) => accounts.map((item) => (item.id === account.id ? account : item)));
      setAntigravityError(null);
    } catch (error) {
      setAntigravityError(error instanceof Error ? error.message : String(error));
    } finally {
      setAntigravityBusy(false);
    }
  }

  async function refreshAllAntigravity() {
    setAntigravityBusy(true);
    try {
      const accounts = await refreshAllAntigravityAccounts();
      setAntigravityAccounts(accounts);
      setAntigravityError(null);
    } catch (error) {
      setAntigravityError(error instanceof Error ? error.message : String(error));
    } finally {
      setAntigravityBusy(false);
    }
  }

  async function removeAntigravityAccount(accountId: string) {
    setAntigravityBusy(true);
    try {
      await deleteAntigravityAccount(accountId);
      setAntigravityAccounts((accounts) => accounts.filter((item) => item.id !== accountId));
      setAntigravityError(null);
    } catch (error) {
      setAntigravityError(error instanceof Error ? error.message : String(error));
    } finally {
      setAntigravityBusy(false);
    }
  }

  async function startClaudeAuth() {
    setClaudeBusy(true);
    try {
      setClaudeLogin(await startClaudeOAuthLogin());
      setClaudeCallbackInput('');
      setClaudeEmailHint('');
      setView('integrations');
      setClaudeError(null);
    } catch (error) {
      setClaudeError(error instanceof Error ? error.message : String(error));
    } finally {
      setClaudeBusy(false);
    }
  }

  async function openClaudeAuthUrl() {
    if (!claudeLogin) return;

    try {
      await openUrl(claudeLogin.authUrl);
      setClaudeError(null);
    } catch (error) {
      window.open(claudeLogin.authUrl, '_blank', 'noopener,noreferrer');
      setClaudeError(error instanceof Error ? `Opened with browser fallback. Tauri opener said: ${error.message}` : null);
    }
  }

  async function completeClaudeAuth() {
    if (!claudeLogin) return;
    setClaudeBusy(true);
    try {
      const account = await completeClaudeOAuthLogin(claudeLogin.loginId, claudeCallbackInput, claudeEmailHint);
      setClaudeAccounts((accounts) => [account, ...accounts.filter((item) => item.id !== account.id)]);
      setClaudeLogin(null);
      setClaudeCallbackInput('');
      setClaudeEmailHint('');
      setView('dashboard');
      setClaudeError(null);
    } catch (error) {
      setClaudeError(error instanceof Error ? error.message : String(error));
    } finally {
      setClaudeBusy(false);
    }
  }

  async function cancelClaudeAuth() {
    setClaudeBusy(true);
    try {
      await cancelClaudeOAuthLogin(claudeLogin?.loginId);
      setClaudeLogin(null);
      setClaudeCallbackInput('');
      setClaudeEmailHint('');
      setClaudeError(null);
    } catch (error) {
      setClaudeError(error instanceof Error ? error.message : String(error));
    } finally {
      setClaudeBusy(false);
    }
  }

  async function refreshClaude(accountId: string) {
    setClaudeBusy(true);
    try {
      const account = await refreshClaudeAccount(accountId);
      setClaudeAccounts((accounts) => accounts.map((item) => (item.id === account.id ? account : item)));
      setClaudeError(null);
    } catch (error) {
      setClaudeError(error instanceof Error ? error.message : String(error));
    } finally {
      setClaudeBusy(false);
    }
  }

  async function refreshAllClaude() {
    setClaudeBusy(true);
    try {
      const accounts = await refreshAllClaudeAccounts();
      setClaudeAccounts(accounts);
      setClaudeError(null);
    } catch (error) {
      setClaudeError(error instanceof Error ? error.message : String(error));
    } finally {
      setClaudeBusy(false);
    }
  }

  async function removeClaudeAccount(accountId: string) {
    setClaudeBusy(true);
    try {
      await deleteClaudeAccount(accountId);
      setClaudeAccounts((accounts) => accounts.filter((item) => item.id !== accountId));
      setClaudeError(null);
    } catch (error) {
      setClaudeError(error instanceof Error ? error.message : String(error));
    } finally {
      setClaudeBusy(false);
    }
  }

  async function importLocalKiro() {
    setKiroBusy(true);
    try {
      const accounts = await importKiroFromLocal();
      setKiroAccounts((prev) => {
        const byId = new Map(prev.map((a) => [a.id, a]));
        for (const a of accounts) byId.set(a.id, a);
        return Array.from(byId.values());
      });
      setView('dashboard');
      setKiroError(null);
    } catch (error) {
      setKiroError(error instanceof Error ? error.message : String(error));
    } finally {
      setKiroBusy(false);
    }
  }

  async function startKiroAuth() {
    setKiroBusy(true);
    try {
      setKiroLogin(await startKiroOAuthLogin());
      setView('integrations');
      setKiroError(null);
    } catch (error) {
      setKiroError(error instanceof Error ? error.message : String(error));
    } finally {
      setKiroBusy(false);
    }
  }

  async function openKiroAuthUrl() {
    if (!kiroLogin) return;
    try {
      await openUrl(kiroLogin.authUrl);
      setKiroError(null);
    } catch (error) {
      window.open(kiroLogin.authUrl, '_blank', 'noopener,noreferrer');
      setKiroError(error instanceof Error ? `Opened with browser fallback. Tauri opener said: ${error.message}` : null);
    }
  }

  async function completeKiroAuth() {
    if (!kiroLogin) return;
    setKiroBusy(true);
    try {
      const account = await completeKiroOAuthLogin(kiroLogin.loginId);
      setKiroAccounts((accounts) => [account, ...accounts.filter((item) => item.id !== account.id)]);
      setKiroLogin(null);
      setView('dashboard');
      setKiroError(null);
    } catch (error) {
      setKiroError(error instanceof Error ? error.message : String(error));
    } finally {
      setKiroBusy(false);
    }
  }

  async function cancelKiroAuth() {
    setKiroBusy(true);
    try {
      await cancelKiroOAuthLogin(kiroLogin?.loginId);
      setKiroLogin(null);
      setKiroError(null);
    } catch (error) {
      setKiroError(error instanceof Error ? error.message : String(error));
    } finally {
      setKiroBusy(false);
    }
  }

  async function refreshKiro(accountId: string) {
    setKiroBusy(true);
    try {
      const account = await refreshKiroAccount(accountId);
      setKiroAccounts((accounts) => accounts.map((item) => (item.id === account.id ? account : item)));
      setKiroError(null);
    } catch (error) {
      setKiroError(error instanceof Error ? error.message : String(error));
    } finally {
      setKiroBusy(false);
    }
  }

  async function refreshAllKiro() {
    setKiroBusy(true);
    try {
      const accounts = await refreshAllKiroAccounts();
      setKiroAccounts(accounts);
      setKiroError(null);
    } catch (error) {
      setKiroError(error instanceof Error ? error.message : String(error));
    } finally {
      setKiroBusy(false);
    }
  }

  async function removeKiroAccount(accountId: string) {
    setKiroBusy(true);
    try {
      await deleteKiroAccount(accountId);
      setKiroAccounts((accounts) => accounts.filter((item) => item.id !== accountId));
      setKiroError(null);
    } catch (error) {
      setKiroError(error instanceof Error ? error.message : String(error));
    } finally {
      setKiroBusy(false);
    }
  }

  async function importLocalCursor() {
    setCursorBusy(true);
    try {
      const account = await importCursorFromLocal();
      setCursorAccounts((accounts) => [account, ...accounts.filter((item) => item.id !== account.id)]);
      setView('dashboard');
      setCursorError(null);
    } catch (error) {
      setCursorError(error instanceof Error ? error.message : String(error));
    } finally {
      setCursorBusy(false);
    }
  }

  async function startCursorAuth() {
    setCursorBusy(true);
    try {
      setCursorLogin(await startCursorOAuthLogin());
      setView('integrations');
      setCursorError(null);
    } catch (error) {
      setCursorError(error instanceof Error ? error.message : String(error));
    } finally {
      setCursorBusy(false);
    }
  }

  async function openCursorAuthUrl() {
    if (!cursorLogin) return;
    try {
      await openUrl(cursorLogin.verificationUri);
      setCursorError(null);
    } catch (error) {
      window.open(cursorLogin.verificationUri, '_blank', 'noopener,noreferrer');
      setCursorError(error instanceof Error ? `Opened with browser fallback. Tauri opener said: ${error.message}` : null);
    }
  }

  async function completeCursorAuth() {
    if (!cursorLogin) return;
    setCursorBusy(true);
    try {
      const account = await completeCursorOAuthLogin(cursorLogin.loginId);
      setCursorAccounts((accounts) => [account, ...accounts.filter((item) => item.id !== account.id)]);
      setCursorLogin(null);
      setView('dashboard');
      setCursorError(null);
    } catch (error) {
      setCursorError(error instanceof Error ? error.message : String(error));
    } finally {
      setCursorBusy(false);
    }
  }

  async function cancelCursorAuth() {
    setCursorBusy(true);
    try {
      await cancelCursorOAuthLogin(cursorLogin?.loginId);
      setCursorLogin(null);
      setCursorError(null);
    } catch (error) {
      setCursorError(error instanceof Error ? error.message : String(error));
    } finally {
      setCursorBusy(false);
    }
  }

  async function refreshCursor(accountId: string) {
    setCursorBusy(true);
    try {
      const account = await refreshCursorAccount(accountId);
      setCursorAccounts((accounts) => accounts.map((item) => (item.id === account.id ? account : item)));
      setCursorError(null);
    } catch (error) {
      setCursorError(error instanceof Error ? error.message : String(error));
    } finally {
      setCursorBusy(false);
    }
  }

  async function refreshAllCursor() {
    setCursorBusy(true);
    try {
      const accounts = await refreshAllCursorAccounts();
      setCursorAccounts(accounts);
      setCursorError(null);
    } catch (error) {
      setCursorError(error instanceof Error ? error.message : String(error));
    } finally {
      setCursorBusy(false);
    }
  }

  async function removeCursorAccount(accountId: string) {
    setCursorBusy(true);
    try {
      await deleteCursorAccount(accountId);
      setCursorAccounts((accounts) => accounts.filter((item) => item.id !== accountId));
      setCursorError(null);
    } catch (error) {
      setCursorError(error instanceof Error ? error.message : String(error));
    } finally {
      setCursorBusy(false);
    }
  }

  function togglePinnedAccount(accountId: string) {
    setPinnedAccounts((prev) => {
      const next = new Set(prev);
      if (next.has(accountId)) next.delete(accountId);
      else next.add(accountId);
      return next;
    });
  }

  function toggleHiddenProvider(provider: ProviderKey) {
    setHiddenProviders((prev) => {
      const next = new Set(prev);
      if (next.has(provider)) next.delete(provider);
      else next.add(provider);
      return next;
    });
  }

  function exportSafeAccountSummaries() {
    const exportedAt = new Date().toISOString();
    const payload = {
      app: 'Quota',
      exportedAt,
      exportType: 'safe-account-summaries',
      includesSecrets: false,
      accountTotals: {
        total: connectedCount,
        githubCopilot: copilotAccounts.length,
        codex: codexAccounts.length,
        antigravity: antigravityAccounts.length,
        claude: claudeAccounts.length,
        kiro: kiroAccounts.length,
        cursor: cursorAccounts.length,
      },
      providers: {
        githubCopilot: copilotAccounts,
        codex: codexAccounts,
        antigravity: antigravityAccounts,
        claude: claudeAccounts,
        kiro: kiroAccounts,
        cursor: cursorAccounts,
      },
    };
    const json = JSON.stringify(payload, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `quota-safe-account-summaries-${exportedAt.slice(0, 10)}.json`;
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
  }

  return (
    <main className="app-shell">
      <aside className="sidebar" aria-label="Quota navigation">
        <div className="brand">
          <img src="/icon.png" alt="" className="brand__icon" />
          <div>
            <span className="brand__name">Quota</span>
            <span className="brand__meta">Track AI usage</span>
          </div>
        </div>

        <nav className="nav-list" aria-label="Main sections">
          <button
            type="button"
            className={`nav-list__item ${view === 'dashboard' ? 'nav-list__item--active' : ''}`}
            onClick={() => setView('dashboard')}
          >
            <Grid2X2 size={17} />
            Dashboard
          </button>
          <button
            type="button"
            className={`nav-list__item ${view === 'integrations' ? 'nav-list__item--active' : ''}`}
            onClick={() => setView('integrations')}
          >
            <Plus size={17} />
            Integrations
          </button>
          <button
            type="button"
            className={`nav-list__item ${view === 'settings' ? 'nav-list__item--active' : ''}`}
            onClick={() => setView('settings')}
          >
            <Settings size={17} />
            Settings
          </button>
        </nav>

      </aside>

      <section className="content">
        {view === 'dashboard' ? (
          <DashboardView
            viewMode={dashboardViewMode}
            providerOrder={providerOrder}
            pinnedAccounts={pinnedAccounts}
            hiddenProviders={hiddenProviders}
            onTogglePinnedAccount={togglePinnedAccount}
            copilotAccounts={copilotAccounts}
            codexAccounts={codexAccounts}
            antigravityAccounts={antigravityAccounts}
            claudeAccounts={claudeAccounts}
            kiroAccounts={kiroAccounts}
            cursorAccounts={cursorAccounts}
            copilotBusy={copilotBusy}
            codexBusy={codexBusy}
            antigravityBusy={antigravityBusy}
            claudeBusy={claudeBusy}
            kiroBusy={kiroBusy}
            cursorBusy={cursorBusy}
            copilotError={copilotError}
            codexError={codexError}
            antigravityError={antigravityError}
            claudeError={claudeError}
            kiroError={kiroError}
            cursorError={cursorError}
            onRefreshAllCopilot={refreshAllCopilot}
            onRefreshCopilotAccount={refreshCopilotAccount}
            onRemoveCopilotAccount={removeCopilotAccount}
            onRefreshAllCodex={refreshAllCodex}
            onRefreshCodexAccount={refreshCodex}
            onRemoveCodexAccount={removeCodexAccount}
            onRefreshAllAntigravity={refreshAllAntigravity}
            onRefreshAntigravityAccount={refreshAntigravity}
            onRemoveAntigravityAccount={removeAntigravityAccount}
            onRefreshAllClaude={refreshAllClaude}
            onRefreshClaudeAccount={refreshClaude}
            onRemoveClaudeAccount={removeClaudeAccount}
            onRefreshAllKiro={refreshAllKiro}
            onRefreshKiroAccount={refreshKiro}
            onRemoveKiroAccount={removeKiroAccount}
            onRefreshAllCursor={refreshAllCursor}
            onRefreshCursorAccount={refreshCursor}
            onRemoveCursorAccount={removeCursorAccount}
            onOpenIntegrations={() => setView('integrations')}
            onOpenCopilotAccounts={() => setView('github-copilot-accounts')}
            onOpenCodexAccounts={() => setView('codex-accounts')}
            onOpenAntigravityAccounts={() => setView('antigravity-accounts')}
            onOpenClaudeAccounts={() => setView('claude-accounts')}
            onOpenKiroAccounts={() => setView('kiro-accounts')}
            onOpenCursorAccounts={() => setView('cursor-accounts')}
          />
        ) : view === 'settings' ? (
          <SettingsView
            connectedCount={connectedCount}
            themeMode={themeMode}
            dashboardViewMode={dashboardViewMode}
            accountPagesViewMode={accountPagesViewMode}
            providerOrder={providerOrder}
            hiddenProviders={hiddenProviders}
            onThemeModeChange={setThemeMode}
            onDashboardViewModeChange={setDashboardViewMode}
            onAccountPagesViewModeChange={setAccountPagesViewMode}
            onMoveProvider={(provider, direction) => setProviderOrder((order) => moveProvider(order, provider, direction))}
            onToggleHiddenProvider={toggleHiddenProvider}
            onExportSafeAccountSummaries={exportSafeAccountSummaries}
          />
        ) : view === 'github-copilot-accounts' ? (
          <ProviderAccountsView
            viewMode={accountPagesViewMode}
            accounts={copilotAccounts}
            busy={copilotBusy}
            error={copilotError}
            pinnedAccounts={pinnedAccounts}
            onBack={() => setView('dashboard')}
            onOpenIntegrations={() => setView('integrations')}
            onRefreshAll={refreshAllCopilot}
            onRefreshAccount={refreshCopilotAccount}
            onRemoveAccount={removeCopilotAccount}
            onTogglePinnedAccount={togglePinnedAccount}
          />
        ) : view === 'codex-accounts' ? (
          <CodexAccountsView
            viewMode={accountPagesViewMode}
            accounts={codexAccounts}
            busy={codexBusy}
            error={codexError}
            pinnedAccounts={pinnedAccounts}
            onBack={() => setView('dashboard')}
            onOpenIntegrations={() => setView('integrations')}
            onRefreshAll={refreshAllCodex}
            onRefreshAccount={refreshCodex}
            onRemoveAccount={removeCodexAccount}
            onTogglePinnedAccount={togglePinnedAccount}
          />
        ) : view === 'antigravity-accounts' ? (
          <AntigravityAccountsView
            viewMode={accountPagesViewMode}
            accounts={antigravityAccounts}
            busy={antigravityBusy}
            error={antigravityError}
            pinnedAccounts={pinnedAccounts}
            onBack={() => setView('dashboard')}
            onOpenIntegrations={() => setView('integrations')}
            onRefreshAll={refreshAllAntigravity}
            onRefreshAccount={refreshAntigravity}
            onRemoveAccount={removeAntigravityAccount}
            onTogglePinnedAccount={togglePinnedAccount}
          />
        ) : view === 'claude-accounts' ? (
          <ClaudeAccountsView
            viewMode={accountPagesViewMode}
            accounts={claudeAccounts}
            busy={claudeBusy}
            error={claudeError}
            pinnedAccounts={pinnedAccounts}
            onBack={() => setView('dashboard')}
            onOpenIntegrations={() => setView('integrations')}
            onRefreshAll={refreshAllClaude}
            onRefreshAccount={refreshClaude}
            onRemoveAccount={removeClaudeAccount}
            onTogglePinnedAccount={togglePinnedAccount}
          />
        ) : view === 'kiro-accounts' ? (
          <KiroAccountsView
            viewMode={accountPagesViewMode}
            accounts={kiroAccounts}
            busy={kiroBusy}
            error={kiroError}
            pinnedAccounts={pinnedAccounts}
            onBack={() => setView('dashboard')}
            onOpenIntegrations={() => setView('integrations')}
            onRefreshAll={refreshAllKiro}
            onRefreshAccount={refreshKiro}
            onRemoveAccount={removeKiroAccount}
            onTogglePinnedAccount={togglePinnedAccount}
          />
        ) : view === 'cursor-accounts' ? (
          <CursorAccountsView
            viewMode={accountPagesViewMode}
            accounts={cursorAccounts}
            busy={cursorBusy}
            error={cursorError}
            pinnedAccounts={pinnedAccounts}
            onBack={() => setView('dashboard')}
            onOpenIntegrations={() => setView('integrations')}
            onRefreshAll={refreshAllCursor}
            onRefreshAccount={refreshCursor}
            onRemoveAccount={removeCursorAccount}
            onTogglePinnedAccount={togglePinnedAccount}
          />
        ) : (
          <IntegrationsView
            connectedCount={connectedCount}
            codexConnectedCount={codexAccounts.length}
            antigravityConnectedCount={antigravityAccounts.length}
            claudeConnectedCount={claudeAccounts.length}
            kiroConnectedCount={kiroAccounts.length}
            cursorConnectedCount={cursorAccounts.length}
            copilotBusy={copilotBusy}
            codexBusy={codexBusy}
            antigravityBusy={antigravityBusy}
            claudeBusy={claudeBusy}
            kiroBusy={kiroBusy}
            cursorBusy={cursorBusy}
            copilotError={copilotError}
            codexError={codexError}
            antigravityError={antigravityError}
            claudeError={claudeError}
            kiroError={kiroError}
            cursorError={cursorError}
            copilotLogin={copilotLogin}
            codexLogin={codexLogin}
            antigravityLogin={antigravityLogin}
            claudeLogin={claudeLogin}
            kiroLogin={kiroLogin}
            cursorLogin={cursorLogin}
            claudeCallbackInput={claudeCallbackInput}
            claudeEmailHint={claudeEmailHint}
            onStartCopilotAuth={startCopilotAuth}
            onStartCodexAuth={startCodexAuth}
            onStartAntigravityAuth={startAntigravityAuth}
            onStartClaudeAuth={startClaudeAuth}
            onStartKiroAuth={startKiroAuth}
            onStartCursorAuth={startCursorAuth}
            onImportLocalCodex={importLocalCodex}
            onImportLocalAntigravity={importLocalAntigravity}
            onImportLocalKiro={importLocalKiro}
            onImportLocalCursor={importLocalCursor}
            onOpenCopilotAuthUrl={openCopilotAuthUrl}
            onOpenCodexAuthUrl={openCodexAuthUrl}
            onOpenAntigravityAuthUrl={openAntigravityAuthUrl}
            onOpenClaudeAuthUrl={openClaudeAuthUrl}
            onOpenKiroAuthUrl={openKiroAuthUrl}
            onOpenCursorAuthUrl={openCursorAuthUrl}
            onCompleteCopilotAuth={completeCopilotAuth}
            onCompleteCodexAuth={completeCodexAuth}
            onCompleteAntigravityAuth={completeAntigravityAuth}
            onCompleteClaudeAuth={completeClaudeAuth}
            onCompleteKiroAuth={completeKiroAuth}
            onCompleteCursorAuth={completeCursorAuth}
            onCancelCopilotAuth={cancelCopilotAuth}
            onCancelCodexAuth={cancelCodexAuth}
            onCancelAntigravityAuth={cancelAntigravityAuth}
            onCancelClaudeAuth={cancelClaudeAuth}
            onCancelKiroAuth={cancelKiroAuth}
            onCancelCursorAuth={cancelCursorAuth}
            onClaudeCallbackInputChange={setClaudeCallbackInput}
            onClaudeEmailHintChange={setClaudeEmailHint}
          />
        )}
      </section>
    </main>
  );
}

interface SettingsViewProps {
  connectedCount: number;
  themeMode: ThemeMode;
  dashboardViewMode: ViewMode;
  accountPagesViewMode: ViewMode;
  providerOrder: ProviderKey[];
  hiddenProviders: Set<ProviderKey>;
  onThemeModeChange: (mode: ThemeMode) => void;
  onDashboardViewModeChange: (mode: ViewMode) => void;
  onAccountPagesViewModeChange: (mode: ViewMode) => void;
  onMoveProvider: (provider: ProviderKey, direction: -1 | 1) => void;
  onToggleHiddenProvider: (provider: ProviderKey) => void;
  onExportSafeAccountSummaries: () => void;
}

function SettingsView({
  connectedCount,
  themeMode,
  dashboardViewMode,
  accountPagesViewMode,
  providerOrder,
  hiddenProviders,
  onThemeModeChange,
  onDashboardViewModeChange,
  onAccountPagesViewModeChange,
  onMoveProvider,
  onToggleHiddenProvider,
  onExportSafeAccountSummaries,
}: SettingsViewProps) {
  return (
    <div className="page-stack">
      <PageHeader
        title="Settings"
        description="App preferences, safe data export, and future appearance controls."
      />

      <section className="settings-grid" aria-label="Settings sections">
        <article className="settings-section">
          <div className="settings-section__header">
            <div className="settings-section__icon">
              <Palette size={18} />
            </div>
            <div>
              <h2>Appearance</h2>
              <p>Theme and layout preferences for the dashboard and account pages.</p>
            </div>
          </div>

          <div className="settings-list">
            <SettingsControlRow label="Theme">
              <ThemeModeControl
                value={themeMode}
                onChange={onThemeModeChange}
              />
            </SettingsControlRow>
            <SettingsControlRow label="Dashboard view">
              <ViewModeControl
                label="Dashboard view"
                value={dashboardViewMode}
                onChange={onDashboardViewModeChange}
              />
            </SettingsControlRow>
            <SettingsControlRow label="Account pages view">
              <ViewModeControl
                label="Account pages view"
                value={accountPagesViewMode}
                onChange={onAccountPagesViewModeChange}
              />
            </SettingsControlRow>
            <SettingsControlRow label="Dashboard order">
              <ProviderOrderControl
                providerOrder={providerOrder}
                hiddenProviders={hiddenProviders}
                onMoveProvider={onMoveProvider}
                onToggleHiddenProvider={onToggleHiddenProvider}
              />
            </SettingsControlRow>
            <div className="settings-hint">
              Pin accounts from any provider's accounts page to choose which appear on the dashboard.
            </div>
          </div>
        </article>

        <article className="settings-section">
          <div className="settings-section__header">
            <div className="settings-section__icon">
              <Database size={18} />
            </div>
            <div>
              <h2>Data</h2>
              <p>Export the safe summaries already visible in Quota. Raw tokens stay out of the file.</p>
            </div>
          </div>

          <div className="settings-action">
            <div>
              <strong>Safe account summaries</strong>
              <span>{connectedCount} connected account{connectedCount === 1 ? '' : 's'} included.</span>
            </div>
            <button type="button" className="button-primary" onClick={onExportSafeAccountSummaries}>
              <Download size={15} />
              Export JSON
            </button>
          </div>
        </article>

        <article className="settings-section">
          <div className="settings-section__header">
            <div className="settings-section__icon">
              <LockKeyhole size={18} />
            </div>
            <div>
              <h2>Privacy</h2>
              <p>Quota keeps secret-sensitive account data in Rust-owned local storage.</p>
            </div>
          </div>

          <div className="settings-list">
            <SettingsRow label="Export contents" value="Safe frontend summaries only." />
            <SettingsRow label="Tokens and API keys" value="Never included in the JSON export." />
          </div>
        </article>
      </section>
    </div>
  );
}

interface SettingsRowProps {
  label: string;
  value: string;
}

function SettingsRow({ label, value }: SettingsRowProps) {
  return (
    <div className="settings-row">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

interface SettingsControlRowProps {
  label: string;
  children: ReactNode;
}

function SettingsControlRow({ label, children }: SettingsControlRowProps) {
  return (
    <div className="settings-row settings-row--control">
      <span>{label}</span>
      {children}
    </div>
  );
}

interface ViewModeControlProps {
  label: string;
  value: ViewMode;
  onChange: (mode: ViewMode) => void;
}

function ViewModeControl({ label, value, onChange }: ViewModeControlProps) {
  return (
    <div className="segmented-control" role="group" aria-label={label}>
      {VIEW_MODES.map((mode) => (
        <button
          key={mode}
          type="button"
          className={mode === value ? 'segmented-control__item segmented-control__item--active' : 'segmented-control__item'}
          onClick={() => onChange(mode)}
          aria-pressed={mode === value}
        >
          {formatViewMode(mode)}
        </button>
      ))}
    </div>
  );
}

function formatViewMode(mode: ViewMode) {
  return mode[0].toUpperCase() + mode.slice(1);
}

interface ThemeModeControlProps {
  value: ThemeMode;
  onChange: (mode: ThemeMode) => void;
}

function ThemeModeControl({ value, onChange }: ThemeModeControlProps) {
  return (
    <div className="segmented-control" role="group" aria-label="Theme">
      {THEME_MODES.map((mode) => (
        <button
          key={mode}
          type="button"
          className={mode === value ? 'segmented-control__item segmented-control__item--active' : 'segmented-control__item'}
          onClick={() => onChange(mode)}
          aria-pressed={mode === value}
        >
          {formatThemeMode(mode)}
        </button>
      ))}
    </div>
  );
}

function formatThemeMode(mode: ThemeMode) {
  return mode[0].toUpperCase() + mode.slice(1);
}

interface ProviderOrderControlProps {
  providerOrder: ProviderKey[];
  hiddenProviders: Set<ProviderKey>;
  onMoveProvider: (provider: ProviderKey, direction: -1 | 1) => void;
  onToggleHiddenProvider: (provider: ProviderKey) => void;
}

function ProviderOrderControl({ providerOrder, hiddenProviders, onMoveProvider, onToggleHiddenProvider }: ProviderOrderControlProps) {
  return (
    <div className="provider-order-list">
      {providerOrder.map((providerKey, index) => {
        const provider = PROVIDERS.find((item) => item.key === providerKey);
        if (!provider) return null;
        const hidden = hiddenProviders.has(provider.key);

        return (
          <div className={`provider-order-row${hidden ? ' provider-order-row--hidden' : ''}`} key={provider.key}>
            <div className="provider-order-row__label">
              <BrandIcon src={provider.iconPath} alt="" size="small" />
              <strong>{provider.name}</strong>
            </div>
            <div className="provider-order-row__actions">
              <button
                type="button"
                className={hidden ? 'provider-order-row__visibility provider-order-row__visibility--hidden' : 'provider-order-row__visibility'}
                onClick={() => onToggleHiddenProvider(provider.key)}
                aria-label={hidden ? `Show ${provider.name} on dashboard` : `Hide ${provider.name} from dashboard`}
              >
                {hidden ? <EyeOff size={14} /> : <Eye size={14} />}
              </button>
              <button
                type="button"
                onClick={() => onMoveProvider(provider.key, -1)}
                disabled={index === 0}
                aria-label={`Move ${provider.name} up`}
              >
                <ArrowUp size={14} />
              </button>
              <button
                type="button"
                onClick={() => onMoveProvider(provider.key, 1)}
                disabled={index === providerOrder.length - 1}
                aria-label={`Move ${provider.name} down`}
              >
                <ArrowDown size={14} />
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}

interface DashboardViewProps {
  viewMode: ViewMode;
  providerOrder: ProviderKey[];
  pinnedAccounts: Set<string>;
  hiddenProviders: Set<ProviderKey>;
  onTogglePinnedAccount: (accountId: string) => void;
  copilotAccounts: GitHubCopilotAccountSummary[];
  codexAccounts: CodexAccountSummary[];
  antigravityAccounts: AntigravityAccountSummary[];
  claudeAccounts: ClaudeAccountSummary[];
  kiroAccounts: KiroAccountSummary[];
  cursorAccounts: CursorAccountSummary[];
  copilotBusy: boolean;
  codexBusy: boolean;
  antigravityBusy: boolean;
  claudeBusy: boolean;
  kiroBusy: boolean;
  cursorBusy: boolean;
  copilotError: string | null;
  codexError: string | null;
  antigravityError: string | null;
  claudeError: string | null;
  kiroError: string | null;
  cursorError: string | null;
  onRefreshAllCopilot: () => void;
  onRefreshCopilotAccount: (accountId: string) => void;
  onRemoveCopilotAccount: (accountId: string) => void;
  onRefreshAllCodex: () => void;
  onRefreshCodexAccount: (accountId: string) => void;
  onRemoveCodexAccount: (accountId: string) => void;
  onRefreshAllAntigravity: () => void;
  onRefreshAntigravityAccount: (accountId: string) => void;
  onRemoveAntigravityAccount: (accountId: string) => void;
  onRefreshAllClaude: () => void;
  onRefreshClaudeAccount: (accountId: string) => void;
  onRemoveClaudeAccount: (accountId: string) => void;
  onRefreshAllKiro: () => void;
  onRefreshKiroAccount: (accountId: string) => void;
  onRemoveKiroAccount: (accountId: string) => void;
  onRefreshAllCursor: () => void;
  onRefreshCursorAccount: (accountId: string) => void;
  onRemoveCursorAccount: (accountId: string) => void;
  onOpenIntegrations: () => void;
  onOpenCopilotAccounts: () => void;
  onOpenCodexAccounts: () => void;
  onOpenAntigravityAccounts: () => void;
  onOpenClaudeAccounts: () => void;
  onOpenKiroAccounts: () => void;
  onOpenCursorAccounts: () => void;
}

function DashboardView({
  viewMode,
  providerOrder,
  pinnedAccounts,
  hiddenProviders,
  onTogglePinnedAccount,
  copilotAccounts,
  codexAccounts,
  antigravityAccounts,
  claudeAccounts,
  kiroAccounts,
  cursorAccounts,
  copilotBusy,
  codexBusy,
  antigravityBusy,
  claudeBusy,
  kiroBusy,
  cursorBusy,
  copilotError,
  codexError,
  antigravityError,
  claudeError,
  kiroError,
  cursorError,
  onRefreshAllCopilot,
  onRefreshCopilotAccount,
  onRemoveCopilotAccount,
  onRefreshAllCodex,
  onRefreshCodexAccount,
  onRemoveCodexAccount,
  onRefreshAllAntigravity,
  onRefreshAntigravityAccount,
  onRemoveAntigravityAccount,
  onRefreshAllClaude,
  onRefreshClaudeAccount,
  onRemoveClaudeAccount,
  onRefreshAllKiro,
  onRefreshKiroAccount,
  onRemoveKiroAccount,
  onRefreshAllCursor,
  onRefreshCursorAccount,
  onRemoveCursorAccount,
  onOpenIntegrations,
  onOpenCopilotAccounts,
  onOpenCodexAccounts,
  onOpenAntigravityAccounts,
  onOpenClaudeAccounts,
  onOpenKiroAccounts,
  onOpenCursorAccounts,
}: DashboardViewProps) {
  const visibleCopilotAccounts = getVisibleAccounts(copilotAccounts, pinnedAccounts);
  const visibleCodexAccounts = getVisibleAccounts(codexAccounts, pinnedAccounts);
  const visibleAntigravityAccounts = getVisibleAccounts(antigravityAccounts, pinnedAccounts);
  const visibleClaudeAccounts = getVisibleAccounts(claudeAccounts, pinnedAccounts);
  const visibleKiroAccounts = getVisibleAccounts(kiroAccounts, pinnedAccounts);
  const visibleCursorAccounts = getVisibleAccounts(cursorAccounts, pinnedAccounts);
  const hasAccounts =
    copilotAccounts.length > 0 ||
    codexAccounts.length > 0 ||
    antigravityAccounts.length > 0 ||
    claudeAccounts.length > 0 ||
    kiroAccounts.length > 0 ||
    cursorAccounts.length > 0;

  return (
    <div className={`page-stack dashboard-view dashboard-view--${viewMode}`}>
      <PageHeader
        title="Dashboard"
        description=""
        action={
          <div className="button-row">
            <button
              type="button"
              onClick={() => {
                onRefreshAllCopilot();
                onRefreshAllCodex();
                onRefreshAllAntigravity();
                onRefreshAllClaude();
                onRefreshAllKiro();
                onRefreshAllCursor();
              }}
              disabled={
                (copilotBusy || copilotAccounts.length === 0) &&
                (codexBusy || codexAccounts.length === 0) &&
                (antigravityBusy || antigravityAccounts.length === 0) &&
                (claudeBusy || claudeAccounts.length === 0) &&
                (kiroBusy || kiroAccounts.length === 0) &&
                (cursorBusy || cursorAccounts.length === 0)
              }
            >
              <RefreshCcw size={15} />
              Refresh
            </button>
            <button type="button" className="button-primary" onClick={onOpenIntegrations}>
              <Plus size={15} />
              Add account
            </button>
          </div>
        }
      />

      {copilotError ? <p className="account-panel__error">{copilotError}</p> : null}
      {codexError ? <p className="account-panel__error">{codexError}</p> : null}
      {antigravityError ? <p className="account-panel__error">{antigravityError}</p> : null}
      {claudeError ? <p className="account-panel__error">{claudeError}</p> : null}
      {kiroError ? <p className="account-panel__error">{kiroError}</p> : null}
      {cursorError ? <p className="account-panel__error">{cursorError}</p> : null}

      <ProviderSummaryGrid
        copilotCount={copilotAccounts.length}
        codexCount={codexAccounts.length}
        antigravityCount={antigravityAccounts.length}
        claudeCount={claudeAccounts.length}
        kiroCount={kiroAccounts.length}
        cursorCount={cursorAccounts.length}
        providerOrder={providerOrder}
        onOpenCopilotAccounts={onOpenCopilotAccounts}
        onOpenCodexAccounts={onOpenCodexAccounts}
        onOpenAntigravityAccounts={onOpenAntigravityAccounts}
        onOpenClaudeAccounts={onOpenClaudeAccounts}
        onOpenKiroAccounts={onOpenKiroAccounts}
        onOpenCursorAccounts={onOpenCursorAccounts}
      />

      {!hasAccounts ? (
        <div className="empty-state empty-state--large">
          <Grid2X2 size={26} />
          <strong>No connected accounts yet.</strong>
          <span>Connect an integration and Quota will turn this into a usage dashboard.</span>
          <button type="button" className="button-primary" onClick={onOpenIntegrations}>
            <Plus size={15} />
            Open integrations
          </button>
        </div>
      ) : (
        <div className="dashboard-providers">
          {copilotAccounts.length > 0 && !hiddenProviders.has('githubCopilot') ? (
            <section
              className="provider-section"
              aria-labelledby="github-copilot-section-title"
              style={{ order: getProviderOrderIndex(providerOrder, 'githubCopilot') }}
            >
              <div className="provider-section__header">
                <div>
                  <span className="provider-section__eyebrow">Connected provider</span>
                  <h2 id="github-copilot-section-title">
                    <BrandIcon src={GITHUB_COPILOT_ICON} alt="" size="small" />
                    GitHub Copilot
                  </h2>
                </div>
                <div className="button-row">
                  <button type="button" onClick={onRefreshAllCopilot} disabled={copilotBusy || copilotAccounts.length === 0}>
                    <RefreshCcw size={15} />
                    Refresh
                  </button>
                  <button type="button" onClick={onOpenCopilotAccounts}>
                    <Users size={15} />
                    View all accounts
                  </button>
                </div>
              </div>

              <div className="dashboard-grid">
                {visibleCopilotAccounts.map((account) => (
                  <CopilotUsageCard
                    key={account.id}
                    account={account}
                    busy={copilotBusy}
                    pinned={pinnedAccounts.has(account.id)}
                    dashboardMode={true}
                    onRefresh={() => onRefreshCopilotAccount(account.id)}
                    onRemove={() => onRemoveCopilotAccount(account.id)}
                    onTogglePin={() => onTogglePinnedAccount(account.id)}
                  />
                ))}
              </div>
            </section>
          ) : null}

          {codexAccounts.length > 0 && !hiddenProviders.has('codex') ? (
            <section
              className="provider-section"
              aria-labelledby="codex-section-title"
              style={{ order: getProviderOrderIndex(providerOrder, 'codex') }}
            >
              <div className="provider-section__header">
                <div>
                  <span className="provider-section__eyebrow">Connected provider</span>
                  <h2 id="codex-section-title">
                    <BrandIcon src={CODEX_ICON} alt="" size="small" />
                    Codex
                  </h2>
                </div>
                <div className="button-row">
                  <button type="button" onClick={onRefreshAllCodex} disabled={codexBusy || codexAccounts.length === 0}>
                    <RefreshCcw size={15} />
                    Refresh
                  </button>
                  <button type="button" onClick={onOpenCodexAccounts}>
                    <Users size={15} />
                    View all accounts
                  </button>
                </div>
              </div>

              <div className="dashboard-grid">
                {visibleCodexAccounts.map((account) => (
                  <CodexUsageCard
                    key={account.id}
                    account={account}
                    busy={codexBusy}
                    pinned={pinnedAccounts.has(account.id)}
                    dashboardMode={true}
                    onRefresh={() => onRefreshCodexAccount(account.id)}
                    onRemove={() => onRemoveCodexAccount(account.id)}
                    onTogglePin={() => onTogglePinnedAccount(account.id)}
                  />
                ))}
              </div>
            </section>
          ) : null}

          {antigravityAccounts.length > 0 && !hiddenProviders.has('antigravity') ? (
            <section
              className="provider-section"
              aria-labelledby="antigravity-section-title"
              style={{ order: getProviderOrderIndex(providerOrder, 'antigravity') }}
            >
              <div className="provider-section__header">
                <div>
                  <span className="provider-section__eyebrow">Connected provider</span>
                  <h2 id="antigravity-section-title">
                    <BrandIcon src={ANTIGRAVITY_ICON} alt="" size="small" />
                    Antigravity
                  </h2>
                </div>
                <div className="button-row">
                  <button type="button" onClick={onRefreshAllAntigravity} disabled={antigravityBusy || antigravityAccounts.length === 0}>
                    <RefreshCcw size={15} />
                    Refresh
                  </button>
                  <button type="button" onClick={onOpenAntigravityAccounts}>
                    <Users size={15} />
                    View all accounts
                  </button>
                </div>
              </div>

              <div className="dashboard-grid">
                {visibleAntigravityAccounts.map((account) => (
                  <AntigravityUsageCard
                    key={account.id}
                    account={account}
                    busy={antigravityBusy}
                    pinned={pinnedAccounts.has(account.id)}
                    dashboardMode={true}
                    onRefresh={() => onRefreshAntigravityAccount(account.id)}
                    onRemove={() => onRemoveAntigravityAccount(account.id)}
                    onTogglePin={() => onTogglePinnedAccount(account.id)}
                  />
                ))}
              </div>
            </section>
          ) : null}

          {claudeAccounts.length > 0 && !hiddenProviders.has('claude') ? (
            <section
              className="provider-section"
              aria-labelledby="claude-section-title"
              style={{ order: getProviderOrderIndex(providerOrder, 'claude') }}
            >
              <div className="provider-section__header">
                <div>
                  <span className="provider-section__eyebrow">Connected provider</span>
                  <h2 id="claude-section-title">
                    <BrandIcon src={CLAUDE_ICON} alt="" size="small" />
                    Claude
                  </h2>
                </div>
                <div className="button-row">
                  <button type="button" onClick={onRefreshAllClaude} disabled={claudeBusy || claudeAccounts.length === 0}>
                    <RefreshCcw size={15} />
                    Refresh
                  </button>
                  <button type="button" onClick={onOpenClaudeAccounts}>
                    <Users size={15} />
                    View all accounts
                  </button>
                </div>
              </div>

              <div className="dashboard-grid">
                {visibleClaudeAccounts.map((account) => (
                  <ClaudeUsageCard
                    key={account.id}
                    account={account}
                    busy={claudeBusy}
                    pinned={pinnedAccounts.has(account.id)}
                    dashboardMode={true}
                    onRefresh={() => onRefreshClaudeAccount(account.id)}
                    onRemove={() => onRemoveClaudeAccount(account.id)}
                    onTogglePin={() => onTogglePinnedAccount(account.id)}
                  />
                ))}
              </div>
            </section>
          ) : null}

          {kiroAccounts.length > 0 && !hiddenProviders.has('kiro') ? (
            <section
              className="provider-section"
              aria-labelledby="kiro-section-title"
              style={{ order: getProviderOrderIndex(providerOrder, 'kiro') }}
            >
              <div className="provider-section__header">
                <div>
                  <span className="provider-section__eyebrow">Connected provider</span>
                  <h2 id="kiro-section-title">
                    <BrandIcon src={KIRO_ICON} alt="" size="small" />
                    Kiro
                  </h2>
                </div>
                <div className="button-row">
                  <button type="button" onClick={onRefreshAllKiro} disabled={kiroBusy || kiroAccounts.length === 0}>
                    <RefreshCcw size={15} />
                    Refresh
                  </button>
                  <button type="button" onClick={onOpenKiroAccounts}>
                    <Users size={15} />
                    View all accounts
                  </button>
                </div>
              </div>

              <div className="dashboard-grid">
                {visibleKiroAccounts.map((account) => (
                  <KiroUsageCard
                    key={account.id}
                    account={account}
                    busy={kiroBusy}
                    pinned={pinnedAccounts.has(account.id)}
                    dashboardMode={true}
                    onRefresh={() => onRefreshKiroAccount(account.id)}
                    onRemove={() => onRemoveKiroAccount(account.id)}
                    onTogglePin={() => onTogglePinnedAccount(account.id)}
                  />
                ))}
              </div>
            </section>
          ) : null}

          {cursorAccounts.length > 0 && !hiddenProviders.has('cursor') ? (
            <section
              className="provider-section"
              aria-labelledby="cursor-section-title"
              style={{ order: getProviderOrderIndex(providerOrder, 'cursor') }}
            >
              <div className="provider-section__header">
                <div>
                  <span className="provider-section__eyebrow">Connected provider</span>
                  <h2 id="cursor-section-title">
                    <BrandIcon src={CURSOR_ICON} alt="" size="small" />
                    Cursor
                  </h2>
                </div>
                <div className="button-row">
                  <button type="button" onClick={onRefreshAllCursor} disabled={cursorBusy || cursorAccounts.length === 0}>
                    <RefreshCcw size={15} />
                    Refresh
                  </button>
                  <button type="button" onClick={onOpenCursorAccounts}>
                    <Users size={15} />
                    View all accounts
                  </button>
                </div>
              </div>

              <div className="dashboard-grid">
                {visibleCursorAccounts.map((account) => (
                  <CursorUsageCard
                    key={account.id}
                    account={account}
                    busy={cursorBusy}
                    pinned={pinnedAccounts.has(account.id)}
                    dashboardMode={true}
                    onRefresh={() => onRefreshCursorAccount(account.id)}
                    onRemove={() => onRemoveCursorAccount(account.id)}
                    onTogglePin={() => onTogglePinnedAccount(account.id)}
                  />
                ))}
              </div>
            </section>
          ) : null}
        </div>
      )}
    </div>
  );
}

interface ProviderAccountsViewProps {
  viewMode: ViewMode;
  accounts: GitHubCopilotAccountSummary[];
  busy: boolean;
  error: string | null;
  pinnedAccounts: Set<string>;
  onBack: () => void;
  onOpenIntegrations: () => void;
  onRefreshAll: () => void;
  onRefreshAccount: (accountId: string) => void;
  onRemoveAccount: (accountId: string) => void;
  onTogglePinnedAccount: (accountId: string) => void;
}

function ProviderAccountsView({
  viewMode,
  accounts,
  busy,
  error,
  pinnedAccounts,
  onBack,
  onOpenIntegrations,
  onRefreshAll,
  onRefreshAccount,
  onRemoveAccount,
  onTogglePinnedAccount,
}: ProviderAccountsViewProps) {
  return (
    <div className="page-stack">
      <PageHeader
        title="GitHub Copilot Accounts"
        description="All connected Copilot accounts, with the same usage details shown on the dashboard."
        action={
          <div className="button-row">
            <button type="button" onClick={onBack}>
              <ArrowLeft size={15} />
              Dashboard
            </button>
            <button type="button" onClick={onRefreshAll} disabled={busy || accounts.length === 0}>
              <RefreshCcw size={15} />
              Refresh all
            </button>
            <button type="button" className="button-primary" onClick={onOpenIntegrations}>
              <Plus size={15} />
              Add account
            </button>
          </div>
        }
      />

      {error ? <p className="account-panel__error">{error}</p> : null}

      <section className="account-view-toolbar" aria-label="GitHub Copilot account summary">
        <div>
          <span>Total accounts</span>
          <strong>{accounts.length}</strong>
        </div>
        <div>
          <span>Provider</span>
          <strong>GitHub Copilot</strong>
        </div>
      </section>

      {accounts.length === 0 ? (
        <div className="empty-state empty-state--large">
          <BrandIcon src={GITHUB_COPILOT_ICON} alt="" size="large" />
          <strong>No GitHub Copilot accounts connected.</strong>
          <span>Connect Copilot and this page will show every authorized account.</span>
          <button type="button" className="button-primary" onClick={onOpenIntegrations}>
            <Plus size={15} />
            Open integrations
          </button>
        </div>
      ) : (
        <section className={`accounts-grid accounts-grid--${viewMode}`} aria-label="All GitHub Copilot accounts">
          {accounts.map((account) => (
            <CopilotUsageCard
              key={account.id}
              account={account}
              busy={busy}
              pinned={pinnedAccounts.has(account.id)}
              onRefresh={() => onRefreshAccount(account.id)}
              onRemove={() => onRemoveAccount(account.id)}
              onTogglePin={() => onTogglePinnedAccount(account.id)}
            />
          ))}
        </section>
      )}
    </div>
  );
}

interface CodexAccountsViewProps {
  viewMode: ViewMode;
  accounts: CodexAccountSummary[];
  busy: boolean;
  error: string | null;
  pinnedAccounts: Set<string>;
  onBack: () => void;
  onOpenIntegrations: () => void;
  onRefreshAll: () => void;
  onRefreshAccount: (accountId: string) => void;
  onRemoveAccount: (accountId: string) => void;
  onTogglePinnedAccount: (accountId: string) => void;
}

function CodexAccountsView({
  viewMode,
  accounts,
  busy,
  error,
  pinnedAccounts,
  onBack,
  onOpenIntegrations,
  onRefreshAll,
  onRefreshAccount,
  onRemoveAccount,
  onTogglePinnedAccount,
}: CodexAccountsViewProps) {
  return (
    <div className="page-stack">
      <PageHeader
        title="Codex Accounts"
        description="Imported local Codex accounts with safe quota summaries."
        action={
          <div className="button-row">
            <button type="button" onClick={onBack}>
              <ArrowLeft size={15} />
              Dashboard
            </button>
            <button type="button" onClick={onRefreshAll} disabled={busy || accounts.length === 0}>
              <RefreshCcw size={15} />
              Refresh all
            </button>
            <button type="button" className="button-primary" onClick={onOpenIntegrations}>
              <Plus size={15} />
              Add account
            </button>
          </div>
        }
      />

      {error ? <p className="account-panel__error">{error}</p> : null}

      <section className="account-view-toolbar" aria-label="Codex account summary">
        <div>
          <span>Total accounts</span>
          <strong>{accounts.length}</strong>
        </div>
        <div>
          <span>Provider</span>
          <strong>Codex</strong>
        </div>
      </section>

      {accounts.length === 0 ? (
        <div className="empty-state empty-state--large">
          <BrandIcon src={CODEX_ICON} alt="" size="large" />
          <strong>No Codex accounts imported.</strong>
          <span>Import your local Codex auth file and Quota will show usage windows here.</span>
          <button type="button" className="button-primary" onClick={onOpenIntegrations}>
            <Plus size={15} />
            Open integrations
          </button>
        </div>
      ) : (
        <section className={`accounts-grid accounts-grid--${viewMode}`} aria-label="All Codex accounts">
          {accounts.map((account) => (
            <CodexUsageCard
              key={account.id}
              account={account}
              busy={busy}
              pinned={pinnedAccounts.has(account.id)}
              onRefresh={() => onRefreshAccount(account.id)}
              onRemove={() => onRemoveAccount(account.id)}
              onTogglePin={() => onTogglePinnedAccount(account.id)}
            />
          ))}
        </section>
      )}
    </div>
  );
}

interface AntigravityAccountsViewProps {
  viewMode: ViewMode;
  accounts: AntigravityAccountSummary[];
  busy: boolean;
  error: string | null;
  pinnedAccounts: Set<string>;
  onBack: () => void;
  onOpenIntegrations: () => void;
  onRefreshAll: () => void;
  onRefreshAccount: (accountId: string) => void;
  onRemoveAccount: (accountId: string) => void;
  onTogglePinnedAccount: (accountId: string) => void;
}

function AntigravityAccountsView({
  viewMode,
  accounts,
  busy,
  error,
  pinnedAccounts,
  onBack,
  onOpenIntegrations,
  onRefreshAll,
  onRefreshAccount,
  onRemoveAccount,
  onTogglePinnedAccount,
}: AntigravityAccountsViewProps) {
  return (
    <div className="page-stack">
      <PageHeader
        title="Antigravity Accounts"
        description="Imported local Antigravity accounts with safe quota summaries."
        action={
          <div className="button-row">
            <button type="button" onClick={onBack}>
              <ArrowLeft size={15} />
              Dashboard
            </button>
            <button type="button" onClick={onRefreshAll} disabled={busy || accounts.length === 0}>
              <RefreshCcw size={15} />
              Refresh all
            </button>
            <button type="button" className="button-primary" onClick={onOpenIntegrations}>
              <Plus size={15} />
              Add account
            </button>
          </div>
        }
      />

      {error ? <p className="account-panel__error">{error}</p> : null}

      <section className="account-view-toolbar" aria-label="Antigravity account summary">
        <div>
          <span>Total accounts</span>
          <strong>{accounts.length}</strong>
        </div>
        <div>
          <span>Provider</span>
          <strong>Antigravity</strong>
        </div>
      </section>

      {accounts.length === 0 ? (
        <div className="empty-state empty-state--large">
          <BrandIcon src={ANTIGRAVITY_ICON} alt="" size="large" />
          <strong>No Antigravity accounts imported.</strong>
          <span>Import local Google/Gemini credentials and Quota will show Antigravity usage windows here.</span>
          <button type="button" className="button-primary" onClick={onOpenIntegrations}>
            <Plus size={15} />
            Open integrations
          </button>
        </div>
      ) : (
        <section className={`accounts-grid accounts-grid--${viewMode}`} aria-label="All Antigravity accounts">
          {accounts.map((account) => (
            <AntigravityUsageCard
              key={account.id}
              account={account}
              busy={busy}
              pinned={pinnedAccounts.has(account.id)}
              onRefresh={() => onRefreshAccount(account.id)}
              onRemove={() => onRemoveAccount(account.id)}
              onTogglePin={() => onTogglePinnedAccount(account.id)}
            />
          ))}
        </section>
      )}
    </div>
  );
}

interface ClaudeAccountsViewProps {
  viewMode: ViewMode;
  accounts: ClaudeAccountSummary[];
  busy: boolean;
  error: string | null;
  pinnedAccounts: Set<string>;
  onBack: () => void;
  onOpenIntegrations: () => void;
  onRefreshAll: () => void;
  onRefreshAccount: (accountId: string) => void;
  onRemoveAccount: (accountId: string) => void;
  onTogglePinnedAccount: (accountId: string) => void;
}

function ClaudeAccountsView({
  viewMode,
  accounts,
  busy,
  error,
  pinnedAccounts,
  onBack,
  onOpenIntegrations,
  onRefreshAll,
  onRefreshAccount,
  onRemoveAccount,
  onTogglePinnedAccount,
}: ClaudeAccountsViewProps) {
  return (
    <div className="page-stack">
      <PageHeader
        title="Claude Accounts"
        description="Connected Claude Code OAuth accounts with safe usage summaries."
        action={
          <div className="button-row">
            <button type="button" onClick={onBack}>
              <ArrowLeft size={15} />
              Dashboard
            </button>
            <button type="button" onClick={onRefreshAll} disabled={busy || accounts.length === 0}>
              <RefreshCcw size={15} />
              Refresh all
            </button>
            <button type="button" className="button-primary" onClick={onOpenIntegrations}>
              <Plus size={15} />
              Add account
            </button>
          </div>
        }
      />

      {error ? <p className="account-panel__error">{error}</p> : null}

      <section className="account-view-toolbar" aria-label="Claude account summary">
        <div>
          <span>Total accounts</span>
          <strong>{accounts.length}</strong>
        </div>
        <div>
          <span>Provider</span>
          <strong>Claude</strong>
        </div>
      </section>

      {accounts.length === 0 ? (
        <div className="empty-state empty-state--large">
          <BrandIcon src={CLAUDE_ICON} alt="" size="large" />
          <strong>No Claude accounts connected.</strong>
          <span>Connect Claude Code OAuth and Quota will show usage windows here.</span>
          <button type="button" className="button-primary" onClick={onOpenIntegrations}>
            <Plus size={15} />
            Open integrations
          </button>
        </div>
      ) : (
        <section className={`accounts-grid accounts-grid--${viewMode}`} aria-label="All Claude accounts">
          {accounts.map((account) => (
            <ClaudeUsageCard
              key={account.id}
              account={account}
              busy={busy}
              pinned={pinnedAccounts.has(account.id)}
              onRefresh={() => onRefreshAccount(account.id)}
              onRemove={() => onRemoveAccount(account.id)}
              onTogglePin={() => onTogglePinnedAccount(account.id)}
            />
          ))}
        </section>
      )}
    </div>
  );
}

interface KiroAccountsViewProps {
  viewMode: ViewMode;
  accounts: KiroAccountSummary[];
  busy: boolean;
  error: string | null;
  pinnedAccounts: Set<string>;
  onBack: () => void;
  onOpenIntegrations: () => void;
  onRefreshAll: () => void;
  onRefreshAccount: (accountId: string) => void;
  onRemoveAccount: (accountId: string) => void;
  onTogglePinnedAccount: (accountId: string) => void;
}

function KiroAccountsView({
  viewMode,
  accounts,
  busy,
  error,
  pinnedAccounts,
  onBack,
  onOpenIntegrations,
  onRefreshAll,
  onRefreshAccount,
  onRemoveAccount,
  onTogglePinnedAccount,
}: KiroAccountsViewProps) {
  return (
    <div className="page-stack">
      <PageHeader
        title="Kiro Accounts"
        description="Connected Kiro accounts with safe usage summaries."
        action={
          <div className="button-row">
            <button type="button" onClick={onBack}>
              <ArrowLeft size={15} />
              Dashboard
            </button>
            <button type="button" onClick={onRefreshAll} disabled={busy || accounts.length === 0}>
              <RefreshCcw size={15} />
              Refresh all
            </button>
            <button type="button" className="button-primary" onClick={onOpenIntegrations}>
              <Plus size={15} />
              Add account
            </button>
          </div>
        }
      />

      {error ? <p className="account-panel__error">{error}</p> : null}

      <section className="account-view-toolbar" aria-label="Kiro account summary">
        <div>
          <span>Total accounts</span>
          <strong>{accounts.length}</strong>
        </div>
        <div>
          <span>Provider</span>
          <strong>Kiro</strong>
        </div>
      </section>

      {accounts.length === 0 ? (
        <div className="empty-state empty-state--large">
          <BrandIcon src={KIRO_ICON} alt="" size="large" />
          <strong>No Kiro accounts connected.</strong>
          <span>Connect Kiro and this page will show every authorized account.</span>
          <button type="button" className="button-primary" onClick={onOpenIntegrations}>
            <Plus size={15} />
            Open integrations
          </button>
        </div>
      ) : (
        <section className={`accounts-grid accounts-grid--${viewMode}`} aria-label="All Kiro accounts">
          {accounts.map((account) => (
            <KiroUsageCard
              key={account.id}
              account={account}
              busy={busy}
              pinned={pinnedAccounts.has(account.id)}
              onRefresh={() => onRefreshAccount(account.id)}
              onRemove={() => onRemoveAccount(account.id)}
              onTogglePin={() => onTogglePinnedAccount(account.id)}
            />
          ))}
        </section>
      )}
    </div>
  );
}

interface ProviderSummaryGridProps {
  copilotCount: number;
  codexCount: number;
  antigravityCount: number;
  claudeCount: number;
  kiroCount: number;
  cursorCount: number;
  providerOrder: ProviderKey[];
  onOpenCopilotAccounts: () => void;
  onOpenCodexAccounts: () => void;
  onOpenAntigravityAccounts: () => void;
  onOpenClaudeAccounts: () => void;
  onOpenKiroAccounts: () => void;
  onOpenCursorAccounts: () => void;
}

function ProviderSummaryGrid({
  copilotCount,
  codexCount,
  antigravityCount,
  claudeCount,
  kiroCount,
  cursorCount,
  providerOrder,
  onOpenCopilotAccounts,
  onOpenCodexAccounts,
  onOpenAntigravityAccounts,
  onOpenClaudeAccounts,
  onOpenKiroAccounts,
  onOpenCursorAccounts,
}: ProviderSummaryGridProps) {
  const totalCount = copilotCount + codexCount + antigravityCount + claudeCount + kiroCount + cursorCount;
  const providerSummaries = PROVIDERS.map((provider) => {
    const connected =
      provider.key === 'githubCopilot'
        ? copilotCount
        : provider.key === 'codex'
          ? codexCount
          : provider.key === 'antigravity'
            ? antigravityCount
            : provider.key === 'claude'
              ? claudeCount
              : provider.key === 'kiro'
                ? kiroCount
                : cursorCount;
    const openAccounts =
      provider.key === 'githubCopilot'
        ? onOpenCopilotAccounts
        : provider.key === 'codex'
          ? onOpenCodexAccounts
          : provider.key === 'antigravity'
            ? onOpenAntigravityAccounts
            : provider.key === 'claude'
              ? onOpenClaudeAccounts
              : provider.key === 'kiro'
                ? onOpenKiroAccounts
                : onOpenCursorAccounts;

    return { ...provider, connected, openAccounts };
  }).sort((a, b) => getProviderOrderIndex(providerOrder, a.key) - getProviderOrderIndex(providerOrder, b.key));

  return (
    <section className="summary-grid" aria-label="Connected account totals">
      <article className="summary-card summary-card--total">
        <div className="summary-card__icon">
          <Users size={19} />
        </div>
        <div>
          <span>Total accounts</span>
          <strong>{totalCount}</strong>
        </div>
      </article>

      {providerSummaries.map((provider) => (
        <button
          type="button"
          className="summary-card summary-card--button"
          key={provider.key}
          onClick={provider.openAccounts}
          aria-label={`Open ${provider.name} accounts`}
        >
          <div className="summary-card__icon">
            <BrandIcon src={provider.iconPath} alt="" />
          </div>
          <div>
            <span>{provider.name}</span>
            <strong>{provider.connected}</strong>
          </div>
        </button>
      ))}
    </section>
  );
}

interface IntegrationsViewProps {
  connectedCount: number;
  codexConnectedCount: number;
  antigravityConnectedCount: number;
  claudeConnectedCount: number;
  kiroConnectedCount: number;
  cursorConnectedCount: number;
  copilotBusy: boolean;
  codexBusy: boolean;
  antigravityBusy: boolean;
  claudeBusy: boolean;
  kiroBusy: boolean;
  cursorBusy: boolean;
  copilotError: string | null;
  codexError: string | null;
  antigravityError: string | null;
  claudeError: string | null;
  kiroError: string | null;
  cursorError: string | null;
  copilotLogin: GitHubCopilotOAuthStartResponse | null;
  codexLogin: CodexOAuthStartResponse | null;
  antigravityLogin: AntigravityOAuthStartResponse | null;
  claudeLogin: ClaudeOAuthStartResponse | null;
  kiroLogin: KiroOAuthStartResponse | null;
  cursorLogin: CursorOAuthStartResponse | null;
  claudeCallbackInput: string;
  claudeEmailHint: string;
  onStartCopilotAuth: () => void;
  onStartCodexAuth: () => void;
  onStartAntigravityAuth: () => void;
  onStartClaudeAuth: () => void;
  onStartKiroAuth: () => void;
  onStartCursorAuth: () => void;
  onImportLocalCodex: () => void;
  onImportLocalAntigravity: () => void;
  onImportLocalKiro: () => void;
  onImportLocalCursor: () => void;
  onOpenCopilotAuthUrl: () => void;
  onOpenCodexAuthUrl: () => void;
  onOpenAntigravityAuthUrl: () => void;
  onOpenClaudeAuthUrl: () => void;
  onOpenKiroAuthUrl: () => void;
  onOpenCursorAuthUrl: () => void;
  onCompleteCopilotAuth: () => void;
  onCompleteCodexAuth: () => void;
  onCompleteAntigravityAuth: () => void;
  onCompleteClaudeAuth: () => void;
  onCompleteKiroAuth: () => void;
  onCompleteCursorAuth: () => void;
  onCancelCopilotAuth: () => void;
  onCancelCodexAuth: () => void;
  onCancelAntigravityAuth: () => void;
  onCancelClaudeAuth: () => void;
  onCancelKiroAuth: () => void;
  onCancelCursorAuth: () => void;
  onClaudeCallbackInputChange: (value: string) => void;
  onClaudeEmailHintChange: (value: string) => void;
}

function IntegrationsView({
  connectedCount,
  codexConnectedCount,
  antigravityConnectedCount,
  claudeConnectedCount,
  kiroConnectedCount,
  cursorConnectedCount,
  copilotBusy,
  codexBusy,
  antigravityBusy,
  claudeBusy,
  kiroBusy,
  cursorBusy,
  copilotError,
  codexError,
  antigravityError,
  claudeError,
  kiroError,
  cursorError,
  copilotLogin,
  codexLogin,
  antigravityLogin,
  claudeLogin,
  kiroLogin,
  cursorLogin,
  claudeCallbackInput,
  claudeEmailHint,
  onStartCopilotAuth,
  onStartCodexAuth,
  onStartAntigravityAuth,
  onStartClaudeAuth,
  onStartKiroAuth,
  onStartCursorAuth,
  onImportLocalCodex,
  onImportLocalAntigravity,
  onImportLocalKiro,
  onImportLocalCursor,
  onOpenCopilotAuthUrl,
  onOpenCodexAuthUrl,
  onOpenAntigravityAuthUrl,
  onOpenClaudeAuthUrl,
  onOpenKiroAuthUrl,
  onOpenCursorAuthUrl,
  onCompleteCopilotAuth,
  onCompleteCodexAuth,
  onCompleteAntigravityAuth,
  onCompleteClaudeAuth,
  onCompleteKiroAuth,
  onCompleteCursorAuth,
  onCancelCopilotAuth,
  onCancelCodexAuth,
  onCancelAntigravityAuth,
  onCancelClaudeAuth,
  onCancelKiroAuth,
  onCancelCursorAuth,
  onClaudeCallbackInputChange,
  onClaudeEmailHintChange,
}: IntegrationsViewProps) {
  return (
    <div className="page-stack">
      <PageHeader
        title="Integrations"
        description="Connect providers here. The dashboard stays focused on accounts that are already authorized."
      />

      {copilotError ? <p className="account-panel__error">{copilotError}</p> : null}
      {codexError ? <p className="account-panel__error">{codexError}</p> : null}
      {antigravityError ? <p className="account-panel__error">{antigravityError}</p> : null}
      {claudeError ? <p className="account-panel__error">{claudeError}</p> : null}
      {kiroError ? <p className="account-panel__error">{kiroError}</p> : null}
      {cursorError ? <p className="account-panel__error">{cursorError}</p> : null}

      {copilotLogin ? (
        <div className="auth-panel">
          <div>
            <span className="auth-panel__label">GitHub device code</span>
            <strong>{copilotLogin.userCode}</strong>
          </div>
          <p>Open GitHub, authorize Quota, then return here and complete the import.</p>
          <div className="button-row">
            <button type="button" onClick={onOpenCopilotAuthUrl} disabled={copilotBusy}>
              <ExternalLink size={15} />
              Open GitHub
            </button>
            <button type="button" className="button-primary" onClick={onCompleteCopilotAuth} disabled={copilotBusy}>
              Complete import
            </button>
            <button type="button" onClick={onCancelCopilotAuth} disabled={copilotBusy}>
              Cancel
            </button>
          </div>
        </div>
      ) : null}

      {codexLogin ? (
        <div className="auth-panel">
          <div>
            <span className="auth-panel__label">Codex browser login</span>
            <strong>OpenAI</strong>
          </div>
          <p>Open OpenAI, finish the Codex authorization, then return here once the browser says Codex connected.</p>
          <div className="button-row">
            <button type="button" onClick={onOpenCodexAuthUrl} disabled={codexBusy}>
              <ExternalLink size={15} />
              Open OpenAI
            </button>
            <button type="button" className="button-primary" onClick={onCompleteCodexAuth} disabled={codexBusy}>
              Complete connection
            </button>
            <button type="button" onClick={onCancelCodexAuth} disabled={codexBusy}>
              Cancel
            </button>
          </div>
        </div>
      ) : null}

      {antigravityLogin ? (
        <div className="auth-panel">
          <div>
            <span className="auth-panel__label">Antigravity Google login</span>
            <strong>Google</strong>
          </div>
          <p>Open Google, authorize Cloud Code Assist access, then return here once the browser says Antigravity connected.</p>
          <div className="button-row">
            <button type="button" onClick={onOpenAntigravityAuthUrl} disabled={antigravityBusy}>
              <ExternalLink size={15} />
              Open Google
            </button>
            <button type="button" className="button-primary" onClick={onCompleteAntigravityAuth} disabled={antigravityBusy}>
              Complete connection
            </button>
            <button type="button" onClick={onCancelAntigravityAuth} disabled={antigravityBusy}>
              Cancel
            </button>
          </div>
        </div>
      ) : null}

      {claudeLogin ? (
        <div className="auth-panel">
          <div>
            <span className="auth-panel__label">Claude Code OAuth</span>
            <strong>Claude</strong>
          </div>
          <p>Open Claude, authorize Quota, then paste the redirected callback URL or the code value here.</p>
          <div className="auth-panel__fields">
            <label>
              <span>Callback URL or code</span>
              <textarea
                value={claudeCallbackInput}
                onChange={(event) => onClaudeCallbackInputChange(event.currentTarget.value)}
                placeholder="https://platform.claude.com/oauth/code/callback?code=..."
                rows={3}
              />
            </label>
            <label>
              <span>Email hint</span>
              <input
                type="email"
                value={claudeEmailHint}
                onChange={(event) => onClaudeEmailHintChange(event.currentTarget.value)}
                placeholder="optional@example.com"
              />
            </label>
          </div>
          <div className="button-row">
            <button type="button" onClick={onOpenClaudeAuthUrl} disabled={claudeBusy}>
              <ExternalLink size={15} />
              Open Claude
            </button>
            <button
              type="button"
              className="button-primary"
              onClick={onCompleteClaudeAuth}
              disabled={claudeBusy || claudeCallbackInput.trim().length === 0}
            >
              Complete connection
            </button>
            <button type="button" onClick={onCancelClaudeAuth} disabled={claudeBusy}>
              Cancel
            </button>
          </div>
        </div>
      ) : null}

      {kiroLogin ? (
        <div className="auth-panel">
          <div>
            <span className="auth-panel__label">Kiro browser login</span>
            <strong>Kiro</strong>
          </div>
          <p>Open Kiro, sign in with your provider, then return here once the browser says you are connected.</p>
          <div className="button-row">
            <button type="button" onClick={onOpenKiroAuthUrl} disabled={kiroBusy}>
              <ExternalLink size={15} />
              Open Kiro
            </button>
            <button type="button" className="button-primary" onClick={onCompleteKiroAuth} disabled={kiroBusy}>
              Complete connection
            </button>
            <button type="button" onClick={onCancelKiroAuth} disabled={kiroBusy}>
              Cancel
            </button>
          </div>
        </div>
      ) : null}

      {cursorLogin ? (
        <div className="auth-panel">
          <div>
            <span className="auth-panel__label">Cursor browser login</span>
            <strong>Cursor</strong>
          </div>
          <p>Open Cursor, authorize Quota, then return here once the browser says you are connected.</p>
          <div className="button-row">
            <button type="button" onClick={onOpenCursorAuthUrl} disabled={cursorBusy}>
              <ExternalLink size={15} />
              Open Cursor
            </button>
            <button type="button" className="button-primary" onClick={onCompleteCursorAuth} disabled={cursorBusy}>
              Complete connection
            </button>
            <button type="button" onClick={onCancelCursorAuth} disabled={cursorBusy}>
              Cancel
            </button>
          </div>
        </div>
      ) : null}

      <section className="integration-list" aria-label="Available integrations">
        {integrations.map((integration) => {
          const isCopilot = integration.name === 'GitHub Copilot';
          const isCodex = integration.name === 'Codex';
          const isAntigravity = integration.name === 'Antigravity';
          const isClaude = integration.name === 'Claude';
          const isKiro = integration.name === 'Kiro';
          const isCursor = integration.name === 'Cursor';
          const copilotCount = connectedCount - codexConnectedCount - antigravityConnectedCount - claudeConnectedCount - kiroConnectedCount - cursorConnectedCount;
          return (
            <article className="integration-row" key={integration.name}>
              <div className="integration-row__icon">
                <BrandIcon src={integration.iconPath} alt="" />
              </div>
              <div className="integration-row__body">
                <div>
                  <h2>{integration.name}</h2>
                  <span>
                    {isCopilot
                      ? `${copilotCount} connected`
                      : isCodex
                        ? `${codexConnectedCount} imported`
                        : isAntigravity
                          ? `${antigravityConnectedCount} imported`
                          : isClaude
                            ? `${claudeConnectedCount} connected`
                            : isKiro
                              ? `${kiroConnectedCount} connected`
                              : isCursor
                                ? `${cursorConnectedCount} connected`
                                : integration.status}
                  </span>
                </div>
                <p>{integration.description}</p>
              </div>
              {isCopilot ? (
                <button type="button" className="button-primary" onClick={onStartCopilotAuth} disabled={copilotBusy}>
                  <Github size={15} />
                  Connect
                </button>
              ) : isCodex ? (
                <div className="button-row integration-row__actions">
                  <button type="button" className="button-primary" onClick={onStartCodexAuth} disabled={codexBusy}>
                    <ExternalLink size={15} />
                    Connect
                  </button>
                  <button type="button" onClick={onImportLocalCodex} disabled={codexBusy}>
                    <Plus size={15} />
                    Import local
                  </button>
                </div>
              ) : isAntigravity ? (
                <div className="button-row integration-row__actions">
                  <button type="button" className="button-primary" onClick={onStartAntigravityAuth} disabled={antigravityBusy}>
                    <ExternalLink size={15} />
                    Connect
                  </button>
                  <button type="button" onClick={onImportLocalAntigravity} disabled={antigravityBusy}>
                    <Plus size={15} />
                    Import local
                  </button>
                </div>
              ) : isClaude ? (
                <button type="button" className="button-primary" onClick={onStartClaudeAuth} disabled={claudeBusy}>
                  <ExternalLink size={15} />
                  Connect
                </button>
              ) : isKiro ? (
                <div className="button-row integration-row__actions">
                  <button type="button" className="button-primary" onClick={onStartKiroAuth} disabled={kiroBusy}>
                    <ExternalLink size={15} />
                    Connect
                  </button>
                  <button type="button" onClick={onImportLocalKiro} disabled={kiroBusy}>
                    <Plus size={15} />
                    Import local
                  </button>
                </div>
              ) : isCursor ? (
                <div className="button-row integration-row__actions">
                  <button type="button" className="button-primary" onClick={onStartCursorAuth} disabled={cursorBusy}>
                    <ExternalLink size={15} />
                    Connect
                  </button>
                  <button type="button" onClick={onImportLocalCursor} disabled={cursorBusy}>
                    <Plus size={15} />
                    Import local
                  </button>
                </div>
              ) : (
                <button type="button" disabled>
                  Planned
                </button>
              )}
            </article>
          );
        })}
      </section>
    </div>
  );
}

interface PageHeaderProps {
  title: string;
  description: string;
  action?: ReactNode;
}

function PageHeader({ title, description, action }: PageHeaderProps) {
  return (
    <header className="page-header">
      <div>
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      {action}
    </header>
  );
}

interface CopilotUsageCardProps {
  account: GitHubCopilotAccountSummary;
  busy: boolean;
  pinned: boolean;
  dashboardMode?: boolean;
  onRefresh: () => void;
  onRemove: () => void;
  onTogglePin: () => void;
}

function CopilotUsageCard({ account, busy, pinned, dashboardMode = false, onRefresh, onRemove, onTogglePin }: CopilotUsageCardProps) {
  const resetText = useMemo(() => formatResetLine(account.usage.allowanceResetAt), [account.usage.allowanceResetAt]);

  return (
    <article className="usage-card">
      <div className="usage-card__header">
        <div>
          <span className="usage-card__provider">
            <BrandIcon src={GITHUB_COPILOT_ICON} alt="" size="small" />
            GitHub Copilot
          </span>
          <h2>{account.githubEmail || account.githubLogin}</h2>
          <p>
            @{account.githubLogin}
            {account.plan ? ` · ${formatPlan(account.plan)}` : ''}
          </p>
        </div>
        <div className="button-row usage-card__actions">
          {(!dashboardMode || pinned) ? (
            <button
              type="button"
              className={pinned ? 'usage-card__pin usage-card__pin--pinned' : 'usage-card__pin'}
              onClick={onTogglePin}
              aria-label={pinned ? 'Unpin account from dashboard' : 'Pin account to dashboard'}
            >
              {pinned ? <PinOff size={14} /> : <Pin size={14} />}
            </button>
          ) : null}
          <button type="button" onClick={onRemove} disabled={busy} aria-label="Remove GitHub Copilot account">
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      <div className="usage-card__rows">
        <UsageMetricRow
          label="Inline suggestions"
          percent={account.usage.inlineSuggestionsUsedPercent}
          included={account.usage.inlineIncluded}
          remaining={account.usage.remainingCompletions}
          total={account.usage.totalCompletions}
          resetText={resetText}
        />
        <UsageMetricRow
          label="Chat messages"
          percent={account.usage.chatMessagesUsedPercent}
          included={account.usage.chatIncluded}
          remaining={account.usage.remainingChat}
          total={account.usage.totalChat}
          resetText={resetText}
        />
        <UsageMetricRow
          label="Premium requests"
          percent={account.usage.premiumRequestsUsedPercent}
          included={account.usage.premiumIncluded}
          remaining={account.usage.remainingPremiumRequests}
          total={account.usage.totalPremiumRequests}
          used={account.usage.usedPremiumRequests}
          resetText={resetText}
          emphasized
        />
      </div>
    </article>
  );
}

interface CodexUsageCardProps {
  account: CodexAccountSummary;
  busy: boolean;
  pinned: boolean;
  dashboardMode?: boolean;
  onRefresh: () => void;
  onRemove: () => void;
  onTogglePin: () => void;
}

function CodexUsageCard({ account, busy, pinned, dashboardMode = false, onRefresh, onRemove, onTogglePin }: CodexUsageCardProps) {
  return (
    <article className="usage-card">
      <div className="usage-card__header">
        <div>
          <span className="usage-card__provider">
            <BrandIcon src={CODEX_ICON} alt="" size="small" />
            Codex
          </span>
          <h2>{account.email}</h2>
          <p>
            {formatCodexAuthMode(account.authMode)}
            {account.plan ? ` · ${formatPlan(account.plan)}` : ''}
          </p>
        </div>
        <div className="button-row usage-card__actions">
          {(!dashboardMode || pinned) ? (
            <button
              type="button"
              className={pinned ? 'usage-card__pin usage-card__pin--pinned' : 'usage-card__pin'}
              onClick={onTogglePin}
              aria-label={pinned ? 'Unpin account from dashboard' : 'Pin account to dashboard'}
            >
              {pinned ? <PinOff size={14} /> : <Pin size={14} />}
            </button>
          ) : null}
          <button type="button" onClick={onRemove} disabled={busy} aria-label="Remove Codex account">
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      <div className="usage-card__rows">
        <CodexMetricRow
          label="Primary window"
          remaining={account.quota.hourlyRemainingPercent}
          resetAt={account.quota.hourlyResetAt}
          windowMinutes={account.quota.hourlyWindowMinutes}
        />
        <CodexMetricRow
          label="Weekly window"
          remaining={account.quota.weeklyRemainingPercent}
          resetAt={account.quota.weeklyResetAt}
          windowMinutes={account.quota.weeklyWindowMinutes}
        />
      </div>

      {account.quotaQueryLastError ? <p className="usage-card__error">{account.quotaQueryLastError}</p> : null}
    </article>
  );
}

interface ClaudeUsageCardProps {
  account: ClaudeAccountSummary;
  busy: boolean;
  pinned: boolean;
  dashboardMode?: boolean;
  onRefresh: () => void;
  onRemove: () => void;
  onTogglePin: () => void;
}

function ClaudeUsageCard({ account, busy, pinned, dashboardMode = false, onRefresh, onRemove, onTogglePin }: ClaudeUsageCardProps) {
  const extraUsageDetail = formatClaudeExtraUsage(account.quota);

  return (
    <article className="usage-card">
      <div className="usage-card__header">
        <div>
          <span className="usage-card__provider">
            <BrandIcon src={CLAUDE_ICON} alt="" size="small" />
            Claude
          </span>
          <h2>{account.email}</h2>
          <p>
            {formatClaudeAuthMode(account.authMode)}
            {account.planType ? ` · ${formatPlan(account.planType)}` : ''}
            {account.organizationName ? ` · ${account.organizationName}` : ''}
          </p>
        </div>
        <div className="button-row usage-card__actions">
          {(!dashboardMode || pinned) ? (
            <button
              type="button"
              className={pinned ? 'usage-card__pin usage-card__pin--pinned' : 'usage-card__pin'}
              onClick={onTogglePin}
              aria-label={pinned ? 'Unpin account from dashboard' : 'Pin account to dashboard'}
            >
              {pinned ? <PinOff size={14} /> : <Pin size={14} />}
            </button>
          ) : null}
          <button type="button" onClick={onRemove} disabled={busy} aria-label="Remove Claude account">
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      <div className="usage-card__rows">
        <ClaudeMetricRow
          label="Five hour window"
          remaining={account.quota.fiveHourRemainingPercent}
          resetAt={account.quota.fiveHourResetAt}
          windowText="5h window"
        />
        <ClaudeMetricRow
          label="Weekly window"
          remaining={account.quota.weeklyRemainingPercent}
          resetAt={account.quota.weeklyResetAt}
          windowText="7d window"
        />
        {account.quota.weeklySonnetRemainingPercent != null ? (
          <ClaudeMetricRow
            label="Weekly Sonnet"
            remaining={account.quota.weeklySonnetRemainingPercent}
            resetAt={account.quota.weeklySonnetResetAt}
            windowText="7d window"
          />
        ) : null}
        {account.quota.extraUsageRemainingPercent != null ? (
          <ClaudeMetricRow
            label="Extra usage"
            remaining={account.quota.extraUsageRemainingPercent}
            resetAt={account.quota.extraUsageResetAt}
            windowText={extraUsageDetail || 'Extra usage'}
          />
        ) : null}
      </div>

      {account.quotaQueryLastError ? <p className="usage-card__error">{account.quotaQueryLastError}</p> : null}
    </article>
  );
}

interface AntigravityUsageCardProps {
  account: AntigravityAccountSummary;
  busy: boolean;
  pinned: boolean;
  dashboardMode?: boolean;
  onRefresh: () => void;
  onRemove: () => void;
  onTogglePin: () => void;
}

function AntigravityUsageCard({ account, busy, pinned, dashboardMode = false, onRefresh, onRemove, onTogglePin }: AntigravityUsageCardProps) {
  const errors = getUniqueAntigravityErrors(account);

  return (
    <article className="usage-card">
      <div className="usage-card__header">
        <div>
          <span className="usage-card__provider">
            <BrandIcon src={ANTIGRAVITY_ICON} alt="" size="small" />
            Antigravity
          </span>
          <h2>{account.email}</h2>
          <p>
            {formatAntigravityAccountLabel(account)}
            {account.selectedAuthType ? ` · ${formatAntigravityAuthType(account.selectedAuthType)}` : ''}
          </p>
        </div>
        <div className="button-row usage-card__actions">
          {(!dashboardMode || pinned) ? (
            <button
              type="button"
              className={pinned ? 'usage-card__pin usage-card__pin--pinned' : 'usage-card__pin'}
              onClick={onTogglePin}
              aria-label={pinned ? 'Unpin account from dashboard' : 'Pin account to dashboard'}
            >
              {pinned ? <PinOff size={14} /> : <Pin size={14} />}
            </button>
          ) : null}
          <button type="button" onClick={onRemove} disabled={busy} aria-label="Remove Antigravity account">
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      <div className="usage-card__groups">
        <AntigravityQuotaGroup
          title="Gemini Models"
          weekly={account.quota.geminiWeekly}
          fiveHour={account.quota.geminiFiveHour}
        />
        <AntigravityQuotaGroup
          title="Claude and GPT models"
          weekly={account.quota.thirdPartyWeekly}
          fiveHour={account.quota.thirdPartyFiveHour}
        />
        <AntigravityCreditsGroup credits={account.credits} />
      </div>

      {errors.map((error) => (
        <p className="usage-card__error" key={error}>
          {error}
        </p>
      ))}
    </article>
  );
}

interface AntigravityCreditsGroupProps {
  credits: AntigravityCreditInfo[];
}

function AntigravityCreditsGroup({ credits }: AntigravityCreditsGroupProps) {
  const display = formatAntigravityCredits(credits);
  if (!display) return null;

  return (
    <section className="usage-card__group usage-card__group--credits" aria-label="Antigravity AI credits">
      <h3>Model Credits</h3>
      <div className="usage-metric usage-metric--credits">
        <div className="usage-metric__line">
          <span>Available AI Credits</span>
          <strong>{display}</strong>
        </div>
      </div>
    </section>
  );
}

interface AntigravityQuotaGroupProps {
  title: string;
  weekly: AntigravityQuotaWindow;
  fiveHour: AntigravityQuotaWindow;
}

function AntigravityQuotaGroup({ title, weekly, fiveHour }: AntigravityQuotaGroupProps) {
  return (
    <section className="usage-card__group" aria-label={title}>
      <h3>{title}</h3>
      <AntigravityMetricRow label="Five Hour Limit" window={fiveHour} />
      <AntigravityMetricRow label="Weekly Limit" window={weekly} />
    </section>
  );
}

interface AntigravityMetricRowProps {
  label: string;
  window: AntigravityQuotaWindow;
}

function AntigravityMetricRow({ label, window }: AntigravityMetricRowProps) {
  const remaining = window.remainingPercent;
  const remainingPercent = remaining == null ? null : Math.max(0, Math.min(100, remaining));
  const toneClass = remainingPercent != null && remainingPercent <= 20 ? ' usage-metric--remaining-low' : '';

  return (
    <div className={`usage-metric usage-metric--remaining${toneClass}`}>
      <div className="usage-metric__line">
        <span>{label}</span>
        <strong>{remaining == null ? '-' : `${remaining}%`}</strong>
      </div>
      <div className="usage-metric__bar" aria-hidden="true">
        <span style={{ width: `${remainingPercent ?? 0}%` }} />
      </div>
      <div className="usage-metric__meta">{formatResetLine(window.resetAt)}</div>
    </div>
  );
}

interface KiroUsageCardProps {
  account: KiroAccountSummary;
  busy: boolean;
  pinned: boolean;
  dashboardMode?: boolean;
  onRefresh: () => void;
  onRemove: () => void;
  onTogglePin: () => void;
}

function KiroUsageCard({ account, busy, pinned, dashboardMode = false, onRefresh, onRemove, onTogglePin }: KiroUsageCardProps) {
  const planLabel = getKiroPlanDisplayName(account.planName);
  const loginProviderText = account.loginProvider ? `Signed in with ${account.loginProvider}` : null;

  const creditsTotal = account.creditsTotal ?? 0;
  const creditsUsed = account.creditsUsed ?? 0;
  const creditsLeft = Math.max(0, creditsTotal - creditsUsed);
  const creditsUsedPct = creditsTotal > 0 ? Math.round((creditsUsed / creditsTotal) * 100) : 0;
  const creditsRemPct = 100 - creditsUsedPct;

  const bonusTotal = account.bonusTotal;
  const bonusUsed = account.bonusUsed ?? 0;

  return (
    <article className="usage-card">
      <div className="usage-card__header">
        <div>
          <span className="usage-card__provider">
            <BrandIcon src={KIRO_ICON} alt="" size="small" />
            Kiro
          </span>
          <h2>{account.email}</h2>
          <p>
            <span className="plan-badge">KIRO {planLabel}</span>
            {loginProviderText ? <> · {loginProviderText}</> : null}
          </p>
        </div>
        <div className="button-row usage-card__actions">
          {(!dashboardMode || pinned) ? (
            <button
              type="button"
              className={pinned ? 'usage-card__pin usage-card__pin--pinned' : 'usage-card__pin'}
              onClick={onTogglePin}
              aria-label={pinned ? 'Unpin account from dashboard' : 'Pin account to dashboard'}
            >
              {pinned ? <PinOff size={14} /> : <Pin size={14} />}
            </button>
          ) : null}
          <button type="button" onClick={onRemove} disabled={busy} aria-label="Remove Kiro account">
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      <div className="usage-card__rows">
        <KiroMetricRow
          label="User Prompt credits"
          usedPct={creditsUsedPct}
          remainingPct={creditsRemPct}
          used={creditsUsed}
          total={creditsTotal}
          left={creditsLeft}
          resetAt={account.usageResetAt}
        />
        {bonusTotal != null ? (
          <KiroMetricRow
            label="Add-on credits"
            usedPct={bonusTotal > 0 ? Math.round(((bonusUsed) / bonusTotal) * 100) : 0}
            remainingPct={bonusTotal > 0 ? Math.round(((bonusTotal - bonusUsed) / bonusTotal) * 100) : 0}
            used={bonusUsed}
            total={bonusTotal}
            left={Math.max(0, bonusTotal - bonusUsed)}
            resetAt={account.usageResetAt}
            expireDays={account.bonusExpireDays}
          />
        ) : null}
      </div>

      {account.quotaQueryLastError ? <p className="usage-card__error">{account.quotaQueryLastError}</p> : null}
    </article>
  );
}

interface KiroMetricRowProps {
  label: string;
  usedPct: number;
  remainingPct: number;
  used: number;
  total: number;
  left: number;
  resetAt?: number | null;
  expireDays?: number | null;
}

function KiroMetricRow({ label, usedPct, remainingPct, used, total, left, resetAt, expireDays }: KiroMetricRowProps) {
  const toneClass = usedPct >= 80 ? ' usage-metric--remaining-low' : '';
  const resetText = formatResetLine(resetAt);
  const expireText = expireDays != null ? ` · Expires in ${expireDays}d` : '';

  return (
    <div className={`usage-metric${toneClass}`}>
      <div className="usage-metric__line">
        <span>{label}</span>
        <strong>{usedPct}%</strong>
      </div>
      <div className="usage-metric__bar" aria-hidden="true">
        <span style={{ width: `${Math.max(0, Math.min(100, remainingPct))}%` }} />
      </div>
      <div className="usage-metric__meta">
        {used} / {total} used · {left} left
      </div>
      <div className="usage-metric__meta">
        {resetText}{expireText}
      </div>
    </div>
  );
}

function getUniqueAntigravityErrors(account: AntigravityAccountSummary) {
  return [account.quotaQueryLastError, account.statusReason]
    .filter((error): error is string => Boolean(error?.trim()))
    .filter((error, index, errors) => errors.indexOf(error) === index);
}

interface CodexMetricRowProps {
  label: string;
  remaining?: number | null;
  resetAt?: number | null;
  windowMinutes?: number | null;
}

function CodexMetricRow({ label, remaining, resetAt, windowMinutes }: CodexMetricRowProps) {
  const remainingPercent = remaining == null ? null : Math.max(0, Math.min(100, remaining));
  const value = remaining == null ? '-' : `${remaining}% left`;
  const toneClass = remainingPercent != null && remainingPercent <= 20 ? ' usage-metric--remaining-low' : '';
  const windowText = windowMinutes ? `${formatWindowMinutes(windowMinutes)} window` : 'Window unknown';
  const resetText = formatResetLine(resetAt);

  return (
    <div className={`usage-metric usage-metric--remaining${toneClass}`}>
      <div className="usage-metric__line">
        <span>{label}</span>
        <strong>{value}</strong>
      </div>
      <div className="usage-metric__bar" aria-hidden="true">
        <span style={{ width: `${remainingPercent ?? 0}%` }} />
      </div>
      <div className="usage-metric__meta">
        {windowText} · {resetText}
      </div>
    </div>
  );
}

interface ClaudeMetricRowProps {
  label: string;
  remaining?: number | null;
  resetAt?: number | null;
  windowText: string;
}

function ClaudeMetricRow({ label, remaining, resetAt, windowText }: ClaudeMetricRowProps) {
  const remainingPercent = remaining == null ? null : Math.max(0, Math.min(100, remaining));
  const value = remaining == null ? '-' : `${remaining}% left`;
  const toneClass = remainingPercent != null && remainingPercent <= 20 ? ' usage-metric--remaining-low' : '';
  const resetText = formatResetLine(resetAt);

  return (
    <div className={`usage-metric usage-metric--remaining${toneClass}`}>
      <div className="usage-metric__line">
        <span>{label}</span>
        <strong>{value}</strong>
      </div>
      <div className="usage-metric__bar" aria-hidden="true">
        <span style={{ width: `${remainingPercent ?? 0}%` }} />
      </div>
      <div className="usage-metric__meta">
        {windowText} · {resetText}
      </div>
    </div>
  );
}

interface UsageMetricRowProps {
  label: string;
  percent?: number | null;
  included?: boolean;
  remaining?: number | null;
  total?: number | null;
  used?: number | null;
  resetText: string;
  emphasized?: boolean;
}

function UsageMetricRow({
  label,
  percent,
  included = false,
  remaining,
  total,
  used,
  resetText,
  emphasized = false,
}: UsageMetricRowProps) {
  const computedUsed = used ?? (total != null && remaining != null ? Math.max(0, total - remaining) : null);
  const value = included
    ? 'Included'
    : computedUsed != null && total != null
      ? `${computedUsed} / ${total}`
      : percent != null
        ? `${percent}% used`
        : '-';

  return (
    <div className={`usage-metric ${emphasized ? 'usage-metric--emphasized' : ''}`}>
      <div className="usage-metric__line">
        <span>{label}</span>
        <strong>{value}</strong>
      </div>
      <div className="usage-metric__bar" aria-hidden="true">
        <span style={{ width: `${included ? 100 : percent ?? 0}%` }} />
      </div>
      <div className="usage-metric__meta">{resetText}</div>
    </div>
  );
}

interface BrandIconProps {
  src: string;
  alt: string;
  size?: 'small' | 'default' | 'large';
}

function BrandIcon({ src, alt, size = 'default' }: BrandIconProps) {
  return <img className={`brand-icon brand-icon--${size}`} src={src} alt={alt} />;
}

function formatPlan(plan: string) {
  const lower = plan.toLowerCase();
  if (lower.includes('individual')) return 'Pro';
  if (lower.includes('pro_plus') || lower.includes('pro+')) return 'Pro+';
  return plan.replace(/_/g, ' ');
}

function formatCodexAuthMode(authMode: string) {
  return authMode === 'apikey' ? 'API key' : 'OAuth';
}

function formatClaudeAuthMode(authMode: string) {
  return authMode === 'oauth' ? 'OAuth' : authMode;
}

function formatClaudeExtraUsage(quota: ClaudeQuotaSummary) {
  if (quota.extraUsageLimitCents == null && quota.extraUsageUsedCents == null) return '';
  const used = quota.extraUsageUsedCents == null ? '-' : formatCurrencyCents(quota.extraUsageUsedCents);
  const limit = quota.extraUsageLimitCents == null ? '-' : formatCurrencyCents(quota.extraUsageLimitCents);
  return `${used} / ${limit}`;
}

function formatCurrencyCents(cents: number) {
  return `$${(cents / 100).toFixed(2)}`;
}

function formatAntigravityAuthType(authType: string) {
  return authType.replace(/^oauth-/, '').replace(/-/g, ' ');
}

function formatAntigravityAccountLabel(account: AntigravityAccountSummary) {
  return (
    account.planName ||
    formatAntigravityTier(account.tierId) ||
    (account.source === 'oauth' ? 'OAuth account' : 'Local account')
  );
}

function formatAntigravityTier(tierId?: string | null) {
  if (!tierId) return '';
  const lower = tierId.toLowerCase();
  if (lower.includes('ultra')) return 'Ultra';
  if (lower.includes('pro') || lower.includes('premium')) return 'Pro';
  if (lower.includes('free') || lower === 'standard-tier') return 'Free';
  return tierId.replace(/-/g, ' ');
}

function formatAntigravityCredits(credits: AntigravityCreditInfo[]) {
  let total = 0;
  let hasValidAmount = false;

  for (const credit of credits) {
    if (credit.creditAmount == null) continue;
    const parsed = Number.parseFloat(String(credit.creditAmount).replace(/,/g, '').trim());
    if (!Number.isFinite(parsed)) continue;
    total += parsed;
    hasValidAmount = true;
  }

  if (!hasValidAmount) return '';
  return total.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

function formatWindowMinutes(minutes: number) {
  if (minutes % 1440 === 0) return `${minutes / 1440}d`;
  if (minutes % 60 === 0) return `${minutes / 60}h`;
  return `${minutes}m`;
}

function formatResetLine(seconds?: number | null) {
  if (!seconds) return 'Reset unknown';

  const resetDate = new Date(seconds * 1000);
  if (Number.isNaN(resetDate.getTime())) return 'Reset unknown';

  const now = Date.now();
  const diffMs = resetDate.getTime() - now;
  const absolute = formatShortDate(resetDate);
  if (diffMs <= 0) return `Reset due (${absolute})`;

  const totalMinutes = Math.floor(diffMs / 60000);
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  const relativeParts = [
    days > 0 ? `${days}d` : '',
    hours > 0 ? `${hours}h` : '',
    days === 0 && hours === 0 ? `${Math.max(1, minutes)}m` : '',
  ].filter(Boolean);

  return `${relativeParts.join(' ')} (${absolute})`;
}

function formatShortDate(date: Date) {
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function formatCursorResetDate(seconds?: number | null) {
  if (!seconds) return 'Reset: unknown';
  const date = new Date(seconds * 1000);
  if (Number.isNaN(date.getTime())) return 'Reset: unknown';
  const pad = (value: number) => String(value).padStart(2, '0');
  const month = pad(date.getMonth() + 1);
  const day = pad(date.getDate());
  const year = date.getFullYear();
  let hours = date.getHours();
  const ampm = hours >= 12 ? 'PM' : 'AM';
  hours = hours % 12;
  hours = hours ? hours : 12;
  const minutes = pad(date.getMinutes());
  return `Reset: ${month}/${day}/${year} ${pad(hours)}:${minutes} ${ampm}`;
}

export interface CursorUsageCardProps {
  account: CursorAccountSummary;
  busy: boolean;
  pinned: boolean;
  dashboardMode?: boolean;
  onRefresh: () => void;
  onRemove: () => void;
  onTogglePin: () => void;
}

export function CursorUsageCard({ account, busy, pinned, dashboardMode = false, onRefresh, onRemove, onTogglePin }: CursorUsageCardProps) {
  const badge = useMemo(() => getCursorPlanBadge(account.membershipType), [account.membershipType]);
  const resetText = useMemo(() => formatCursorResetDate(account.billingCycleEnd), [account.billingCycleEnd]);

  const planUsed = account.planUsed ?? 0;
  const planLimit = account.planLimit ?? 0;
  const totalUsageText = `$${planUsed.toFixed(2)} / $${planLimit.toFixed(2)}`;

  const totalPercent = account.totalPercent ?? 0;
  const autoPercent = account.autoPercent ?? 0;
  const apiPercent = account.apiPercent ?? 0;

  const onDemandText = account.onDemandEnabled
    ? `$${(account.onDemandUsed ?? 0).toFixed(2)}${account.onDemandLimit != null ? ` / $${account.onDemandLimit.toFixed(2)}` : ''}`
    : 'Disabled';

  const onDemandPercent = (account.onDemandEnabled && account.onDemandLimit && account.onDemandLimit > 0)
    ? Math.round(((account.onDemandUsed ?? 0) / account.onDemandLimit) * 100)
    : 0;

  return (
    <article className="usage-card">
      <div className="usage-card__header">
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <BrandIcon src={CURSOR_ICON} alt="" size="small" />
            <h2 style={{ margin: 0, fontSize: '1.05rem', fontWeight: 600 }}>{account.email || 'Cursor Account'}</h2>
            <span className="plan-badge" style={{ marginLeft: '4px' }}>{badge}</span>
          </div>
          {account.authId ? (
            <p style={{ marginTop: '10px', fontSize: '0.8rem', color: 'var(--subtle)', textOverflow: 'ellipsis', overflow: 'hidden', whiteSpace: 'nowrap', maxWidth: '300px' }}>
              Auth ID: {account.authId}
            </p>
          ) : null}
        </div>
        <div className="button-row usage-card__actions">
          {(!dashboardMode || pinned) ? (
            <button
              type="button"
              className={pinned ? 'usage-card__pin usage-card__pin--pinned' : 'usage-card__pin'}
              onClick={onTogglePin}
              aria-label={pinned ? 'Unpin account from dashboard' : 'Pin account to dashboard'}
            >
              {pinned ? <PinOff size={14} /> : <Pin size={14} />}
            </button>
          ) : null}
          <button type="button" onClick={onRemove} disabled={busy} aria-label="Remove Cursor account">
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      <div className="usage-card__rows">
        {/* Total Usage */}
        <div className="usage-metric">
          <div className="usage-metric__line">
            <span>Total Usage</span>
            <strong style={{ color: 'var(--success)' }}>{totalPercent}%</strong>
          </div>
          <div className="usage-metric__bar" aria-hidden="true">
            <span style={{ width: `${totalPercent}%`, background: 'var(--success)' }} />
          </div>
          <div className="usage-metric__meta" style={{ textAlign: 'left' }}>{totalUsageText}</div>
          <div className="usage-metric__meta" style={{ textAlign: 'left' }}>{resetText}</div>
        </div>

        {/* Auto + Composer */}
        <div className="usage-metric">
          <div className="usage-metric__line">
            <span>Auto + Composer</span>
            <strong style={{ color: 'var(--success)' }}>{autoPercent}%</strong>
          </div>
          <div className="usage-metric__bar" aria-hidden="true">
            <span style={{ width: `${autoPercent}%`, background: 'var(--success)' }} />
          </div>
        </div>

        {/* API Usage */}
        <div className="usage-metric">
          <div className="usage-metric__line">
            <span>API Usage</span>
            <strong style={{ color: 'var(--success)' }}>{apiPercent}%</strong>
          </div>
          <div className="usage-metric__bar" aria-hidden="true">
            <span style={{ width: `${apiPercent}%`, background: 'var(--success)' }} />
          </div>
        </div>

        {/* On-Demand */}
        <div className="usage-metric">
          <div className="usage-metric__line">
            <span>On-Demand</span>
            <strong style={{ color: account.onDemandEnabled ? 'var(--success)' : 'var(--subtle)' }}>{onDemandText}</strong>
          </div>
          <div className="usage-metric__bar" aria-hidden="true">
            <span style={{ width: `${onDemandPercent}%`, background: 'var(--success)' }} />
          </div>
        </div>
      </div>

      {account.quotaQueryLastError ? <p className="usage-card__error">{account.quotaQueryLastError}</p> : null}
    </article>
  );
}

export interface CursorAccountsViewProps {
  viewMode: ViewMode;
  accounts: CursorAccountSummary[];
  busy: boolean;
  error: string | null;
  pinnedAccounts: Set<string>;
  onBack: () => void;
  onOpenIntegrations: () => void;
  onRefreshAll: () => void;
  onRefreshAccount: (accountId: string) => void;
  onRemoveAccount: (accountId: string) => void;
  onTogglePinnedAccount: (accountId: string) => void;
}

export function CursorAccountsView({
  viewMode,
  accounts,
  busy,
  error,
  pinnedAccounts,
  onBack,
  onOpenIntegrations,
  onRefreshAll,
  onRefreshAccount,
  onRemoveAccount,
  onTogglePinnedAccount,
}: CursorAccountsViewProps) {
  return (
    <div className="page-stack">
      <PageHeader
        title="Cursor Accounts"
        description="Connected Cursor accounts with safe usage summaries."
        action={
          <div className="button-row">
            <button type="button" onClick={onBack}>
              <ArrowLeft size={15} />
              Dashboard
            </button>
            <button type="button" onClick={onRefreshAll} disabled={busy || accounts.length === 0}>
              <RefreshCcw size={15} />
              Refresh all
            </button>
            <button type="button" className="button-primary" onClick={onOpenIntegrations}>
              <Plus size={15} />
              Add account
            </button>
          </div>
        }
      />

      {error ? <p className="account-panel__error">{error}</p> : null}

      <section className="account-view-toolbar" aria-label="Cursor account summary">
        <div>
          <span>Total accounts</span>
          <strong>{accounts.length}</strong>
        </div>
        <div>
          <span>Provider</span>
          <strong>Cursor</strong>
        </div>
      </section>

      {accounts.length === 0 ? (
        <div className="empty-state empty-state--large">
          <BrandIcon src={CURSOR_ICON} alt="" size="large" />
          <strong>No Cursor accounts connected.</strong>
          <span>Connect Cursor and this page will show every authorized account.</span>
          <button type="button" className="button-primary" onClick={onOpenIntegrations}>
            <Plus size={15} />
            Open integrations
          </button>
        </div>
      ) : (
        <section className={`accounts-grid accounts-grid--${viewMode}`} aria-label="All Cursor accounts">
          {accounts.map((account) => (
            <CursorUsageCard
              key={account.id}
              account={account}
              busy={busy}
              pinned={pinnedAccounts.has(account.id)}
              onRefresh={() => onRefreshAccount(account.id)}
              onRemove={() => onRemoveAccount(account.id)}
              onTogglePin={() => onTogglePinnedAccount(account.id)}
            />
          ))}
        </section>
      )}
    </div>
  );
}
