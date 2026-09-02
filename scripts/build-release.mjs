/**
 * Builds every installer for the current version and records what came out.
 *
 * Run it with `npm run build:release`. It exists so a release build is one
 * command that always applies the same environment, instead of a checklist that
 * is easy to half-remember.
 *
 * Usage:
 *   npm run build:release
 *   npm run build:release -- --skip-tests
 *   npm run build:release -- --checksums-only
 *   npm run build:release -- --bundles deb,rpm
 *   npm run build:release -- -- --verbose      (anything after `--` goes to Tauri)
 */

import { spawnSync } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  findArtifacts,
  formatSize,
  missingTargets,
  renderChecksumFile,
  withChecksums,
} from './lib/release-artifacts.mjs';

const require = createRequire(import.meta.url);
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const tauriDir = path.join(repoRoot, 'src-tauri');
const bundleRoot = path.join(tauriDir, 'target', 'release', 'bundle');
const checksumFile = path.join(repoRoot, 'SHA256SUMS.txt');

function parseArguments(argv) {
  const options = {
    skipTests: false,
    skipChecksums: false,
    checksumsOnly: false,
    bundles: null,
    tauriArgs: [],
  };

  const separator = argv.indexOf('--');
  const flags = separator === -1 ? argv : argv.slice(0, separator);
  if (separator !== -1) options.tauriArgs = argv.slice(separator + 1);

  for (let index = 0; index < flags.length; index += 1) {
    const flag = flags[index];

    if (flag === '--skip-tests') {
      options.skipTests = true;
    } else if (flag === '--skip-checksums') {
      options.skipChecksums = true;
    } else if (flag === '--checksums-only') {
      options.checksumsOnly = true;
    } else if (flag === '--bundles') {
      index += 1;
      options.bundles = flags[index];
    } else if (flag.startsWith('--bundles=')) {
      options.bundles = flag.slice('--bundles='.length);
    } else {
      throw new Error(`Unknown option: ${flag}`);
    }
  }

  if (options.bundles && !options.bundles.trim()) {
    throw new Error('--bundles needs a comma-separated list, for example: --bundles deb,rpm');
  }

  return options;
}

const step = (() => {
  let current = 0;
  return (total, message) => {
    current += 1;
    process.stdout.write(`\n[${current}/${total}] ${message}\n`);
  };
})();

function run(command, args, { cwd = repoRoot, env } = {}) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: 'inherit',
    env: env ? { ...process.env, ...env } : process.env,
  });

  if (result.error) fail(`${command} could not run: ${result.error.message}`);
  if (result.status !== 0) fail(`${command} exited with code ${result.status ?? 'unknown'}`);
}

function fail(message) {
  process.stderr.write(`\nBuild stopped: ${message}\n`);
  process.exit(1);
}

async function readJson(filePath) {
  return JSON.parse(await readFile(filePath, 'utf8'));
}

/**
 * The root `package.json` version is the source of truth. A mismatch means the
 * build would produce installers with two different version numbers, so it is
 * worth catching before spending minutes on a compile.
 */
async function resolveVersion() {
  const { version } = await readJson(path.join(repoRoot, 'package.json'));
  if (!version) fail('package.json has no version field.');

  const tauriConfig = await readJson(path.join(tauriDir, 'tauri.conf.json'));
  const cargoToml = await readFile(path.join(tauriDir, 'Cargo.toml'), 'utf8');
  const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

  const mismatches = [];
  if (tauriConfig.version !== version) {
    mismatches.push(`src-tauri/tauri.conf.json has ${tauriConfig.version}`);
  }
  if (cargoVersion !== version) {
    mismatches.push(`src-tauri/Cargo.toml has ${cargoVersion}`);
  }

  if (mismatches.length > 0) {
    fail(
      `Version fields disagree. package.json has ${version}, but ${mismatches.join(' and ')}.`,
    );
  }

  return version;
}

/**
 * Environment for the bundling step.
 *
 * `NO_STRIP` is already set by `scripts/run-tauri.mjs`, because the bundled
 * `strip` cannot handle libraries with `.relr.dyn` sections. The two Linux
 * additions here guard the AppImage tooling itself: `APPIMAGE_EXTRACT_AND_RUN`
 * lets linuxdeploy run where FUSE is unavailable, such as a container, and
 * `ARCH` is what appimagetool reads to name the output. Both stay overridable.
 */
