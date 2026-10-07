// Verify retained measurement bytes and scope, not a claim of good performance.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {gunzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import {compareRustPaths} from './research-source-files.mjs';
const directory=resolve(process.argv[2]??'benchmarks/results/s6-loop-b');
const sha=b=>createHash('sha256').update(b).digest('hex');
const read=p=>readFileSync(resolve(directory,p));
const c=JSON.parse(read('collection.json')),protocol=JSON.parse(read('protocol.json'));
assert.deepEqual(c.protocol,protocol);
assert.equal(protocol.schema,'loop-b-diagnostic-v1');
assert.equal(protocol.samples,3);assert.equal(protocol.warmups,1);
assert.ok(c.rows.length>0,'The collection must not be empty');
const bundle=read('sources.json.gz');assert.equal(sha(bundle),c.source_bundle_sha256);
const files=JSON.parse(gunzipSync(bundle)).files;
const inputs=[];
for(const path of Object.keys(files)){
 if(path.startsWith('crates/engine/src/')&&path.endsWith('.rs'))inputs.push([path.slice('crates/engine/'.length),path]);
 if(path.startsWith('crates/graph-core/src/')&&path.endsWith('.rs'))inputs.push(['../graph-core/'+path.slice('crates/graph-core/'.length),path]);
}
for(const [local,path] of [['build.rs','crates/engine/build.rs'],['Cargo.toml','crates/engine/Cargo.toml'],['../graph-core/Cargo.toml','crates/graph-core/Cargo.toml'],['../../Cargo.toml','Cargo.toml'],['../../Cargo.lock','Cargo.lock'],['../../rust-toolchain.toml','rust-toolchain.toml']])inputs.push([local,path]);
const fingerprint=createHash('sha256');for(const [name,path] of inputs.sort(([a],[b])=>compareRustPaths(a,b))){assert.ok(files[path],path);fingerprint.update(name);fingerprint.update('\0');fingerprint.update(Buffer.from(files[path],'base64'));fingerprint.update('\0');}
const implementation='engine-0.1.0-source-sha256='+fingerprint.digest('hex');
const cells=new Map();
for(const row of c.rows){
 const key=JSON.stringify([row.build,row.family,row.size,row.phase,row.events]);
 if(!cells.has(key))cells.set(key,[]);cells.get(key).push(row);
 assert.equal(row.warmup,row.sample<0);
 if(row.result){
  assert.deepEqual(row.result,JSON.parse(row.stdout));assert.equal(row.status,0);
  if(row.build==='candidate')assert.equal(row.result.implementation,implementation);
  if(row.phase.startsWith('pipeline')&&row.result.outcome==='NormalForm'){
   assert.equal(row.result.replay_verified,true);assert.equal(row.result.domain_verified,true);assert.equal(row.result.pending,0);
   assert.equal(row.result.commits,row.size+(row.family==='guarded'?1:0));
  }
 }
}
for(const rows of cells.values())assert.deepEqual(rows.map(r=>r.sample),[-1,0,1,2]);
for(const size of [10,100,1000])for(const build of ['baseline','candidate'])for(const phase of ['clone','prepare','enabled-compact'])assert.ok(cells.has(JSON.stringify([build,'fixed',size,phase,1])),'Missing baseline/candidate cell');
for(const family of ['fixed','symmetry'])for(const size of protocol.comparison_sizes)for(const phase of ['compare-indexed','compare-permuted'])assert.ok(cells.has(JSON.stringify(['candidate',family,size,phase,1])),'Missing comparison cell');
for(const phase of ['pipeline-reference','pipeline-indexed'])assert.ok(cells.has(JSON.stringify(['candidate','fixed',10,phase,10])),'Missing initial full-run gate');
for(const gate of c.gates)if(gate.outcome==='GateFailed')for(const row of c.rows)assert.ok(!(row.phase===gate.phase&&row.size>gate.size),'A failed full-run gate cannot authorise larger sizes');
const compatibility=resolve(directory,'../s6-loop-b-compatibility');
const migration=JSON.parse(readFileSync(resolve(compatibility,'manifest.json')));
assert.deepEqual(migration.rows.map(r=>r.name),['normal','recurrence','reflective','exhausted','yielded']);
for(const row of migration.rows){
 const legacyBytes=readFileSync(resolve(compatibility,`${row.name}.legacy.json`));
 const migratedBytes=readFileSync(resolve(compatibility,`${row.name}.migrated.json`));
 assert.equal(sha(legacyBytes),row.legacy_sha256);assert.equal(sha(migratedBytes),row.migrated_sha256);
 assert.equal(sha(Buffer.from(files[row.source],'base64')),row.source_sha256);
 const legacy=JSON.parse(legacyBytes),migrated=JSON.parse(migratedBytes);
 assert.equal(legacy.settings.execution_profile,undefined);
 assert.equal(migrated.settings.execution_profile,'reference-v1');
 delete migrated.settings.execution_profile;assert.deepEqual(migrated,legacy);
}
console.log(`Verified ${cells.size} cells, ${c.rows.length} processes, source identity, full-run gate disclosures and five retained legacy migrations. Runtime replay is checked by the engine tests.`);
