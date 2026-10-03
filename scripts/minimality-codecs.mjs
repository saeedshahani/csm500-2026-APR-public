// Experimental structural encodings, not a production evaluator or wire format.
import assert from 'node:assert/strict';
export const roles = ['ObjectLink','Lhs','Rhs','Active','Rank','ModeObject','ModeMeta','Revision'];
export const sorts = ['Constructor','PatternVariable','RuleHead','Nat',...roles.map(Record=>({Record}))];
const eq = (a,b) => JSON.stringify(a) === JSON.stringify(b);
const sortOf = kind => typeof kind === 'string' ? kind : kind.Record ? {Record:kind.Record.role} : Object.keys(kind)[0];
const tagIndex = sort => sorts.findIndex(s=>eq(s,sort));
const arity = sort => typeof sort === 'object' ? (['ObjectLink','Lhs','Rhs','Rank'].includes(sort.Record)?2:1) : sort === 'Nat' ? -1 : 0;
export function encode(graph, variant='P') {
  if(variant==='M')return encodeMultiplicity(graph);
  assert.ok(['T','P'].includes(variant));
  const count = variant === 'T' ? 17 : 15;
  const nodes = {}, mapping = {};
  let next = 0;
  const add = (label=null) => { const id=`n${next++}`;nodes[id]={label,arcs:[]};return id; };
  const tags = Array.from({length:count},()=>add());
  for(let i=0;i<count-1;i++)nodes[tags[i]].arcs.push(tags[i+1]);
  for(const [id,a] of Object.entries(graph.atoms)) mapping[id]=add(a.kind.Constructor ?? a.kind.Record?.label ?? null);
  for(const [id,a] of Object.entries(graph.atoms)) {
    const node=nodes[mapping[id]], index=tagIndex(sortOf(a.kind));assert.ok(index>=0);
    node.arcs.push(tags[index+1]);
    if(a.kind.PatternVariable) {
      const hub=add();node.arcs.push(hub);nodes[hub].arcs.push(tags[14]);
      for(const sort of a.kind.PatternVariable.admits){const i=tagIndex(sort);assert.ok(i>=0);nodes[hub].arcs.push(tags[i+1]);}
      if(a.kind.PatternVariable.binding==='AppliedRule')node.arcs.push(tags[13]);
      else assert.equal(a.kind.PatternVariable.binding,'Free');
    }
    a.fields.forEach((to,slot)=>{
      assert.ok(slot<2 && mapping[to]);
      if(variant==='P' && slot===0)node.arcs.push(mapping[to]);
      else {const field=add();node.arcs.push(field);nodes[field].arcs.push(mapping[to]);if(variant==='T')nodes[field].arcs.push(tags[15+slot]);}
    });
  }
  return {wire:{root:tags[0],nodes},mapping};
}

