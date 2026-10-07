// Release versions and download links. Update this file when a new release ships.

export const repo = 'https://github.com/pinkpixel-dev/quota';
export const releasesUrl = `${repo}/releases`;

export const desktop = {
  version: '1.7.0',
  get tag() {
    return `v${this.version}`;
  },
  get releaseUrl() {
    return `${releasesUrl}/tag/${this.tag}`;
  },
  get checksumsUrl() {
    return `${releasesUrl}/download/${this.tag}/SHA256SUMS.txt`;
  },
};

const desktopAsset = (name: string) => `${releasesUrl}/download/${desktop.tag}/${name}`;

export const desktopDownloads = [
  {
    os: 'Windows',
    label: 'Installer',
    file: `Quota_${desktop.version}_x64-setup.exe`,
    href: desktopAsset(`Quota_${desktop.version}_x64-setup.exe`),
  },
  {
    os: 'Windows',
    label: 'MSI',
    file: `Quota_${desktop.version}_x64_en-US.msi`,
    href: desktopAsset(`Quota_${desktop.version}_x64_en-US.msi`),
  },
  {
    os: 'Linux',
    label: 'AppImage',
    file: `Quota_${desktop.version}_amd64.AppImage`,
    href: desktopAsset(`Quota_${desktop.version}_amd64.AppImage`),
  },
  {
    os: 'Linux',
    label: 'Debian / Ubuntu',
    file: `Quota_${desktop.version}_amd64.deb`,
    href: desktopAsset(`Quota_${desktop.version}_amd64.deb`),
  },
  {
    os: 'Linux',
    label: 'Fedora / openSUSE',
    file: `Quota-${desktop.version}-1.x86_64.rpm`,
    href: desktopAsset(`Quota-${desktop.version}-1.x86_64.rpm`),
  },
];

export const vscode = {
  version: '1.4.0',
  // The .vsix is attached to whichever GitHub release shipped it.
  releaseTag: 'v1.7.0',
  marketplace: 'https://marketplace.visualstudio.com/items?itemName=pinkpixel.quota-ai-usage-tracker',
  openVsx: 'https://open-vsx.org/extension/pinkpixel/quota-ai-usage-tracker',
  get vsix() {
    return `${releasesUrl}/download/${this.releaseTag}/quota-ai-usage-tracker-${this.version}.vsix`;
  },
};

export const cli = {
  version: '0.2.1',
  crate: 'https://crates.io/crates/quota-cli',
  get tag() {
    return `quota-cli-v${this.version}`;
  },
  get releaseUrl() {
    return `${releasesUrl}/tag/${this.tag}`;
  },
  get linux() {
    return `${releasesUrl}/download/${this.tag}/quota-cli-x86_64-unknown-linux-gnu.tar.gz`;
  },
  get windows() {
    return `${releasesUrl}/download/${this.tag}/quota-cli-x86_64-pc-windows-msvc.zip`;
  },
};

export const herdr = {
  source: `${repo}/tree/main/herdr-plugin`,
  site: 'https://herdr.dev',
};

export const providers = [
  { id: 'claude', name: 'Claude Code', icon: 'claude' },
  { id: 'codex', name: 'Codex', icon: 'openai' },
  { id: 'githubCopilot', name: 'GitHub Copilot', icon: 'githubcopilot' },
  { id: 'cursor', name: 'Cursor', icon: 'cursor' },
  { id: 'antigravity', name: 'Antigravity', icon: 'antigravity' },
  { id: 'kiro', name: 'Kiro', icon: 'kiro' },
  { id: 'grok', name: 'Grok', icon: 'grok' },
  { id: 'opencodeGo', name: 'OpenCode Go', icon: 'opencode' },
] as const;

export const pinkpixel = {
  site: 'https://pinkpixel.dev',
  github: 'https://github.com/pinkpixel-dev',
  kofi: 'https://ko-fi.com/sizzlebop',
  bmac: 'https://www.buymeacoffee.com/pinkpixel',
};