function bundleEnvironment() {
  if (process.platform !== 'linux') return {};

  return {
    APPIMAGE_EXTRACT_AND_RUN: process.env.APPIMAGE_EXTRACT_AND_RUN ?? '1',
    ARCH: process.env.ARCH ?? (process.arch === 'x64' ? 'x86_64' : process.arch),
  };
}

function typecheck() {
  run(process.execPath, [require.resolve('typescript/lib/tsc.js'), '--noEmit']);
}

function rustTests() {
  run('cargo', ['test', '--quiet'], { cwd: tauriDir });
}

function bundle(options) {
  const args = ['build'];
  if (options.bundles) args.push('--bundles', options.bundles);
  args.push(...options.tauriArgs);

  const environment = bundleEnvironment();
  for (const [name, value] of Object.entries(environment)) {
    process.stdout.write(`      ${name}=${value}\n`);
  }
  process.stdout.write('      NO_STRIP=true (from scripts/run-tauri.mjs)\n\n');

  run(process.execPath, [path.join(scriptDir, 'run-tauri.mjs'), ...args], { env: environment });
}

function reportArtifacts(artifacts, version) {
  if (artifacts.length === 0) {
    fail(
      `No installers for version ${version} were found under ${path.relative(repoRoot, bundleRoot)}. ` +
        'Check the bundling output above.',
    );
  }

  const widest = Math.max(...artifacts.map((artifact) => artifact.relativePath.length));
  for (const artifact of artifacts) {
    const size = formatSize(artifact.size).padStart(9);
    process.stdout.write(`      ${artifact.relativePath.padEnd(widest)}  ${size}\n`);
  }

  const { failed, unbuildable } = missingTargets(artifacts, process.platform);

  if (failed.length > 0) {
    process.stdout.write(
      `\n      Expected on this platform but not produced: ${failed.join(', ')}.\n` +
        '      Check the bundling output above before shipping.\n',
    );
  }

  if (unbuildable.length > 0) {
    process.stdout.write(
      `\n      Not buildable on ${process.platform}: ${unbuildable.join(', ')}.\n` +
        '      Build them on their own platform, copy them into the bundle directory,\n' +
        '      then run npm run checksums to record every installer in one manifest.\n',
    );
  }
}

async function writeChecksums(artifacts) {
  const hashed = await withChecksums(artifacts);
  await writeFile(checksumFile, renderChecksumFile(hashed), 'utf8');

  for (const artifact of hashed) {
    process.stdout.write(`      ${artifact.checksum.slice(0, 16)}…  ${artifact.fileName}\n`);
  }
  process.stdout.write(
    `\n      Wrote SHA256SUMS.txt with ${hashed.length} ` +
      `${hashed.length === 1 ? 'entry' : 'entries'}.\n` +
      '      Verify it with: sha256sum -c SHA256SUMS.txt\n',
  );
}

async function main() {
  const options = parseArguments(process.argv.slice(2));

  if (options.checksumsOnly && options.skipChecksums) {
    fail('--checksums-only and --skip-checksums cannot be used together.');
  }

  const version = await resolveVersion();
  const wantsChecksums = !options.skipChecksums;
  const buildSteps = options.checksumsOnly ? 0 : 2 + (options.skipTests ? 0 : 1);
  const total = buildSteps + 1 + (wantsChecksums ? 1 : 0);

  process.stdout.write(`\nQuota ${version} release build\n`);

  if (!options.checksumsOnly) {
    step(total, 'Type checking the frontend');
    typecheck();

    if (!options.skipTests) {
      step(total, 'Running the Rust test suite');
      rustTests();
    }

    step(total, 'Bundling installers');
    bundle(options);
  }

  step(total, `Collecting ${version} installers`);
  const artifacts = await findArtifacts(repoRoot, bundleRoot, version);
  reportArtifacts(artifacts, version);

  if (wantsChecksums) {
    step(total, 'Recording checksums');
    await writeChecksums(artifacts);
  }

  process.stdout.write('\nDone.\n');
}

main().catch((error) => fail(error.message));
