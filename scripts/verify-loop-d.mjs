import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
const dir=resolve(process.argv[2]??'benchmarks/results/loop-d-02');
const c=JSON.parse(readFileSync(resolve(dir,'collection.json')));
const bundle=readFileSync(resolve(dir,'sources.json.gz'));
assert.equal(createHash('sha256').update(bundle).digest('hex'),c.source_bundle_sha256);
const sources=JSON.parse(gunzipSync(bundle));
const original=!Buffer.from(sources.files['crates/engine/src/stability.rs'],'base64').toString().includes('state.status == status');
assert.ok(sources.files['crates/engine/src/stability.rs']);
assert.ok(sources.files['docs/research/reflective-recovery.md'] || sources.files['benchmarks/PROTOCOL.md']);
assert.match(c.binary_sha256,/^[a-f0-9]{64}$/);
const expected=[...Array.from({length:8},(_,i)=>['guard',i+1,512]),['guard',7,128],['output',1,128],['gated',1,128],['ungated',1,128],['frame',2,128],['compose',2,128]];
assert.deepEqual(c.protocol.cells,expected);
assert.deepEqual(c.rows.map(r=>r.cell),expected);
for(const row of c.rows){
  assert.equal(row.status,0);assert.equal(row.error,null);assert.equal(row.signal,null);
  assert.deepEqual(JSON.parse(row.stdout),row.result);
  const r=row.result,[mode,n,bound]=row.cell;
  const verdict=mode==='ungated'?'refuted':mode==='guard'&&(bound===128||(original&&n===8))?'inconclusive':'certified';
  assert.equal(r.verdict,verdict);
  if(verdict==='inconclusive'){assert.match(r.reason,bound===128?/state bound/:/comparison budget exhausted/);continue;}
  assert.equal(r.legitimate.length,r.states);
  const successors=Array.from({length:r.states},()=>[]);
  for(const e of r.edges){assert.ok(Number.isInteger(e.from)&&e.from>=0&&e.from<r.states);assert.ok(Number.isInteger(e.to)&&e.to>=0&&e.to<r.states);successors[e.from].push(e.to);}
  if(verdict==='refuted'){
    assert.match(r.reason,/cycle/);
    assert.ok(r.witness_states.length>1);
    const witness=r.witness_states;
    for(let i=1;i<witness.length;i++)assert.ok(successors[witness[i-1]].includes(witness[i]));
    assert.ok(new Set(witness).size<witness.length);
    assert.ok(witness.every(i=>!r.legitimate[i]));
    continue;
  }
  assert.equal(r.ranks.length,r.states);
  for(let i=0;i<r.states;i++){
    if(r.legitimate[i]){assert.equal(r.ranks[i],null);assert.ok(successors[i].every(j=>r.legitimate[j]));}
    else {assert.ok(successors[i].length>0);assert.equal(r.ranks[i],1+Math.max(...successors[i].map(j=>r.ranks[j]??0)));}
  }
  assert.equal(r.recovery_bound,r.ranks[0]??0);
  if(mode==='guard'){assert.equal(r.states,1+2**n);assert.equal(r.edges.length,1+n*2**(n-1));assert.equal(r.recovery_bound,n+1);}
  else assert.equal(r.recovery_bound,{output:2,gated:1,frame:3,compose:5}[mode]);
  assert.ok(r.edges.some(e=>e.altered_rules.length>0));
}
console.log('Verified 14 Loop D cells, frozen sources, independent rank equations and cycle witness. This does not independently prove complete matching.');
