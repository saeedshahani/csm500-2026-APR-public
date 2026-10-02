// Independent weighted reachability and simple-path oracle, not a second native matcher.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {gunzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
const dir=process.argv[2]??'benchmarks/results/loop-f2-01';
const r=JSON.parse(readFileSync(`${dir}/collection.json`));
const bundle=readFileSync(`${dir}/sources.json.gz`);
assert.equal(createHash('sha256').update(bundle).digest('hex'),r.source_bundle_sha256);
const sources=JSON.parse(gunzipSync(bundle));
for(const p of ['crates/engine/src/meta_activity.rs','crates/engine/examples/support/inert_cases.rs','docs/research/reflective-recovery.md'])assert.ok(sources.files[p]);
const cases=[];
for(const name of ['spawn','credit-restoration'])for(let n=0;n<=4;n++)cases.push([name,n]);
for(const name of ['logger','connected','unfunded-spawn','output','gated-output','gated','ungated','literal-reader','wildcard-reader','new-reader'])cases.push([name,1]);
const cells=cases.flatMap(([name,n])=>['full','inert'].map(mode=>[name,n,16,mode]));
assert.equal(r.protocol.schema,'loop-f2-diagnostic-v1');assert.deepEqual(r.protocol.cells,cells);assert.deepEqual(r.rows.map(x=>x.cell),cells);
const edits=e=>e.entered.length+e.exited.length+e.altered.length>0;
function check(x,result,keep,cost){
  const n=x.states,es=x.edges.filter(keep),reach=Array.from({length:n},(_,a)=>Array.from({length:n},(_,b)=>a===b));
  for(const e of es)reach[e.from][e.to]=true;
  for(let k=0;k<n;k++)for(let a=0;a<n;a++)for(let b=0;b<n;b++)reach[a][b]||=reach[a][k]&&reach[k][b];
  const infinite=es.some(e=>cost(e)&&reach[e.to][e.from]);
  assert.equal(result.verdict,infinite?'refuted':'certified');
  if(infinite){
    const cycle=result.cycle_edges.map(i=>x.edges[i]);
    assert.ok(cycle.length&&cycle.every(keep)&&cycle.some(cost));
    for(let i=0;i<cycle.length;i++)assert.equal(cycle[i].to,cycle[(i+1)%cycle.length].from);
    assert.equal(result.initial_bound,null);assert.equal(result.maximum_bound,null);
  }else{
    function paths(at,seen){let best=0;for(const e of es)if(e.from===at&&!seen.has(e.to))best=Math.max(best,Number(cost(e))+paths(e.to,new Set([...seen,e.to])));return best;}
    const v=Array.from({length:n},(_,i)=>paths(i,new Set([i])));
    assert.deepEqual(result.potential,v);assert.equal(result.initial_bound,v[0]);assert.equal(result.maximum_bound,Math.max(...v));assert.deepEqual(result.cycle_edges,[]);
  }
}
const counts={certified:0,refuted:0,inconclusive:0,unsupported:0};
for(const row of r.rows){
  assert.equal(row.status,0,JSON.stringify(row));assert.equal(row.error,null);assert.deepEqual(JSON.parse(row.stdout),row.result);
  const x=row.result.result,[name,n,,mode]=row.cell;
  const unsupported=mode==='inert'&&name.endsWith('reader');
  const incomplete=mode==='full'||['connected','unfunded-spawn'].includes(name);
  if(unsupported||incomplete){const want=unsupported?'unsupported':'inconclusive';assert.equal(x.complete,false);assert.equal(x.verdict,want,name);counts[want]++;continue;}
  assert.equal(x.complete,true);assert.equal(x.schema,'meta-activity-analysis-v2');assert.equal(x.abstraction.label,'Log');assert.ok(x.abstraction.edge_erasure_count>0);
  const reachable=new Set([0]);for(let i=0;i<x.states;i++)for(const e of x.edges){assert.ok(e.from>=0&&e.to>=0&&e.from<x.states&&e.to<x.states);if(reachable.has(e.from))reachable.add(e.to);}
  assert.equal(reachable.size,x.states);
  check(x,x.total_edits,()=>true,edits);check(x,x.edit_burst,edits,edits);check(x,x.meta_invocations,()=>true,e=>e.meta_source);check(x,x.meta_burst,e=>e.meta_source,()=>true);
  if(x.legitimate)check(x,x.edits_without_object_progress,e=>!x.legitimate[e.from]&&!e.object_progress,edits);
  const expected=['spawn','credit-restoration'].includes(name)?n:name==='logger'?0:['output','gated-output'].includes(name)?1:null;
  assert.equal(x.total_edits.initial_bound,expected,name);counts[x.total_edits.verdict]++;
  if(['output','gated-output','gated','ungated'].includes(name))assert.equal(x.productive_modulation,['gated-output','gated'].includes(name)?'certified':'refuted');
  assert.ok(x.edges.some(e=>e.from===e.to&&!edits(e)));
}
console.log('Loop F2: forty full/reduced cells verified; weighted relations independently checked.',counts);
