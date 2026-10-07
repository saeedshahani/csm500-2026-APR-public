import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import {resolve} from 'node:path';
const directory = process.argv[2] ?? 'benchmarks/results/recovery-c01-rss';
const read = name => readFileSync(resolve(directory,name));
const sha = x => createHash('sha256').update(x).digest('hex');
const env = JSON.parse(read('environment.json'));
assert.equal(env.schema,'recovery-study-v1');
assert.equal(sha(read('sources.json.gz')),env.source_bundle_sha256);
assert.equal(sha(read('protocol.md')),env.protocol_sha256);
assert.match(env.binary_sha256,/^[a-f0-9]{64}$/);
assert.equal(env.samples,1); assert.equal(env.timeout_ms,30000);
const bundle = JSON.parse(gunzipSync(read('sources.json.gz')));
assert.equal(bundle.schema,'source-bundle-v1');
for (const p of ['Cargo.toml','Cargo.lock','crates/engine/examples/recovery_probe.rs',
  'crates/engine/examples/support/recovery.rs','crates/engine/tests/recovery.rs',
  'scripts/recovery-study.mjs','scripts/verify-recovery-study.mjs']) assert.ok(bundle.files[p],p);
const protocolPath = env.protocol_source_path ?? 'docs/recovery-study.md';
assert.ok(['docs/recovery-study.md','docs/research/reflective-recovery.md','benchmarks/PROTOCOL.md'].includes(protocolPath));
assert.equal(sha(Buffer.from(bundle.files[protocolPath],'base64')),env.protocol_sha256);
const rows = JSON.parse(read('raw.json'));
assert.deepEqual(rows.map(r=>r.args),[0,1,3,10,100,1000].flatMap(n=>['finite','modulating'].map(m=>[m,n,2*n+4])));
let completed=0, censored=0, missingRss=0, instrumentationFailures=0;
for (const r of rows) {
  assert.ok(Number.isFinite(r.elapsed_ms) && r.elapsed_ms>=0);
  if (r.censored) { assert.ok(['Timeout','OutputLimit','Cancelled'].includes(r.censored)); censored++; continue; }
  const instrumentationFailure = r.exit_code === 1 && r.peak_rss_bytes === null
    && r.stderr.includes('time: sysctl kern.clockrate: Operation not permitted');
  if (instrumentationFailure) instrumentationFailures++;
  else assert.equal(r.exit_code,0);
  assert.equal(r.signal,null);
  assert.deepEqual(JSON.parse(r.stdout),r.result);
  const [mode,n,bound]=r.args, s=r.result, modulating=mode==='modulating', base=modulating?310:190;
  assert.equal(s.schema,'recovery-probe-v1'); assert.equal(s.profile,'checked-family-v1');
  assert.equal(s.items,n); assert.equal(s.modulating,modulating); assert.equal(s.step_bound,bound);
  assert.equal(s.commits,modulating?bound:2*n); assert.equal(s.processed,n);
  assert.equal(s.meta_steps,modulating?n+4:n); assert.equal(s.pending,0);
  assert.equal(s.first_legitimate,n===0?0:2*n-1);
  assert.equal(s.outcome,modulating?'StepLimit':'FamilyNormalForm');
  assert.equal(s.initial_atoms,base+4*n); assert.equal(s.final_atoms,base+2*n);
  for (const k of ['phase_ns','initial_core_validation_ns','initial_membership_ns','enumerate_ns','admission_commit_ns'])
    assert.ok(Number.isFinite(s[k]) && s[k]>=0,k);
  const match = r.stderr.match(/(\d+)\s+maximum resident set size/);
  if (match) { assert.equal(r.peak_rss_bytes,Number(match[1])); assert.ok(r.peak_rss_bytes>0); }
  else { assert.equal(r.peak_rss_bytes,null); missingRss++; }
  if (!instrumentationFailure) completed++;
}
console.log(`Verified ${completed} completed, ${censored} censored and ${instrumentationFailures} instrumentation-failed diagnostic cells; ${missingRss} result rows lack RSS. Source bundle/protocol integrity checked. Binary hash is provenance, not a rebuild proof.`);
