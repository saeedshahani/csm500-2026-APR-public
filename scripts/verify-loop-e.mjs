// Independent finite transition table, not the engine's family helper.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {gunzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
const dir=process.argv[2]??'benchmarks/results/loop-e-01';
const r=JSON.parse(readFileSync(`${dir}/collection.json`));
const bundle=readFileSync(`${dir}/sources.json.gz`);
assert.equal(createHash('sha256').update(bundle).digest('hex'),r.source_bundle_sha256);
const sources=JSON.parse(gunzipSync(bundle));
assert.ok(sources.files['crates/engine/src/stability.rs']);
assert.ok(sources.files['examples/practical/repair-policy.graph']);
assert.equal(r.protocol.schema,'loop-e-diagnostic-v1');
assert.equal(r.rows.length,270);
const expectedCells=[];
for(let n=1;n<=3;n++)for(let code=0;code<6**n;code++)expectedCells.push(['analyse',n,code]);
for(const mode of ['missing','duplicate'])for(let code=0;code<6;code++)expectedCells.push([mode,1,code]);
assert.deepEqual(r.protocol.cells,expectedCells);
assert.deepEqual(r.rows.map(x=>x.cell),expectedCells);
assert.equal(new Set(r.rows.map(x=>JSON.stringify(x.cell))).size,270);
const transitions=[[3,2],[4,2],[5],[4],[5],[]];
function oracle(start){
  const seen=new Map(),queue=[start];let edges=0;
  seen.set(start.join(','),start);
  for(let i=0;i<queue.length;i++)for(let j=0;j<start.length;j++)for(const v of transitions[queue[i][j]]){
    const s=[...queue[i]];s[j]=v;edges++;
    if(!seen.has(s.join(','))){seen.set(s.join(','),s);queue.push(s);}
  }
  const ranks=new Map();
  function rank(s){
    const key=s.join(',');if(ranks.has(key))return ranks.get(key);
    if(s.every(x=>x%3===2))return 0;
    const children=[];
    for(let j=0;j<s.length;j++)for(const v of transitions[s[j]]){const t=[...s];t[j]=v;children.push(rank(t));}
    assert.ok(children.length);const answer=1+Math.max(...children);ranks.set(key,answer);return answer;
  }
  return {states:seen.size,edges,bound:rank(start)};
}
for(const row of r.rows){
  assert.equal(row.status,0,JSON.stringify(row));assert.equal(row.error,null);
  assert.deepEqual(JSON.parse(row.stdout),row.result);
  const x=row.result;assert.equal(x.verdict,'certified');assert.equal(x.agreement,'agreed');
  const [mode,n,code]=row.cell;
  if(mode==='analyse'){
    let q=code;const start=Array.from({length:n},()=>{const v=q%6;q=Math.floor(q/6);return v;});
    const expected=oracle(start);
    for(const field of ['states','edges','bound'])assert.equal(x[field],expected[field],`${row.cell} ${field}`);
    assert.ok(x.bound<=x.potential);assert.ok(x.elapsed_ns>=0);
    assert.deepEqual(x.outputs,[Array.from({length:n},(_,i)=>[[`${i}:done`,`${i}:Ticket`]])]);
  }else{
    assert.equal(x.bound,code<3?3:2);
    assert.equal(x.receipt.reset_label,'0:pending');
    assert.equal(x.receipt.removed.length,mode==='missing'?0:2);
    assert.equal(x.receipt.created.length,1);
    assert.ok(x.reconstruction_ns>=0&&x.verification_ns>=0);
  }
}
console.log('Loop E: 258 complete-domain analyses and 12 retry reconstructions verified against an independent transition table.');
