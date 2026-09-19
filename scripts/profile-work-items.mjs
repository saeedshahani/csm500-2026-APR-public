// AV-02/C01 diagnostic collection, not a final performance benchmark.
import { spawnSync, execFileSync } from 'node:child_process';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import os from 'node:os';
import { gzipSync } from 'node:zlib';

const dir = process.argv[2];
if (!dir || process.argv.length !== 3) throw Error('Usage: node scripts/profile-work-items.mjs NEW_OUTPUT_DIRECTORY');
const binary = resolve('target/release/examples/profile_work_items');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
mkdirSync(dir, { recursive: false });
const fingerprint = JSON.parse(execFileSync(process.execPath, ['scripts/fingerprint.mjs']));
// Preserve exact build inputs rather than relying on a dirty-tree hash alone.
const sourcePaths = [...new Set(execFileSync('git', ['ls-files', '-co', '--exclude-standard'], { encoding: 'utf8' }).trim().split('\n'))]
  .filter(p => /^(Cargo\.(toml|lock)|rust-toolchain\.toml|rustfmt\.toml)$/.test(p) || /^crates\/.*\.(rs|toml|graph|atoms|lambda|txt)$/.test(p)
    || /^examples\//.test(p) || ['scripts/fingerprint.mjs', 'scripts/profile-work-items.mjs'].includes(p)).sort();
const sources = Object.fromEntries(sourcePaths.map(p => [p, readFileSync(p).toString('base64')]));
const bundle = gzipSync(JSON.stringify({ schema: 'source-bundle-v1', encoding: 'base64', files: sources }));
writeFileSync(resolve(dir, 'sources.json.gz'), bundle, { flag: 'wx' });
const rows = [];
for (const items of [10, 100, 1000]) {
  for (const phase of ['load', 'validate', 'clone', 'observe', 'bindings', 'enabled', 'run', 'recorded_run']) {
    const args = [String(items), phase];
    // RSS covers the whole process, including fixture setup; wall_ns covers the named region.
    const timed = process.platform === 'darwin';
    const r = spawnSync(timed ? '/usr/bin/time' : binary, timed ? ['-l', binary, ...args] : args,
      { encoding: 'utf8', timeout: 30000, maxBuffer: 1024 * 1024 });
    const row = { items, phase, exit_code: r.status, signal: r.signal, stdout: r.stdout, stderr: r.stderr,
      error: r.error?.message ?? null, result: r.status === 0 ? JSON.parse(r.stdout) : null,
      peak_rss_bytes: timed ? Number(r.stderr.match(/(\d+)\s+maximum resident set size/)?.[1] ?? 0) || null : null };
    rows.push(row);
    console.log(items, phase, row.result?.outcome ?? row.error ?? row.signal);
  }
}
const record = { schema: 'work-item-profile-collection-v1', collected_at: new Date().toISOString(),
  purpose: 'Single diagnostic sample per cell; no inferential or final-build performance claim',
  commit: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  dirty_status: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }),
  fingerprint, source_bundle_sha256: sha(bundle), binary_sha256: sha(readFileSync(binary)),
  probe_sha256: sha(readFileSync('crates/engine/examples/profile_work_items.rs')),
  collector_sha256: sha(readFileSync('scripts/profile-work-items.mjs')),
  environment: { platform: process.platform, release: os.release(), arch: os.arch(), cpu: os.cpus()[0]?.model,
    memory: os.totalmem(), node: process.version, rustc: execFileSync('rustc', ['--version'], { encoding: 'utf8' }).trim() },
  protocol: { sizes: [10, 100, 1000], samples: 1, timeout_ms: 30000,
    os_memory_cap: null, memory_note: 'Peak RSS observation only; no OS hard memory cap in this local diagnostic.',
    observation_note: 'observe includes validation; run phases include runner setup but exclude source fixture generation.' }, rows };
writeFileSync(resolve(dir, 'collection.json'), JSON.stringify(record, null, 2) + '\n', { flag: 'wx' });
if (rows.some(r => r.exit_code !== 0)) process.exitCode = 1;
