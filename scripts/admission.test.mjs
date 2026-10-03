import test from 'node:test';
import assert from 'node:assert/strict';
import {abstractInputs,countermodels,closed,fresh,admits,patch,nativeInputs,nativeExpected,checkNative} from './admission-model.mjs';
test('closed/fresh abstract domain exhaustively agrees with local closure predicate',()=>{
  const rows=abstractInputs();assert.ok(rows.length>4000);
  for(const r of rows) {assert.ok(closed(r.graph)&&fresh(r.graph,r.fresh));assert.equal(admits(r.graph,r.delete,r.fresh),closed(patch(r.graph,r.delete,r.fresh)));}
});
test('countermodels separate hypotheses, uniqueness and closure',()=>{
  for(const c of countermodels) {assert.equal(closed(c.graph),c.closed,c.name);assert.equal(fresh(c.graph,c.fresh),c.fresh_ok,c.name);assert.equal(admits(c.graph,c.delete,c.fresh),c.accepted,c.name);assert.equal(closed(patch(c.graph,c.delete,c.fresh)),c.post_closed,c.name);}
});
test('native corpus includes all 531 simple directed hosts and three labelled multigraphs',()=>{
  const q=nativeInputs();assert.equal(q.length,(531+3)*7);assert.equal(new Set(q.map(r=>JSON.stringify(r))).size,q.length);
  const loop=nativeExpected({nodes:1,edges:[[0,0,0]],schema:'reverse-loop'},[100,101]);assert.deepEqual(loop[0].targets,[100,101]);assert.equal(loop[0].allowed,true);
  assert.equal(nativeExpected({nodes:1,edges:[[0,0,0]],schema:'reverse'},[100,101]).length,0);
  const dangling=nativeExpected({nodes:2,edges:[[0,1,0],[0,1,0]],schema:'delete-pair'},[100,101,102,103]);assert.equal(dangling.length,2);assert.ok(dangling.every(r=>!r.allowed));
});
test('oracle detects omitted candidates, wrong allocation and illicit frame mutation',()=>{
  const input={nodes:1,edges:[],schema:'replace-node'};
  const before={atoms:{'1':{kind:'RuleHead',fields:[]},'10':{kind:{Constructor:'N'},fields:[]}},next_identity:'11'};
  const after={atoms:{'1':structuredClone(before.atoms['1']),'11':{kind:{Constructor:'New'},fields:[]},'12':{kind:{Record:{role:'ObjectLink',label:'new'}},fields:['11','11']}},next_identity:'13'};
  const row={input,before,data:[10],reference:{bindings:[[10]]},indexed:{bindings:[[10]]},enabled:{bindings:[[10]],rejected:0},cases:[{targets:[10],result:{status:'Admitted',consumed:[10],preserved:[],produced:[11,12],after}}]};
  assert.equal(checkNative(row).admitted,1);
  for(const mutate of [r=>{r.reference.bindings=[];},r=>{r.cases[0].result.after.next_identity='12';},r=>{r.cases[0].result.after.atoms['1'].kind='Nat';},r=>{r.cases[0].result.after.atoms['12'].fields=['10','11'];}]) {const bad=structuredClone(row);mutate(bad);assert.throws(()=>checkNative(bad));}
});
