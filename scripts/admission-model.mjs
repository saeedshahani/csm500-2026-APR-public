// Independent finite-family oracle. No native matcher/validator output determines legality.
import assert from 'node:assert/strict';
export const schemas = ['delete-node','replace-node','delete-link','copy-marker','reverse','reverse-loop','delete-pair'];
export const sorted = xs => [...xs].sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));
const same = (a,b,message) => assert.deepEqual(sorted(a),sorted(b),message);
export function patch(g,d,f) { return [...g.filter(([id])=>!d.includes(id)),...f]; }
export function closed(g) { const ids=new Set(g.map(([id])=>id)); return g.every(([,fields])=>fields.every(t=>ids.has(t))); }
export function fresh(g,f) { return f.every(([id])=>!g.some(([old])=>old===id)); }
export function admits(g,d,f) {
  const kept=new Set(g.filter(([id])=>!d.includes(id)).map(([id])=>id));
  const added=new Set(f.map(([id])=>id));
  return g.every(([id,fields])=>d.includes(id)||fields.every(t=>!d.includes(t))) &&
    f.every(([,fields])=>fields.every(t=>kept.has(t)||added.has(t)));
}
export function abstractInputs() {
  const rows=[];
  function products(options,n,prefix=[],out=[]) {
    if(!n) out.push(prefix); else for(const x of options) products(options,n-1,[...prefix,x],out);
    return out;
  }
  for(let n=0;n<=3;n++) for(const fields of products([[],...Array.from({length:n},(_,i)=>[i])],n)) {
    const graph=fields.map((f,id)=>[id,f]);
    for(let mask=0;mask<2**n;mask++) {
      const del=Array.from({length:n},(_,i)=>i).filter(i=>mask&(1<<i));
      const fragments=[[],[[n,[]]],...Array.from({length:n+2},(_,t)=>[[n,[t]]]),[[n,[n+1]],[n+1,[n]]],[[n,[...Array.from({length:n},(_,i)=>i),n+1]],[n+1,[]]]];
      for(const f of fragments) rows.push({graph,delete:del,fresh:f});
    }
  }
  return rows;
}
export const countermodels = [
  {name:'missing-dangling-check',graph:[[0,[1]],[1,[]]],delete:[1],fresh:[],closed:true,fresh_ok:true,accepted:false,post_closed:false},
  {name:'identity-reuse-invalidates-converse',graph:[[0,[1]],[1,[]]],delete:[1],fresh:[[1,[]]],closed:true,fresh_ok:false,accepted:false,post_closed:true},
  {name:'missing-new-field-check',graph:[[0,[]]],delete:[],fresh:[[1,[2]]],closed:true,fresh_ok:true,accepted:false,post_closed:false},
  {name:'fresh-cycles-are-valid',graph:[],delete:[],fresh:[[0,[1]],[1,[0]]],closed:true,fresh_ok:true,accepted:true,post_closed:true},
  {name:'old-closure-is-required',graph:[[0,[1]]],delete:[],fresh:[],closed:false,fresh_ok:true,accepted:true,post_closed:false},
  {name:'closure-does-not-imply-unique-identities',graph:[],delete:[],fresh:[[0,[]],[0,[]]],closed:true,fresh_ok:true,accepted:true,post_closed:true},
];
export function checkAbstract(input,result,requirePremises=true) {
  const expected={closed:closed(input.graph),fresh:fresh(input.graph,input.fresh),accepted:admits(input.graph,input.delete,input.fresh),post_closed:closed(patch(input.graph,input.delete,input.fresh)),post:patch(input.graph,input.delete,input.fresh)};
  assert.deepEqual(result,expected);
  if(requirePremises) { assert.ok(expected.closed&&expected.fresh); assert.equal(expected.accepted,expected.post_closed); }
  return expected;
}
export function nativeInputs() {
  const hosts=[];
  for(let nodes=0;nodes<=3;nodes++) for(let mask=0;mask<2**(nodes*nodes);mask++) {
    const edges=[];
    for(let a=0;a<nodes;a++) for(let b=0;b<nodes;b++) if(mask&(1<<(a*nodes+b))) edges.push([a,b,0]);
    hosts.push({nodes,edges});
  }
  // Additional multigraph/label families, not counted as exhaustive simple hosts.
  hosts.push({nodes:2,edges:[[0,1,0],[0,1,0]]},{nodes:2,edges:[[0,1,0],[0,1,1],[1,0,0]]},{nodes:1,edges:[[0,0,0],[0,0,0],[0,0,1]]});
  return hosts.flatMap(h=>schemas.map(schema=>({...h,schema})));
}
export function nativeExpected(input,ids) {
  const {nodes:n,edges,schema}=input;
  const tuples=[];
  if(['delete-node','replace-node'].includes(schema)) for(let i=0;i<n;i++) tuples.push([i]);
  else if(['delete-link','copy-marker'].includes(schema)) for(let i=0;i<edges.length;i++) tuples.push([n+i]);
  else for(let i=0;i<edges.length;i++) {
    const [a,b,label]=edges[i];
    if(label!==0) continue;
    if(schema==='reverse-loop'&&a===b) tuples.push([a,n+i]);
    if(schema!=='reverse-loop'&&a!==b) tuples.push([a,b,n+i]);
  }
  return tuples.map(t=>{
    const c = schema==='copy-marker'?[]:schema==='reverse'||schema==='reverse-loop'?[t.at(-1)]:t;
    const allowed=edges.every(([a,b],i)=>c.includes(n+i)||(!c.includes(a)&&!c.includes(b)));
    const context=new Set(t);
    for(const i of t) if(i>=n) {context.add(edges[i-n][0]);context.add(edges[i-n][1]);}
    const additions=[];
    if(schema==='replace-node') additions.push({kind:{Constructor:'New'},fields:[]},{kind:{Record:{role:'ObjectLink',label:'new'}},fields:['fresh:0','fresh:0']});
    if(schema==='copy-marker') additions.push({kind:{Constructor:'Mark'},fields:[]});
    if(schema==='reverse'||schema==='reverse-loop') {
      const [a,b]=edges[t.at(-1)-n];
      additions.push({kind:{Record:{role:'ObjectLink',label:'reversed'}},fields:[String(ids[b]),String(ids[a])]});
    }
    return {targets:t.map(i=>ids[i]),allowed,consumed:c.map(i=>ids[i]),preserved:[...context].filter(i=>!c.includes(i)).map(i=>ids[i]),additions};
  });
}
export function checkNative(row) {
  const {input,before,data,cases}=row;
  assert.equal(data.length,input.nodes+input.edges.length);
  for(let i=0;i<data.length;i++) {
    const expected=i<input.nodes?{kind:{Constructor:'N'},fields:[]}:{kind:{Record:{role:'ObjectLink',label:input.edges[i-input.nodes][2]===0?'edge':'other'}},fields:input.edges[i-input.nodes].slice(0,2).map(j=>String(data[j]))};
    assert.deepEqual(before.atoms[data[i]],expected,'bridge constructed declared data');
  }
  const expected=nativeExpected(input,data);
  const tuples=expected.map(e=>e.targets);
  same(row.reference.bindings,tuples,'reference enumeration');
  same(row.indexed.bindings,tuples,'indexed enumeration');
  same(row.enabled.bindings,expected.filter(e=>e.allowed).map(e=>e.targets),'admitted enumeration');
  assert.equal(row.enabled.rejected,expected.filter(e=>!e.allowed).length);
  same(cases.map(c=>c.targets),tuples,'all raw candidates checked exactly once');
  for(const c of cases) {
    const e=expected.find(e=>JSON.stringify(e.targets)===JSON.stringify(c.targets));
    if(!e.allowed) {
      assert.equal(c.result.status,'Dangling');
      assert.deepEqual(c.result.after,before,'rejection atomicity including allocator');
      continue;
    }
    assert.equal(c.result.status,'Admitted');
    same(c.result.consumed,e.consumed);same(c.result.preserved,e.preserved);
    const next=BigInt(before.next_identity);
    const produced=e.additions.map((_,i)=>Number(next+BigInt(i)));
    same(c.result.produced,produced);
    const after=structuredClone(before);
    for(const id of e.consumed) delete after.atoms[id];
    e.additions.forEach((a,i)=>{after.atoms[produced[i]]={kind:a.kind,fields:a.fields.map(t=>t.startsWith('fresh:')?String(produced[Number(t.slice(6))]):t)};});
    after.next_identity=String(next+BigInt(produced.length));
    assert.deepEqual(c.result.after,after,'complete carrier and allocation agree; rule frame unchanged');
  }
  return {candidates:expected.length,admitted:expected.filter(e=>e.allowed).length,rejected:expected.filter(e=>!e.allowed).length};
}
export const boundaryInputs = [
  {nodes:1,edges:[],schema:'replace-node',tuples:[[0]],max_work:0},
  {nodes:1,edges:[],schema:'replace-node',tuples:[[0]],max_candidates:0},
  {nodes:1,edges:[],schema:'replace-node',tuples:[[0]],exhaust_identity:true},
  {nodes:2,edges:[[0,1,0]],schema:'reverse',tuples:[[0,0,2],[2,1,0]]},
];
export function checkBoundary(row,index) {
  assert.deepEqual(row.input,boundaryInputs[index]);
  if(index<3) {
    assert.equal(nativeExpected(row.input,row.data)[0].allowed,true);
    assert.equal(row.enabled.error,'ResourceExhaustion');
    if(index<2) for(const name of ['reference','indexed']) assert.equal(row[name].error,'ResourceExhaustion');
    else {same(row.reference.bindings,[[row.data[0]]]);same(row.indexed.bindings,[[row.data[0]]]);assert.equal(row.before.next_identity,'18446744073709551615');}
  }
  for(const c of row.cases) {assert.equal(c.result.status,index<3?'ResourceExhaustion':'InvalidBinding');assert.deepEqual(c.result.after,row.before);}
}
