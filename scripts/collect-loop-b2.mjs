// Freeze the protocol and sources before running serial native acceptance cells.
import {spawn, execFileSync} from 'node:child_process';
import {readFileSync, writeFileSync, mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import os from 'node:os';
const [destination] = process.argv.slice(2);
if (!destination || process.argv.length !== 3) throw Error('Usage: node scripts/collect-loop-b2.mjs NEW_DIRECTORY');
const dir = resolve(destination), binary = resolve('target/release/examples/scale_probe');
const sha = b => createHash('sha256').update(b).digest('hex');
const protocol = {
  schema: 'loop-b-completion-v1', warmups: 1, samples: 3, seed: 42,
  timeout_ms: 600000, sampled_group_rss_ceiling_bytes: 6 * 1024 ** 3, hard_memory_cap: null,
  full_run_ladder: [100, 1000, 10000, 100000],
  gate: 'Every measured sample must finish NormalForm, preserve every input payload and exactly one status per item, and verify portable replay. A failed gate blocks larger full jobs.',
  comparison_sizes: [1000, 10000, 100000],
  envelope: 'Native only. Framed jobs above 100 use the explicit linear-volume recording/history/replay envelope in scale_probe.rs and initial/endpoint checkpoints, not default hosted bounds. Every event is recorded and independently checked. Whole-carrier recurrence remains enabled.',
  comparison: 'B.1 failures are retained separately. The same-size 100-item cell uses the original default observation/recording limits. Larger sizes are not equal-budget speedup comparisons.',
  million_gate: 'One million requires a separately authorised graph/trace envelope after the 100k gate and a peak-RSS/trace-size projection. It is a feasibility decision, not a promised measured result.',
  limitations: 'One active label-parametric one-shot processing rule; data and rules share the carrier. This is not arbitrary reflective-body throughput or a useful public-data transformation. Phase timings overlap internal replay; no causal attribution to individual optimisations.'
};
mkdirSync(dir, {recursive: false});
writeFileSync(resolve(dir, 'protocol.json'), JSON.stringify(protocol, null, 2) + '\n', {flag: 'wx'});
const paths = [...new Set(execFileSync('git', ['ls-files', '-co', '--exclude-standard'], {encoding: 'utf8'}).trim().split('\n'))]
  .filter(p => /^(Cargo\.(toml|lock)|rust-toolchain\.toml)$/.test(p) || /^crates\/.*\.(rs|toml|graph|atoms|lambda|txt)$/.test(p) || p.startsWith('examples/') || p === 'scripts/collect-loop-b2.mjs').sort();
const bundle = gzipSync(JSON.stringify({schema: 'source-bundle-v1', encoding: 'base64', files: Object.fromEntries(paths.map(p => [p, readFileSync(p).toString('base64')]))}));
writeFileSync(resolve(dir, 'sources.json.gz'), bundle, {flag: 'wx'});
const record = {protocol, source_bundle_sha256: sha(bundle), binary_sha256: sha(readFileSync(binary)), collected_at: new Date().toISOString(), environment: {platform: process.platform, arch: os.arch(), release: os.release(), cpu: os.cpus()[0]?.model, memory: os.totalmem()}, rows: [], gates: []};
const save = () => writeFileSync(resolve(dir, 'collection.json'), JSON.stringify(record, null, 2) + '\n');
async function cell(family, size, phase, events) {
  const rows = [];
  for (let sample = -1; sample < protocol.samples; sample++) {
    const row = await new Promise(done => {
      const args = [family, String(size), phase, String(events), String(protocol.seed)];
      const timed = process.platform === 'darwin';
      const p = spawn(timed ? '/usr/bin/time' : binary, timed ? ['-l', binary, ...args] : args, {detached: true, stdio: ['ignore', 'pipe', 'pipe']});
      let stdout = '', stderr = '', error = null, sampled_rss_peak = 0;
      const kill = message => { error = message; try { process.kill(-p.pid, 'SIGKILL'); } catch (e) { if (e.code !== 'ESRCH') throw e; } };
      const timer = setTimeout(() => kill('process-group deadline exceeded'), protocol.timeout_ms);
      const memory = setInterval(() => {
        try {
          const values = execFileSync('ps', ['-axo', 'pgid=,rss='], {encoding: 'utf8'}).trim().split('\n').map(s => s.trim().split(/\s+/).map(Number));
          const bytes = values.filter(([group]) => group === p.pid).reduce((sum, [, rss]) => sum + rss * 1024, 0);
          sampled_rss_peak = Math.max(sampled_rss_peak, bytes);
          if (bytes > protocol.sampled_group_rss_ceiling_bytes) kill('sampled process-group RSS ceiling exceeded');
        } catch (e) { kill(`memory supervision unavailable: ${e.message}`); }
      }, 500);
      p.stdout.on('data', b => { stdout += b; if (stdout.length + stderr.length > 4 * 1024 ** 2) kill('output bound exceeded'); });
      p.stderr.on('data', b => { stderr += b; if (stdout.length + stderr.length > 4 * 1024 ** 2) kill('output bound exceeded'); });
      p.on('error', e => { error = e.message; });
      p.on('close', (status, signal) => {
        clearInterval(memory); clearTimeout(timer);
        let result = null; try { if (status === 0) result = JSON.parse(stdout); } catch { error ??= 'invalid probe output'; }
        done({family, size, phase, events, sample, warmup: sample < 0, status, signal, error, stdout, stderr, result, sampled_rss_peak, peak_rss_bytes: timed ? Number(stderr.match(/(\d+)\s+maximum resident set size/)?.[1]) || null : null});
      });
    });
    record.rows.push(row); rows.push(row); save();
    console.log(family, size, phase, sample, row.result?.outcome ?? row.error ?? row.signal);
  }
  return rows.every(r => r.result?.outcome === 'NormalForm' && r.result.domain_verified && r.result.replay_verified && r.result.pending === 0 && r.result.commits === events);
}
for (const size of protocol.full_run_ladder) {
  if (!await cell('fixed', size, 'pipeline-framed', size)) {
    record.gates.push({size, outcome: 'GateFailed', remaining: protocol.full_run_ladder.filter(n => n > size)}); break;
  }
  record.gates.push({size, outcome: 'Passed'}); save();
}
for (const size of protocol.comparison_sizes) for (const family of ['fixed', 'symmetry']) for (const phase of ['compare-indexed', 'compare-permuted', 'observe']) await cell(family, size, phase, 1);
await cell('symmetry', 1000, 'pipeline-framed', 1000);
await cell('guarded', 10, 'pipeline-framed', 11);
await cell('oscillating', 10, 'pipeline-framed', 20);
record.gates.push({size: 1000000, outcome: 'FeasibilityOnly', reason: 'Outside the frozen 300k-atom / 100k-event / 512 MiB trace envelope. Assess projections from the measured 100k cell; do not label this as a completed job.'});
save();
