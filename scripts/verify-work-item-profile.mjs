import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { gunzipSync } from 'node:zlib';
import { resolve } from 'node:path';

const dir = process.argv[2] ?? 'benchmarks/results/s6-cycle01-reproducible';
const c = JSON.parse(readFileSync(resolve(dir, 'collection.json')));
const sha = b => createHash('sha256').update(b).digest('hex');
assert.equal(c.schema, 'work-item-profile-collection-v1');
const bundle = readFileSync(resolve(dir, 'sources.json.gz'));
assert.equal(sha(bundle), c.source_bundle_sha256);
const sources = JSON.parse(gunzipSync(bundle));
assert.equal(sources.schema, 'source-bundle-v1');
assert.equal(sha(Buffer.from(sources.files['crates/engine/examples/profile_work_items.rs'], 'base64')), c.probe_sha256);
assert.equal(sha(Buffer.from(sources.files['scripts/profile-work-items.mjs'], 'base64')), c.collector_sha256);
assert.equal(c.rows.length, 24);
assert.equal(new Set(c.rows.map(r => `${r.items}/${r.phase}`)).size, 24);
for (const row of c.rows) {
  assert.equal(row.exit_code, 0); assert.equal(row.error, null); assert.equal(row.signal, null);
  const r = row.result;
  assert.deepEqual(JSON.parse(row.stdout), r);
  assert.equal(r.items, row.items); assert.equal(r.phase, row.phase); assert.equal(r.atoms, 2 * r.items + 13);
  assert.equal(r.max_work, 1000000); assert.equal(r.max_candidates, 1000);
  assert.ok(Number.isFinite(r.wall_ns) && r.wall_ns >= 0);
  if (c.environment.platform === 'darwin') assert.ok(row.peak_rss_bytes > 0);
  if (['bindings', 'enabled'].includes(r.phase)) {
    if (r.items === 1000) {
      assert.equal(r.outcome, 'ResourceExhaustion'); assert.match(r.diagnostic, /SearchWork/);
      assert.equal(r.complete_count, undefined);
      assert.equal(r.attempted_candidates, r.phase === 'bindings' ? 249 : 165);
    } else { assert.equal(r.outcome, 'Completed'); assert.equal(r.complete_count, r.items); }
  } else if (['run', 'recorded_run'].includes(r.phase)) {
    assert.equal(r.outcome, r.items === 1000 ? 'ResourceLimit' : 'NormalForm');
    assert.equal(r.commits, r.items === 1000 ? 0 : r.items);
  } else assert.equal(r.outcome, 'Completed');
}
console.log('Verified 24 diagnostic cells, outcome/counter expectations and retained source-bundle hashes. No new timings collected.');
