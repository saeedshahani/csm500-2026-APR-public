// Independent reachability checks the weighted criterion; native admission is trusted separately.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {gunzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
const dir=process.argv[2]??'benchmarks/results/loop-f-01';
const r=JSON.parse(readFileSync(`${dir}/collection.json`));
const bundle=readFileSync(`${dir}/sources.json.gz`);
assert.equal(createHash('sha256').update(bundle).digest('hex'),r.source_bundle_sha256);
const sources=JSON.parse(gunzipSync(bundle));
assert.ok(sources.files['crates/engine/src/meta_activity.rs']);
assert.ok(sources.files['examples/practical/credit-spawn.graph']);
const cells=[];
for(const mode of ['credit-restoration','spawn'])for(let n=0;n<=4;n++)cells.push([mode,n,128]);
for(let n=1;n<=4;n++)cells.push(['guard',n,128]);
for(const mode of ['object-loop','meta-noop','self','finite-then-object-loop','delete-head','restoration','gated','ungated','interleaved'])cells.push([mode,1,128]);
cells.push(['unfunded-spawn',0,8],['spawn',2,1]);
assert.equal(r.protocol.schema,'loop-f-diagnostic-v1');assert.deepEqual(r.protocol.cells,cells);assert.deepEqual(r.rows.map(x=>x.cell),cells);
const edits=e=>e.entered.length+e.exited.length+e.altered.length>0;
function check(report,result,keep,cost){
  const n=report.states,edges=report.edges;
  const reach=Array.from({length:n},(_,a)=>Array.from({length:n},(_,b)=>a===b));
  for(const e of edges.filter(keep))reach[e.from][e.to]=true;
  for(let k=0;k<n;k++)for(let a=0;a<n;a++)for(let b=0;b<n;b++)reach[a][b]||=reach[a][k]&&reach[k][b];
  const infinite=edges.some(e=>keep(e)&&cost(e)&&reach[e.to][e.from]);
  assert.equal(result.verdict,infinite?'refuted':'certified');
  if(infinite){
    const cycle=result.cycle_edges.map(i=>edges[i]);assert.ok(cycle.length&&cycle.every(keep)&&cycle.some(cost));
    for(let i=0;i<cycle.length;i++)assert.equal(cycle[i].to,cycle[(i+1)%cycle.length].from);
    assert.equal(result.initial_bound,null);assert.equal(result.maximum_bound,null);
  }else{
    const v=Array(n).fill(0);
    for(let k=0;k<n;k++)for(const e of edges.filter(keep))v[e.from]=Math.max(v[e.from],Number(cost(e))+v[e.to]);
    assert.deepEqual(result.potential,v);assert.equal(result.initial_bound,v[0]);assert.equal(result.maximum_bound,Math.max(...v));
    assert.deepEqual(result.cycle_edges,[]);
  }
}
for(const row of r.rows){
  assert.equal(row.status,0,JSON.stringify(row));assert.equal(row.error,null);assert.deepEqual(JSON.parse(row.stdout),row.result);
  const x=row.result.result,[mode,n,max]=row.cell;assert.ok(row.result.elapsed_ns>=0);
  if(mode==='unfunded-spawn'||max===1){assert.equal(x.complete,false);assert.equal(x.verdict,'inconclusive');continue;}
  assert.equal(x.complete,true);assert.ok(x.states>0);
  const reachable=new Set([0]);for(let i=0;i<x.states;i++)for(const e of x.edges){assert.ok(e.from<x.states&&e.to<x.states);if(reachable.has(e.from))reachable.add(e.to);}
  assert.equal(reachable.size,x.states);
  check(x,x.total_edits,()=>true,edits);check(x,x.edit_burst,edits,edits);check(x,x.meta_invocations,()=>true,e=>e.meta_source);
  check(x,x.meta_burst,e=>e.meta_source,()=>true);
  if(x.legitimate)check(x,x.edits_without_object_progress,e=>!x.legitimate[e.from]&&!e.object_progress,edits);
  const expected=['restoration','gated','ungated','interleaved'].includes(mode)?null:['credit-restoration','spawn'].includes(mode)?n:['object-loop','meta-noop'].includes(mode)?0:1;
  assert.equal(x.total_edits.initial_bound,expected,mode);
  if(mode==='meta-noop')assert.equal(x.meta_invocations.verdict,'refuted');
  if(mode==='interleaved'){assert.equal(x.edit_burst.maximum_bound,1);assert.equal(x.edits_without_object_progress.verdict,'refuted');}
  if(['guard','gated','ungated','interleaved'].includes(mode))assert.equal(x.productive_modulation,['guard','gated'].includes(mode)?'certified':'refuted');
  if(mode==='spawn'&&n)assert.ok(x.edges.some(e=>e.entered.length));
  if(mode==='delete-head')assert.ok(x.edges.some(e=>e.exited.length));
}
console.log('Loop F: 25 cells verified; weighted bounds/cycles checked independently. Incomplete native exploration remains inconclusive.');
