// This verifies retained evidence, including censoring; it does not certify capacity.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import {compareRustPaths} from '../../../scripts/research-source-files.mjs';
const root=new URL('./',import.meta.url);
const read=p=>readFileSync(new URL(p,root));
const json=p=>JSON.parse(read(p));
const sha=b=>createHash('sha256').update(b).digest('hex');
const scale=json('scale/collection.json');
assert.equal(sha(read('scale/sources.json.gz')),scale.source_bundle_sha256);
assert.deepEqual(scale.protocol,json('scale/protocol.json'));
const files=JSON.parse(gunzipSync(read('scale/sources.json.gz'))).files;
const paths=Object.keys(files).filter(p=>/^crates\/(engine|graph-core)\/src\/.+\.rs$/.test(p)).map(p=>[p.startsWith('crates/engine/')?p.slice('crates/engine/'.length):'../graph-core/'+p.slice('crates/graph-core/'.length),p]);
for(const [name,path] of [['build.rs','crates/engine/build.rs'],['Cargo.toml','crates/engine/Cargo.toml'],['../graph-core/Cargo.toml','crates/graph-core/Cargo.toml'],['../../Cargo.toml','Cargo.toml'],['../../Cargo.lock','Cargo.lock'],['../../rust-toolchain.toml','rust-toolchain.toml']])paths.push([name,path]);
const identity=createHash('sha256');
for(const [name,path] of paths.sort(([a],[b])=>compareRustPaths(a,b)))identity.update(name).update('\0').update(Buffer.from(files[path],'base64')).update('\0');
const implementation='engine-0.1.0-source-sha256='+identity.digest('hex');
assert.equal(scale.rows.length,100);
const cells=new Map();let censored=0;
for(const r of scale.rows){
  const key=JSON.stringify([r.family,r.size,r.phase,r.events]);
  if(!cells.has(key))cells.set(key,[]);cells.get(key).push(r.sample);
  assert.equal(r.warmup,r.sample===-1);
  if(r.error){
    assert.equal(r.family,'fixed');assert.equal(r.size,100000);assert.equal(r.phase,'pipeline-framed');
    assert.equal(r.error,'sampled process-group RSS ceiling exceeded');
    assert.equal(r.status,null);assert.equal(r.signal,'SIGKILL');assert.equal(r.result,null);
    assert.ok(r.sampled_rss_peak>scale.protocol.sampled_group_rss_ceiling_bytes);censored++;
  }else{
    assert.equal(r.status,0);assert.equal(r.signal,null);
    assert.deepEqual(r.result,JSON.parse(r.stdout));assert.equal(r.result.implementation,implementation);
    assert.ok(r.peak_rss_bytes>0);
    if(r.phase==='pipeline-framed'){
      assert.equal(r.result.replay_verified,true);
      if(r.family==='oscillating'){assert.equal(r.result.outcome,'CycleDetected');assert.equal(r.result.commits,2);}
      else{assert.equal(r.result.outcome,'NormalForm');assert.equal(r.result.domain_verified,true);assert.equal(r.result.commits,r.events);assert.equal(r.result.pending,0);}
    }else assert.equal(r.result.outcome,'Completed');
  }
}
assert.equal(cells.size,25);for(const samples of cells.values())assert.deepEqual(samples,[-1,0,1,2]);
assert.equal(censored,4);
assert.deepEqual(scale.gates.slice(0,4),[...[100,1000,10000].map(size=>({size,outcome:'Passed'})),{size:100000,outcome:'GateFailed',remaining:[]}]);
const data=json('public-data/results.json');
assert.equal(sha(read('public-data/sources.json.gz')),data.source_bundle_sha256);
assert.equal(data.archive_sha256,'2972e6184d3ad7beaaa831d9fc2b059dc3ee29df69d1ec593c466a5cd8485d14');
assert.equal(data.rows,12330);assert.equal(data.samples.length,5);
assert.deepEqual(data.samples.map(s=>s.policy),['fixed','fixed','fixed','fixed','guarded']);
for(const [i,s] of data.samples.entries()){
  assert.equal(s.i,i);assert.equal(s.warmup,i===0);assert.equal(s.job.status,'completed');
  assert.equal(s.job.outcome,'NormalForm');assert.equal(s.job.engine_sha256,data.binary_sha256);
  assert.equal(s.job.commits,12330+(s.policy==='guarded'?1:0));assert.equal(s.oracle_verified,true);
  assert.equal(s.csv_sha256,data.expected_sha256);assert.ok(s.wall_ms<data.protocol.deadline_ms);
  assert.ok(s.peak_native_rss>0&&s.peak_native_rss<data.protocol.sampled_native_rss_ceiling);
}
const diagnostic=json('scale-diagnostic/s7a-scale-diagnostic.json');
assert.deepEqual(diagnostic.map(r=>r.name),['historical-rebuilt','current']);
for(const r of diagnostic){assert.equal(r.cause,'sampled process-group RSS ceiling exceeded');assert.equal(r.code,null);assert.equal(r.signal,'SIGKILL');assert.ok(r.peak>6*1024**3);}
console.log('Verified: 96 completed scale processes, 4 censored 100k processes, 5 complete public-data jobs; both diagnostic builds are censored. The 100k gate failed.');
