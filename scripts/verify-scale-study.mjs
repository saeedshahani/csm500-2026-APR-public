import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import {resolve} from 'node:path';
const dir=process.argv[2]??'benchmarks/results/s6-cycle02';
const read=name=>JSON.parse(readFileSync(resolve(dir,name)));
const hash=b=>createHash('sha256').update(b).digest('hex');
const environment=read('environment.json'),matrix=read('matrix.json'),raw=read('raw.json'),summary=read('summary.json');
assert.equal(environment.schema,'scale-study-v1');assert.equal(environment.repetitions,5);assert.equal(environment.warmups,1);
const bundle=readFileSync(resolve(dir,'sources.json.gz'));assert.equal(hash(bundle),environment.source_bundle_sha256);
const sources=JSON.parse(gunzipSync(bundle));assert.equal(sources.schema,'source-bundle-v1');
assert.equal(hash(readFileSync(resolve(dir,'protocol.md'))),environment.protocol_sha256);
assert.equal(hash(Buffer.from(sources.files['docs/scalability-study.md'],'base64')),environment.protocol_sha256);
assert.equal(raw.length,matrix.length*6);assert.equal(summary.length,matrix.length);
const key=a=>JSON.stringify(a);
assert.equal(new Set(matrix.map(key)).size,matrix.length);
for(const args of matrix){
 const rows=raw.filter(r=>key(r.args)===key(args));assert.deepEqual(rows.map(r=>r.repetition),[-1,0,1,2,3,4]);
 const [family,n,phase,events,seed]=args;
 for(const row of rows){
  assert.ok(Number.isFinite(row.elapsed_ms)&&row.elapsed_ms>=0);
  if(row.censored){assert.ok(['Timeout','MemoryCeiling'].includes(row.censored));assert.equal(row.result,null);continue;}
  assert.equal(row.exit_code,0);assert.equal(row.signal,null);assert.ok(row.peak_rss_bytes>0);
  const r=row.result;assert.deepEqual(JSON.parse(row.stdout),r);assert.equal(r.schema,'scale-probe-v1');
  assert.deepEqual([r.family,r.size,r.phase,r.event_bound,r.seed],args);assert.ok(r.phase_ns>=0);
  assert.ok(['Completed','NormalForm','StepLimit','ResourceExhaustion'].includes(r.outcome));
  if(r.outcome==='ResourceExhaustion') {assert.match(r.diagnostic,/limit/i);assert.equal(r.complete_count,undefined);}
  if(family==='fixed')assert.equal(r.atoms,2*n+13);
  if(['fixed','guarded','symmetry','star','degree'].includes(family)){
   assert.equal(r.domain_invariant,true);
   if(phase!=='baseline')assert.equal(r.pending+r.processed,n);
   if(phase.startsWith('run')||phase.startsWith('record'))assert.equal(r.processed,Math.max(0,r.commits-(family==='guarded'?1:0)));
   if(r.complete_count!==undefined&&family!=='guarded')assert.equal(r.complete_count,n);
  }
  if(phase==='inspect'){assert.equal(r.inspection_atoms,Math.min(r.atoms,250));assert.ok(r.inspection_bytes>0);}
  if(phase==='baseline')assert.equal(r.processed,Math.min(n,events));
  if(phase.startsWith('record')&&r.recorded_events!==undefined)assert.equal(r.recorded_events,r.commits);
  if(family==='fixed'&&n===100&&phase.startsWith('run')){assert.equal(r.outcome,'NormalForm');assert.equal(r.commits,100);}
  if(family==='guarded'&&n===10&&events===20){assert.equal(r.outcome,'NormalForm');assert.equal(r.commits,11);}
  if(['oscillating','restoration'].includes(family)&&r.outcome!=='ResourceExhaustion'){assert.equal(r.outcome,'StepLimit');assert.equal(r.commits,events);}
 }
 const s=summary.find(s=>key(s.args)===key(args));assert.equal(s.attempts,5);
 assert.deepEqual(s.outcomes,rows.filter(r=>r.repetition>=0).map(r=>r.censored??r.result.outcome));
}
for(const data of read('datasets.json').datasets)for(const subset of data.subsets){
 const rows=raw.filter(r=>r.args[0].endsWith('/'+subset.file));assert.equal(rows.length,6);
 for(const {result:r,censored} of rows){assert.equal(censored,null);assert.equal(r.outcome,'Completed');assert.equal(r.atoms,subset.counts.carrier_atoms);assert.equal(r.fields,subset.counts.carrier_fields);assert.equal(r.commits,0);}
}
const gate=read('million-gate.json');assert.equal(gate.interrupted,false);
const top=summary.find(r=>r.args[0]==='fixed'&&r.args[1]===100000&&r.args[2]==='enabled-compact');
const eligible=top.outcomes.length===5&&top.outcomes.every(s=>s==='Completed')&&top.elapsed_ms.max*10<15000&&top.peak_rss_bytes?.max*10<256*1024*1024;
assert.equal(gate.eligible,Boolean(eligible));
console.log(`Verified ${matrix.length} cells, ${raw.length} retained processes, source/protocol integrity, domain/count oracles and dataset import totals.`);
