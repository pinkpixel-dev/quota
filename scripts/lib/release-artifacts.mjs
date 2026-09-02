import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { readdir, stat } from 'node:fs/promises';
import path from 'node:path';

/**
 * Bundle directories Tauri writes into, in the order they should appear in
 * `SHA256SUMS.txt`. Linux builds produce the first three, Windows the last two.
 */
export const BUNDLE_TARGETS = [
  { dir: 'appimage', label: 'AppImage', extension: '.appimage', platform: 'linux' },
  { dir: 'deb', label: 'Debian package', extension: '.deb', platform: 'linux' },
  { dir: 'rpm', label: 'RPM package', extension: '.rpm', platform: 'linux' },
  { dir: 'msi', label: 'Windows MSI', extension: '.msi', platform: 'win32' },
  { dir: 'nsis', label: 'Windows installer', extension: '.exe', platform: 'win32' },
];

/**
 * Matches a version inside an installer file name.
 *
 * Tauri writes two shapes: `Quota_1.2.2_amd64.AppImage` and
 * `Quota-1.2.2-1.x86_64.rpm`. Requiring a `_` or `-` on both sides keeps
 * `1.2.2` from matching `1.2.20`.
 */
function matchesVersion(fileName, version) {
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`(?:^|[_-])${escaped}(?:[_-]|$)`).test(path.parse(fileName).name);
}

async function listDirectory(directory) {
  try {
    return await readdir(directory);
  } catch (error) {
    if (error.code === 'ENOENT') return [];
    throw error;
  }
}

/**
 * Finds every installer in the bundle tree that belongs to `version`.
 *
 * Old versions are left in place by Tauri, so filtering by version is the only
 * way to describe the release that was just built.
 */
export async function findArtifacts(repoRoot, bundleRoot, version) {
  const found = [];

  for (const target of BUNDLE_TARGETS) {
    const directory = path.join(bundleRoot, target.dir);
    const entries = await listDirectory(directory);

    for (const entry of entries.sort()) {
      if (path.extname(entry).toLowerCase() !== target.extension) continue;
      if (!matchesVersion(entry, version)) continue;

      const absolutePath = path.join(directory, entry);
      const info = await stat(absolutePath);
      if (!info.isFile()) continue;

      found.push({
        target: target.dir,
        label: target.label,
        platform: target.platform,
        fileName: entry,
        absolutePath,
        relativePath: path.relative(repoRoot, absolutePath).split(path.sep).join('/'),
        size: info.size,
      });
    }
  }

  return found;
}

export function sha256(filePath) {
  return new Promise((resolve, reject) => {
    const hash = createHash('sha256');
    const stream = createReadStream(filePath);

    stream.on('error', reject);
    stream.on('data', (chunk) => hash.update(chunk));
    stream.on('end', () => resolve(hash.digest('hex')));
  });
}

/** Adds a `checksum` field to each artifact, reading the files once. */
export async function withChecksums(artifacts) {
  const results = [];
  for (const artifact of artifacts) {
    results.push({ ...artifact, checksum: await sha256(artifact.absolutePath) });
  }
  return results;
}

/**
 * Renders the `sha256sum -c` manifest. Two spaces between hash and path is what
 * `sha256sum` expects, and what the existing `SHA256SUMS.txt` uses.
 */
export function renderChecksumFile(artifacts) {
  return `${artifacts.map((a) => `${a.checksum}  ${a.relativePath}`).join('\n')}\n`;
}

export function formatSize(bytes) {
  const megabytes = bytes / 1024 / 1024;
  if (megabytes >= 1) return `${megabytes.toFixed(1)} MB`;
  return `${(bytes / 1024).toFixed(0)} KB`;
}

/**
 * Splits the bundle targets that produced nothing into two groups.
 *
 * `unbuildable` targets belong to another operating system and are expected to
 * be absent, so the manifest is simply incomplete until they are copied in.
 * `failed` targets should have been produced here and were not, which usually
 * means the bundling step went wrong.
 */
export function missingTargets(artifacts, platform) {
  const built = new Set(artifacts.map((artifact) => artifact.target));
  const missing = BUNDLE_TARGETS.filter((target) => !built.has(target.dir));

  return {
    failed: missing.filter((target) => target.platform === platform).map((t) => t.dir),
    unbuildable: missing.filter((target) => target.platform !== platform).map((t) => t.dir),
  };
}