// Identities are opaque keys. The only distinguished node is the supplied root.
// Complete ownership/shape checks reject unused nodes, extra arcs and shared gadgets.
export function decode(wire,variant='P',cap=100000) {
  if(variant==='M')return decodeMultiplicity(wire,cap);
  assert.ok(['T','P'].includes(variant));
  assert.deepEqual(Object.keys(wire).sort(),['nodes','root']);
  const {nodes}=wire, ids=Object.keys(nodes);assert.ok(ids.length<=cap);
  let arcs=0, labels=0;
  for(const node of Object.values(nodes)) {
    assert.deepEqual(Object.keys(node).sort(),['arcs','label']);
    assert.ok(node.label===null || typeof node.label==='string');
    assert.ok(Array.isArray(node.arcs));arcs+=node.arcs.length;labels+=node.label?.length??0;
    assert.ok(arcs<=4*cap && labels<=1024*cap);
    assert.equal(new Set(node.arcs).size,node.arcs.length,'simple arcs required');
    for(const id of node.arcs)assert.ok(Object.hasOwn(nodes,id),'missing endpoint');
  }
  const tags=[],count=variant==='T'?17:15;
  let current=wire.root;
  for(let i=0;i<count;i++) {
    assert.ok(Object.hasOwn(nodes,current) && !tags.includes(current));
    const node=nodes[current];assert.equal(node.label,null);
    assert.equal(node.arcs.length,i===count-1?0:1);
    tags.push(current);current=node.arcs[0];
  }
  const registry=new Set(tags), sortTags=tags.slice(1,13), data=new Map(), used=new Set(tags);
  for(const id of ids.filter(x=>!registry.has(x))) {
    // Admission hubs may also point to sort tags, but cannot be data nodes.
    if(nodes[id].arcs.includes(tags[14]))continue;
    const type=nodes[id].arcs.filter(x=>sortTags.includes(x));
    if(type.length){assert.equal(type.length,1);data.set(id,sortTags.indexOf(type[0]));used.add(id);}
  }
  const own = id => {assert.ok(!used.has(id),'gadget shared or confused with data');used.add(id);assert.equal(nodes[id].label,null);};
  const atoms=Object.create(null);
  for(const [id,index] of data) {
    const node=nodes[id], sort=sorts[index];
    let remaining=node.arcs.filter(x=>x!==sortTags[index]),kind;
    if(sort==='Constructor'){assert.equal(typeof node.label,'string');kind={Constructor:node.label};}
    else if(sort==='PatternVariable') {
      assert.equal(node.label,null);
      const hubs=remaining.filter(x=>!registry.has(x)&&nodes[x].arcs.includes(tags[14]));assert.equal(hubs.length,1);
      const hub=hubs[0];own(hub);
      const admitted=nodes[hub].arcs.filter(x=>x!==tags[14]);assert.ok(admitted.length>0 && admitted.every(x=>sortTags.includes(x)));
      kind={PatternVariable:{admits:admitted.map(x=>sorts[sortTags.indexOf(x)]).sort((a,b)=>tagIndex(a)-tagIndex(b)),binding:remaining.includes(tags[13])?'AppliedRule':'Free'}};
      remaining=remaining.filter(x=>x!==hub && x!==tags[13]);
    } else if(typeof sort==='object') {
      assert.ok(sort.Record==='ObjectLink'?typeof node.label==='string':node.label===null);
      kind={Record:{role:sort.Record,label:node.label}};
    } else {assert.equal(node.label,null);kind=sort;}
    const fields=[];
    for(const to of remaining) {
      if(variant==='P' && data.has(to)){assert.equal(fields[0],undefined);fields[0]=to;}
      else {
        assert.ok(!registry.has(to));own(to);
        const ends=nodes[to].arcs.filter(x=>data.has(x));assert.equal(ends.length,1);
        let slot=1;
        if(variant==='T'){const slots=nodes[to].arcs.filter(x=>x===tags[15]||x===tags[16]);assert.equal(slots.length,1);slot=slots[0]===tags[15]?0:1;assert.equal(nodes[to].arcs.length,2);}
        else assert.equal(nodes[to].arcs.length,1);
        assert.equal(fields[slot],undefined);fields[slot]=ends[0];
      }
    }
    assert.ok(fields.length===0 || fields.every(x=>typeof x==='string'));
    for(let i=0;i<fields.length;i++)assert.equal(typeof fields[i],'string');
    const expected=arity(sort);assert.ok(expected===-1?fields.length<=1:fields.length===expected);
    atoms[id]={kind,fields};
  }
  assert.equal(used.size,ids.length,'orphan or unrecognised node');
  return atoms;
}
export function restore(atoms,mapping,next_identity,offset=0n) {
  // This witness transports identities for comparison; the decoder never reads it.
  const inverse=Object.fromEntries(Object.entries(mapping).map(([a,b])=>[b,String(BigInt(a)+offset)]));
  assert.equal(Object.keys(inverse).length,Object.keys(atoms).length);
  return {next_identity:String(BigInt(next_identity)+offset),atoms:Object.fromEntries(Object.entries(atoms).map(([id,a])=>{
    assert.ok(Object.hasOwn(inverse,id));return [inverse[id],{kind:a.kind,fields:a.fields.map(x=>{assert.ok(Object.hasOwn(inverse,x));return inverse[x];})}];}))};
}
export function rename(wire) {
  const ids=Object.keys(wire.nodes), map=Object.fromEntries(ids.map((id,i)=>[id,`opaque-${ids.length-i}`]));
  return {wire:{root:map[wire.root],nodes:Object.fromEntries(ids.toReversed().map(id=>[map[id],{label:wire.nodes[id].label,arcs:wire.nodes[id].arcs.map(x=>map[x]).toReversed()}]))},map};
}
export function counts(graph,variant) {
  let f0=0,f1=0,v=0,a=0,b=0;
  for(const x of Object.values(graph.atoms)){f0+=Number(x.fields.length>0);f1+=Number(x.fields.length>1);if(x.kind.PatternVariable){v++;a+=x.kind.PatternVariable.admits.length;b+=Number(x.kind.PatternVariable.binding==='AppliedRule');}}
  const n=Object.keys(graph.atoms).length;
  if(variant==='M')return {nodes:19+n+f0+f1,arcs:18+n+Object.values(graph.atoms).filter(x=>x.kind.Record).length+v+a+3*(f0+f1),n,f0,f1,v,a,b};
  return {nodes:(variant==='T'?17:15)+n+v+(variant==='T'?f0+f1:f1),arcs:(variant==='T'?16:14)+n+2*v+a+b+(variant==='T'?3*(f0+f1):f0+2*f1),n,f0,f1,v,a,b};
}
export function edgeEncode(graph) {
  // E_G: addressable unary/binary typed edges; vertices retain their native attributes.
  // Endpoint references may refer to other edges, including memberships and controls.
  return Object.fromEntries(Object.entries(graph.atoms).map(([id,a])=>[id,a.kind.Record?
    {edge:a.kind.Record,endpoints:a.fields}:{vertex:a.kind,fields:a.fields}]));
}
export function edgeDecode(elements) {
  return Object.fromEntries(Object.entries(elements).map(([id,e])=>[id,e.edge?
    {kind:{Record:e.edge},fields:e.endpoints}:{kind:e.vertex,fields:e.fields}]));
}

