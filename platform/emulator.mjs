// Isolate CLI preferences from the operator's logged-in Firebase configuration.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const test = process.argv.includes('--test');
if (process.argv.slice(2).some(arg => arg !== '--test')) throw Error('Only --test is supported');
const config = fs.mkdtempSync(path.join(os.tmpdir(), 'graph-firebase-config-'));
const saved = path.join(root, 'output/auth-emulator');
const args = test ? ['emulators:exec', 'node --test platform/tests/emulator/*.test.mjs']
  : ['emulators:start', `--export-on-exit=${saved}`, ...(fs.existsSync(saved) ? [`--import=${saved}`] : [])];
const child = spawn(process.execPath, [path.join(root, 'platform/node_modules/firebase-tools/lib/bin/firebase.js'),
  ...args, '--only', 'auth,firestore,storage', '--project', 'demo-reflective-graph', '--config', path.join(root, 'platform/firebase.json'), '--non-interactive'],
{ cwd: root, stdio: 'inherit', env: { PATH: process.env.PATH, TMPDIR: os.tmpdir(), LANG: 'en_US.UTF-8',
  XDG_CONFIG_HOME: config, CLOUDSDK_CONFIG: config, METADATA_SERVER_DETECTION: 'none', CI: 'true', DEBUG: '', FIREBASE_CLI_DISABLE_UPDATE_CHECK: 'true' } });
// Forward termination so interactive development can export emulator users.
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
child.on('error', error => { console.error(error.message); process.exitCode = 1; fs.rmSync(config, { recursive: true, force: true }); });
child.on('exit', code => { fs.rmSync(config, { recursive: true, force: true }); process.exitCode = code ?? 1; });
