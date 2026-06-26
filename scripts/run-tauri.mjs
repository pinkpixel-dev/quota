import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const tauriCliPath = require.resolve('@tauri-apps/cli/tauri.js');

const result = spawnSync(process.execPath, [tauriCliPath, ...process.argv.slice(2)], {
  env: {
    ...process.env,
    NO_STRIP: process.env.NO_STRIP ?? 'true',
  },
  stdio: 'inherit',
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
