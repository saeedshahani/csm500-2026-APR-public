// Verify the frozen evidence ledger; does not substitute for recollection/replay.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {gunzipSync} from 'node:zlib';
import {hash} from '../platform/store.mjs';
const directory=process.argv[2]??'benchmarks/results/s6-loop-c';
const read=name=>JSON.parse(fs.readFileSync(path.join(directory,name)));
const result=read('results.json'),recovery=read('recovery.json'),alternatives=read('alternatives.json');
assert.equal(result.schema,'loop-c-platform-v1');assert.equal(result.rows,12330);
assert.equal(result.archive_sha256,'2972e6184d3ad7beaaa831d9fc2b059dc3ee29df69d1ec593c466a5cd8485d14');
assert.equal(result.samples.length,5);
const bundle=fs.readFileSync(path.join(directory,'sources.json.gz'));
assert.equal(hash(bundle),result.source_bundle_sha256);
const sources=JSON.parse(gunzipSync(bundle));
for(const file of ['crates/engine/src/one_shot.rs','crates/engine/src/screened.rs','platform/months.mjs','scripts/collect-loop-c.mjs'])assert.ok(sources.files[file]);
assert.equal(result.samples.filter(s=>s.warmup).length,1);
const fixed=result.samples.filter(s=>!s.warmup&&s.policy==='fixed');assert.equal(fixed.length,3);
for(const s of result.samples){
 assert.equal(s.job.status,'completed');assert.equal(s.job.outcome,'NormalForm');
 assert.equal(s.job.commits,12330+(s.policy==='guarded'?1:0));
 assert.equal(s.job.engine_sha256,result.binary_sha256);assert.equal(s.job.settings.execution_profile,'joined-v4');assert.equal(s.job.settings.detect_cycles,true);
 assert.equal(s.oracle_verified,true);assert.equal(s.csv_sha256,result.expected_sha256);
 assert.ok(s.wall_ms<result.protocol.deadline_ms);assert.ok(s.peak_native_rss>0&&s.peak_native_rss<result.protocol.sampled_native_rss_ceiling);
 assert.ok(s.trace_bytes<128*1024*1024);assert.equal(s.summary.altered_rule_identities.length,s.policy==='guarded'?2:0);
 assert.equal(s.summary.initial.atoms,s.summary.final.atoms);
}
assert.equal(new Set(fixed.map(s=>s.job.trace_sha256)).size,1);
assert.equal(recovery.binary_sha256,result.binary_sha256);assert.equal(recovery.csv_sha256,result.csv_sha256);
assert.equal(recovery.paused.status,'paused');assert.equal(recovery.paused.commits,5000);assert.ok(recovery.paused.checkpoint_sha256);
assert.equal(recovery.resumed.trace_sha256,fixed[0].job.trace_sha256);
assert.equal(recovery.cancelled.status,'cancelled');assert.equal(recovery.cancelled.trace_sha256,null);
assert.equal(recovery.resource_limited.outcome,'ResourceLimit');
assert.equal(alternatives.samples.length,9);assert.equal(alternatives.binary_sha256,result.binary_sha256);
for(const s of alternatives.samples){if(s.profile==='joined-v4'){assert.equal(s.code,0);assert.equal(s.commits,s.n);assert.equal(s.oracle_verified,true);}else if(s.n===1000)assert.equal(s.cause,'timed_out');else assert.equal(s.oracle_verified,true);}
const failed=read('failed-screening.json');
assert.equal(hash(fs.readFileSync(path.join(directory,'failed-screening-sources.json.gz'))),failed.source_bundle_sha256);
assert.equal(failed.samples.at(-1).job.outcome,'ResourceLimit');assert.equal(failed.samples.at(-1).job.commits,1);
const pilot=read('failed-pilot.json');assert.equal(pilot.samples.at(-1).job.status,'timed_out');
const components=read('components.json');assert.equal(components.binary_sha256,result.binary_sha256);
assert.equal(hash(fs.readFileSync('scripts/measure-loop-c-components.mjs')),components.script_sha256);
for(const sample of components.samples){assert.equal(sample.oracle_verified,true);assert.equal(sample.replay_verified,true);assert.equal(sample.trace_sha256,result.samples.find(s=>s.policy===sample.policy).job.trace_sha256);}
const browser=read('browser.json');assert.equal(browser.total,12330);assert.equal(browser.rows_rendered,25);assert.equal(browser.csv_sha256,result.expected_sha256);
assert.equal(hash(fs.readFileSync('scripts/accept-loop-c-browser.mjs')),browser.script_sha256);
assert.equal(browser.navigation_ms.length,3);const median=a=>[...a].sort((a,b)=>a-b)[Math.floor(a.length/2)];
assert.ok(median(browser.navigation_ms)<2000);assert.equal(browser.narrow_overflow,false);
console.log(JSON.stringify({rows:result.rows,measured_fixed_jobs:fixed.length,median_ms:median(fixed.map(s=>s.wall_ms)),median_peak_native_rss:median(fixed.map(s=>s.peak_native_rss)),recovery_trace_equal:true,browser_median_ms:median(browser.navigation_ms)}));