// A complete multiset-arc realisation of the historical nineteen-node registry.
// Repeated type references retain multiplicity; binding tags identify variables.
const basic=['Constructor','PatternVariable','RuleHead','Nat','Record'];
const admissionIndex=s=>typeof s==='string'?basic.indexOf(s)+1:6+roles.indexOf(s.Record);
function encodeMultiplicity(graph) {
  const nodes={},mapping={};let next=0;
  const add=label=>{const id=`m${next++}`;nodes[id]={label:label??null,arcs:[]};return id;};
  const tags=Array.from({length:19},()=>add());for(let i=0;i<18;i++)nodes[tags[i]].arcs.push(tags[i+1]);
  for(const [id,a] of Object.entries(graph.atoms))mapping[id]=add(a.kind.Constructor??a.kind.Record?.label);
  for(const [id,a] of Object.entries(graph.atoms)) {
    const kind=typeof a.kind==='string'?a.kind:Object.keys(a.kind)[0], row=nodes[mapping[id]].arcs;
    row.push(tags[1+basic.indexOf(kind)]);
    if(a.kind.Record)row.push(tags[6+roles.indexOf(a.kind.Record.role)]);
    if(a.kind.PatternVariable){row.push(tags[a.kind.PatternVariable.binding==='Free'?16:17]);for(const s of a.kind.PatternVariable.admits)row.push(tags[admissionIndex(s)]);}
    a.fields.forEach((to,slot)=>{const o=add();row.push(o);nodes[o].arcs.push(tags[14+slot],mapping[to]);});
  }
  return {wire:{root:tags[0],nodes},mapping};
}
function decodeMultiplicity(wire,cap) {
  assert.deepEqual(Object.keys(wire).sort(),['nodes','root']);
  const {nodes}=wire,ids=Object.keys(nodes);assert.ok(ids.length<=cap);
  let arcs=0,labels=0;
  for(const n of Object.values(nodes)){assert.deepEqual(Object.keys(n).sort(),['arcs','label']);assert.ok(n.label===null||typeof n.label==='string');assert.ok(Array.isArray(n.arcs));arcs+=n.arcs.length;labels+=n.label?.length??0;assert.ok(arcs<=4*cap&&labels<=1024*cap);for(const id of n.arcs)assert.ok(Object.hasOwn(nodes,id));}
  const tags=[];let cur=wire.root;
  for(let i=0;i<19;i++){assert.ok(Object.hasOwn(nodes,cur)&&!tags.includes(cur));assert.equal(nodes[cur].label,null);assert.equal(nodes[cur].arcs.length,i===18?0:1);tags.push(cur);cur=nodes[cur].arcs[0];}
  const registry=new Set(tags),used=new Set(tags),data=new Set(ids.filter(id=>!registry.has(id)&&nodes[id].arcs.some(x=>tags.slice(1,6).includes(x)||x===tags[16]||x===tags[17]))),atoms=Object.create(null);
  for(const id of data)used.add(id);
  for(const id of data) {
    const node=nodes[id],references=node.arcs.filter(x=>registry.has(x)),fields=[];
    const take=tag=>{const at=references.indexOf(tag);assert.ok(at>=0);references.splice(at,1);};
    let kind;
    const binding=references.filter(x=>x===tags[16]||x===tags[17]);
    if(binding.length){
      assert.equal(binding.length,1);take(binding[0]);take(tags[2]);assert.equal(node.label,null);
      const indices=references.map(x=>tags.indexOf(x));assert.ok(indices.length>0&&indices.every(x=>(x>=1&&x<=4)||(x>=6&&x<=13)));assert.equal(new Set(indices).size,indices.length);
      const admits=indices.map(x=>x<=4?basic[x-1]:{Record:roles[x-6]}).sort((a,b)=>tagIndex(a)-tagIndex(b));
      kind={PatternVariable:{admits,binding:binding[0]===tags[16]?'Free':'AppliedRule'}};references.length=0;
    }else{
      const types=references.filter(x=>tags.slice(1,6).includes(x));assert.equal(types.length,1);take(types[0]);const k=basic[tags.indexOf(types[0])-1];
      if(k==='Constructor'){assert.equal(typeof node.label,'string');kind={Constructor:node.label};}
      else if(k==='Record'){assert.equal(references.length,1);const role=roles[tags.indexOf(references[0])-6];assert.ok(role);assert.ok(role==='ObjectLink'?typeof node.label==='string':node.label===null);kind={Record:{role,label:node.label}};references.length=0;}
      else {assert.ok(k==='RuleHead'||k==='Nat');assert.equal(node.label,null);kind=k;}
    }
    assert.equal(references.length,0);
    for(const field of node.arcs.filter(x=>!registry.has(x))){
      assert.ok(!used.has(field));used.add(field);const f=nodes[field];assert.equal(f.label,null);assert.equal(f.arcs.length,2);
      const slot=f.arcs.includes(tags[14])?0:f.arcs.includes(tags[15])?1:-1;assert.ok(slot>=0);const targets=f.arcs.filter(x=>data.has(x));assert.equal(targets.length,1);assert.equal(fields[slot],undefined);fields[slot]=targets[0];
    }
    for(let j=0;j<fields.length;j++)assert.equal(typeof fields[j],'string');
    const expected=arity(sortOf(kind));assert.ok(expected===-1?fields.length<=1:fields.length===expected);atoms[id]={kind,fields};
  }
  assert.equal(used.size,ids.length);return atoms;
}
