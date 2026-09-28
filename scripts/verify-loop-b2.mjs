// Verify retained completion evidence, source identity and scope. Not a timing oracle.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
const directory = resolve(process.argv[2] ?? 'benchmarks/results/s6-loop-b2');
const read = p => readFileSync(resolve(directory, p));
const sha = b => createHash('sha256').update(b).digest('hex');
const c = JSON.parse(read('collection.json')), p = JSON.parse(read('protocol.json'));
assert.deepEqual(c.protocol, p);
assert.equal(p.schema, 'loop-b-completion-v1');
assert.equal(p.samples, 3); assert.equal(p.warmups, 1);
assert.deepEqual(p.full_run_ladder, [100, 1000, 10000, 100000]);
const bundle = read('sources.json.gz');
assert.equal(sha(bundle), c.source_bundle_sha256);
const files = JSON.parse(gunzipSync(bundle)).files, inputs = [];
for (const path of Object.keys(files)) {
  if (path.startsWith('crates/engine/src/') && path.endsWith('.rs')) inputs.push([path.slice('crates/engine/'.length), path]);
  if (path.startsWith('crates/graph-core/src/') && path.endsWith('.rs')) inputs.push(['../graph-core/' + path.slice('crates/graph-core/'.length), path]);
}
for (const [local, path] of [['build.rs','crates/engine/build.rs'],['Cargo.toml','crates/engine/Cargo.toml'],['../graph-core/Cargo.toml','crates/graph-core/Cargo.toml'],['../../Cargo.toml','Cargo.toml'],['../../Cargo.lock','Cargo.lock'],['../../rust-toolchain.toml','rust-toolchain.toml']]) inputs.push([local,path]);
const h = createHash('sha256');
for (const [name, path] of inputs.sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) {
  assert.ok(files[path], path); h.update(name); h.update('\0'); h.update(Buffer.from(files[path], 'base64')); h.update('\0');
}
const implementation = 'engine-0.1.0-source-sha256=' + h.digest('hex');
const cells = new Map();
for (const r of c.rows) {
  const key = JSON.stringify([r.family, r.size, r.phase, r.events]);
  if (!cells.has(key)) cells.set(key, []);
  cells.get(key).push(r);
  assert.equal(r.warmup, r.sample < 0);
  assert.equal(r.status, 0); assert.equal(r.error, null);
  assert.deepEqual(r.result, JSON.parse(r.stdout));
  assert.equal(r.result.implementation, implementation);
  assert.equal(r.result.size, r.size); assert.equal(r.result.phase, r.phase);
  assert.ok(r.peak_rss_bytes > 0 && r.sampled_rss_peak >= 0);
  assert.ok(r.result.phase_ns > 0);
  if (r.phase === 'pipeline-framed') {
    assert.equal(r.result.replay_verified, true);
    if (r.family === 'oscillating') {
      assert.equal(r.result.outcome, 'CycleDetected'); assert.equal(r.result.commits, 2);
    } else {
      assert.equal(r.result.outcome, 'NormalForm'); assert.equal(r.result.commits, r.events);
      assert.equal(r.result.domain_verified, true); assert.equal(r.result.pending, 0);
      assert.equal(r.result.processed, r.size); assert.ok(r.result.trace_bytes > 0);
      for (const region of ['run_ns', 'export_ns', 'import_ns']) assert.ok(r.result[region] > 0);
    }
  } else assert.equal(r.result.outcome, 'Completed');
}
for (const rows of cells.values()) assert.deepEqual(rows.map(r => r.sample), [-1, 0, 1, 2]);
for (const size of p.full_run_ladder) {
  assert.ok(cells.has(JSON.stringify(['fixed', size, 'pipeline-framed', size])));
  assert.ok(c.gates.some(g => g.size === size && g.outcome === 'Passed'));
}
for (const size of p.comparison_sizes) for (const family of ['fixed', 'symmetry']) for (const phase of ['compare-indexed', 'compare-permuted', 'observe']) assert.ok(cells.has(JSON.stringify([family, size, phase, 1])));
for (const [family,size,events] of [['symmetry',1000,1000],['guarded',10,11],['oscillating',10,20]]) assert.ok(cells.has(JSON.stringify([family,size,'pipeline-framed',events])));
assert.ok(c.gates.some(g => g.size === 1000000 && g.outcome === 'FeasibilityOnly'));
assert.ok(!c.rows.some(r => r.size === 1000000));
assert.equal(cells.size, 25); assert.equal(c.rows.length, 100);
console.log(`Verified ${cells.size} cells / ${c.rows.length} processes, four full-job gates, exact source identity and the unmeasured million-item boundary.`);
