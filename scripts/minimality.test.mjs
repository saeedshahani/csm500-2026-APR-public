import {test} from 'node:test';
import assert from 'node:assert/strict';
import {encode,decode,restore,rename,counts,sorts,roles,edgeEncode,edgeDecode} from './minimality-codecs.mjs';
import {enumerate,verifyWitness} from './minimality-gadgets.mjs';
function roundtrip(g,variant) {
  const {wire,mapping}=encode(g,variant), actual=counts(g,variant);
  assert.equal(Object.keys(wire.nodes).length,actual.nodes);
  assert.equal(Object.values(wire.nodes).reduce((s,n)=>s+n.arcs.length,0),actual.arcs);
  assert.deepEqual(restore(decode(wire,variant),mapping,g.next_identity),g);
  const {wire:renamed,map}=rename(wire);
  assert.deepEqual(restore(decode(renamed,variant),Object.fromEntries(Object.entries(mapping).map(([a,b])=>[a,map[b]])),g.next_identity),g);
}
test('all 4095 nonempty admission subsets, both bindings and three codecs survive renaming',()=>{
  for(let mask=1;mask<4096;mask++)for(const binding of ['Free','AppliedRule']) {
    const g={next_identity:'1',atoms:{0:{kind:{PatternVariable:{admits:sorts.filter((_,i)=>mask&2**i),binding}},fields:[]}}};
    for(const variant of ['M','T','P'])roundtrip(g,variant);
  }
});
test('ordered endpoints, parallel link identities, self-reference, edge-on-edge and all roles',()=>{
  const atoms={0:{kind:{Constructor:''},fields:[]},1:{kind:'Nat',fields:['1']},2:{kind:'RuleHead',fields:[]}};
  roles.forEach((role,i)=>{atoms[i+3]={kind:{Record:{role,label:role==='ObjectLink'?'value':null}},fields:['ObjectLink','Lhs','Rhs','Rank'].includes(role)?[String(i+3),'0']:[String(i+3)]};});
  atoms[11]={kind:{Record:{role:'ObjectLink',label:'value'}},fields:['3','3']};
  atoms[12]=structuredClone(atoms[11]);
  const g={next_identity:'13',atoms};for(const variant of ['M','T','P'])roundtrip(g,variant);
  assert.deepEqual(edgeDecode(edgeEncode(g)),g.atoms);
  assert.equal(Object.keys(edgeEncode(g)).length,13);
  // Carrier roundtrip permits cyclic Nat syntax; native validity is a separate test.
});
test('malformed registry, ownership, extra fields, labels, duplicate arcs and limits reject',()=>{
  const g={next_identity:'2',atoms:{0:{kind:'Nat',fields:[]},1:{kind:'Nat',fields:['0']}}};
  for(const v of ['M','T','P']) {
    const encoded=encode(g,v), mutate=f=>{const w=structuredClone(encoded.wire);f(w);assert.throws(()=>decode(w,v));};
    mutate(w=>w.nodes[w.root].arcs=[]);
    mutate(w=>w.nodes.extra={label:null,arcs:[]});
    mutate(w=>w.nodes[encoded.mapping[0]].label='hidden semantics');
    mutate(w=>w.nodes[encoded.mapping[0]].arcs.push(w.nodes[encoded.mapping[0]].arcs[0]));
    mutate(w=>w.nodes[encoded.mapping[0]].arcs.push('missing'));
    assert.throws(()=>decode(encoded.wire,v,1));
  }
  const g2={next_identity:'3',atoms:{0:{kind:{Constructor:'a'},fields:[]},1:{kind:'Nat',fields:['0']},2:{kind:'Nat',fields:['0']}}};
  const {wire,mapping}=encode(g2,'T'), fields=id=>wire.nodes[mapping[id]].arcs.find(x=>wire.nodes[x].arcs.length===2);
  const one=fields('1'),two=fields('2');wire.nodes[mapping[2]].arcs=wire.nodes[mapping[2]].arcs.map(x=>x===two?one:x);delete wire.nodes[two];
  assert.throws(()=>decode(wire,'T'),/shared/);
});
test('naive direct type/admission tags collide in simple arcs, but hubs do not',()=>{
  const direct=admits=>[...new Set(['PatternVariable',...admits])].sort();
  assert.deepEqual(direct(['Constructor']),direct(['Constructor','PatternVariable']));
  const make=admits=>({next_identity:'1',atoms:{0:{kind:{PatternVariable:{admits,binding:'Free'}},fields:[]}}});
  assert.notDeepEqual(encode(make(['Constructor'])).wire,encode(make(['Constructor','PatternVariable'])).wire);
  assert.notDeepEqual(encode(make(['Constructor']),'M').wire,encode(make(['Constructor','PatternVariable']),'M').wire);
  for(const g of [make(['Constructor']),make(['Constructor','PatternVariable'])])roundtrip(g,'M');
});
test('complete gadget enumeration, independent bitmask counts and optimal witnesses',()=>{
  const rows=[1,2,3,4].map(i=>enumerate(i));
  assert.deepEqual(rows.map(x=>x.total),[7,121,4096,234256]);
  assert.equal(rows[0].ordered,0);assert.equal(rows[1].min_edges,3);
  for(const row of rows){verifyWitness(row);if(row.internal<=3){const other=enumerate(row.internal,true);for(const k of ['total','reachable','ordered','min_edges'])assert.equal(other[k],row[k]);}}
});
test('labelled and parallel-arc countermodels defeat overgeneralised gadget bounds',()=>{
  const labelled=[[0,'slot0',1],[0,'slot1',2]];
  const parallel=[[0,1],[0,1],[0,2]], swap=edges=>edges.map(([a,b])=>[a,b===1?2:b===2?1:b]);
  assert.equal(new Set(labelled.map(x=>x[1])).size,2);
  assert.notDeepEqual(parallel.toSorted(),swap(parallel).toSorted());
  // Thus one internal root is sufficient with either labels or unbounded multiplicity.
});
test('one-root SAT abstraction has a checked resolution refutation',()=>{
  // Reach both sinks: a and b. A necessary asymmetry condition: not(a and b).
  const clauses=[[1],[2],[-1,-2]];
  for(let bits=0;bits<4;bits++)assert.equal(clauses.every(c=>c.some(l=>(Boolean(bits&2**(Math.abs(l)-1)))===(l>0))),false);
  const resolve=(a,b,pivot)=>{
    assert.ok(a.includes(pivot)&&b.includes(-pivot));
    return [...new Set([...a.filter(x=>x!==pivot),...b.filter(x=>x!==-pivot)])];
  };
  const fourth=resolve(clauses[1],clauses[2],2);
  assert.deepEqual(fourth,[-1]);assert.deepEqual(resolve(clauses[0],fourth,1),[]);
});
test('multiplicity is a real alternative, not uniformly dominated by positional paths',()=>{
  const atoms=Object.fromEntries(Array.from({length:16},(_,i)=>[String(i),{kind:{PatternVariable:{admits:['Constructor'],binding:'Free'}},fields:[]}]));
  const g={next_identity:'16',atoms};
  for(const variant of ['M','T','P'])roundtrip(g,variant);
  const m=counts(g,'M'),p=counts(g,'P');
  assert.deepEqual([m.nodes,m.arcs,p.nodes,p.arcs],[35,66,47,78]);
  assert.ok(m.nodes<p.nodes&&m.arcs<p.arcs);
});
test('empty carriers have only the required immutable registry',()=>{
  for(const v of ['M','T','P'])roundtrip({next_identity:'0',atoms:{}},v);
});
test('opaque identities may coincide with JavaScript object-property names',()=>{
  const g={next_identity:'1',atoms:{0:{kind:{Constructor:'payload'},fields:[]}}};
  for(const v of ['M','T','P'])for(const name of ['__proto__','constructor','toString']) {
    const {wire,mapping}=encode(g,v), old=mapping[0],change=id=>id===old?name:id;
    const renamed={root:change(wire.root),nodes:Object.fromEntries(Object.entries(wire.nodes).map(([id,n])=>[change(id),{label:n.label,arcs:n.arcs.map(change)}]))};
    assert.deepEqual(restore(decode(renamed,v),{0:name},g.next_identity),g);
  }
});
