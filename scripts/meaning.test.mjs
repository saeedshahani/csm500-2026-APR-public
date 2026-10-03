import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {SCHEMA,policy,rows,infer,fitContexts,review,compileContext,runContext,observeContext,promoteContext} from '../platform/meaning.mjs';
import {meaningTasks} from './meaning-corpus.mjs';
import {methods,unlabel,interventions,checkInterventions,assess} from './meaning-study.mjs';
const binary='target/release/cli';
const model=policy({schema:SCHEMA,contexts:[{name:'calendar',offset:0},{name:'fiscal',offset:3},{name:'noncalendar',offset:null}]});

test('context grammar rejects ambiguous declarations and executable payloads',()=>{
  for(const m of [null,{schema:SCHEMA,contexts:[]},{...model,source:'attack'}, {...model,contexts:[{name:'calendar',offset:12}]},{...model,contexts:[{name:'bad"',offset:0}]},{...model,contexts:[model.contexts[0],model.contexts[0]]}])assert.throws(()=>policy(m));
  for(const r of [[],[{token:'13',context:'calendar'}],[{token:'03',context:'calendar',expected:null}],Array(65).fill({token:'01',context:'calendar'})])assert.throws(()=>rows(r));
  assert.throws(()=>compileContext(model,[{token:'03',context:'calendar'}],{source:'bad'}));
  assert.deepEqual(infer(model,{token:'03',context:'unknown'}),{processed:false,value:null});
});
test('all independent oracle tasks separate training, validation and test categories',()=>{
  const tasks=meaningTasks();assert.equal(tasks.length,144);
  for(const t of tasks){const sets=[t.training,t.validation,t.test].map(rs=>new Set(rs.map(r=>r.token)));
    assert.deepEqual(sets.map(s=>s.size),[6,3,3]);assert.equal(new Set(sets.flatMap(s=>[...s])).size,12);
    const fitted=fitContexts(t.training);assert.ok(review(fitted.model,t.validation).accepted);
    for(const r of t.test)assert.deepEqual(infer(fitted.model,{token:r.token,context:r.context}),{processed:true,value:r.expected});
    assert.ok(fitted.candidates.every(c=>c.scores.length===13&&c.scores[0].errors===0));
  }
});
test('contradictory feedback and validation disagreement are not concealed',()=>{
  const fitted=fitContexts([{token:'01',context:'calendar',expected:'01'},{token:'01',context:'calendar',expected:'02'}]);
  assert.equal(fitted.candidates[0].scores[0].errors,1);
  const validation=[{token:'02',context:'calendar',expected:'02'}];
  assert.throws(()=>promoteContext(binary,model,validation,[{token:'03',context:'calendar'}],'wrong'),/approval/);
  assert.throws(()=>promoteContext(binary,model,[{...validation[0],expected:'03'}],[{token:'03',context:'calendar'}],'wrong'),/validation rejected/);
});
test('native contextual interpretation distinguishes the same token and explicit negative output',()=>{
  const input=['calendar','fiscal','noncalendar','unknown'].map(context=>({token:'03',context}));
  const compiled=compileContext(model,input),result=runContext(binary,compiled);
  assert.equal(result.code,0);assert.equal(result.observation.commits,6);
  assert.deepEqual(result.observation.results.map(({processed,value})=>({processed,value})),[{processed:true,value:'03'},{processed:true,value:'06'},{processed:true,value:null},{processed:false,value:null}]);
  const empty=runContext(binary,compileContext(model,input,{ruleFree:true}));assert.equal(empty.observation.outcome,'NormalForm');assert.ok(empty.observation.results.every(r=>!r.processed));
  assert.equal(assess(empty.observation,input.map(r=>({...r,expected:null}))).correct,0);
  const zero=runContext(binary,compiled,0);assert.equal(zero.code,3);assert.equal(zero.trace.events.length,0);
});
test('table data, not per-row precomputed answers, determines native output',()=>{
  const input=[{token:'03',context:'calendar'}],compiled=compileContext(model,input);
  const changed={...compiled,source:compiled.source.replace('meaning_0_2 = Record[ObjectLink]("value:0", token_2, value_2)','meaning_0_2 = Record[ObjectLink]("value:0", token_2, value_8)')};
  assert.notEqual(changed.source,compiled.source);
  const result=runContext(binary,changed);assert.equal(result.code,0);assert.equal(result.observation.results[0].value,'09');
  const corrupt=structuredClone(result.trace);Object.values(corrupt.initial.atoms).find(a=>a.kind.Constructor?.startsWith('Row:')).kind.Constructor='Row:[99,"03","calendar"]';
  assert.throws(()=>observeContext(corrupt,compiled));
});
test('counterfactual intervention checks discriminate context, frame and observer',()=>{
  const task=meaningTasks().find(t=>t.id==='r0-s3');
  const records=interventions(task).map(([name,c])=>({name,compiled:c,...runContext(binary,c,name==='zero-step'?0:512)}));
  assert.equal(checkInterventions(records,task).context_changes,3);
});
test('strong fixed baseline ties adaptive; shuffled and rule-free are controls, not gated deployments',()=>{
  const task=meaningTasks()[3],ms=methods(task).methods;
  assert.deepEqual(ms[0].model,ms[1].model);assert.equal(ms[3].review.accepted,false);
  assert.equal(ms[4].review.accepted,true);assert.equal(ms[4].options.ruleFree,true);
  assert.equal(unlabel(task.test).length,9);
});
test('local CLI requires review approval and verifies execution of validation rows',()=>{
  const request={training:[{token:'01',context:'fiscal',expected:'04'}],validation:[{token:'02',context:'fiscal',expected:'05'}]};
  const invoke=(command,value)=>spawnSync(process.execPath,['scripts/interpret-context.mjs',command,'-'],{input:JSON.stringify(value),encoding:'utf8'});
  const fitted=invoke('fit',request);assert.equal(fitted.status,0,fitted.stderr);const out=JSON.parse(fitted.stdout);assert.equal(out.review.accepted,true);
  const req={model:out.model,validation:request.validation,rows:[{token:'03',context:'fiscal'}]};assert.equal(invoke('run',req).status,2);
  req.approval_sha256=out.review.approval_sha256;const accepted=invoke('run',req);assert.equal(accepted.status,0,accepted.stderr);assert.equal(JSON.parse(accepted.stdout).observation.results[0].value,'06');
});

test('an undercovered review cannot certify an untested convention',()=>{
  const wrong=policy({...model,contexts:model.contexts.map(c=>({...c,offset:c.name==='fiscal'?0:c.offset}))});
  const validation=[{token:'02',context:'calendar',expected:'02'}],gate=review(wrong,validation);
  assert.equal(gate.accepted,true);
  const out=promoteContext(binary,wrong,validation,[{token:'03',context:'fiscal'}],gate.approval_sha256);
  assert.equal(out.observation.outcome,'NormalForm');assert.equal(out.observation.results[0].value,'03');
  assert.notEqual(out.observation.results[0].value,'06','Unobserved fiscal convention is not guaranteed by the gate');
});

test('all-negative policies complete deliberately and changed validation invalidates approval',()=>{
  const training=[{token:'01',context:'noncalendar',expected:null}],fitted=fitContexts(training);
  assert.equal(fitted.model.contexts[0].offset,null);
  const validation=[{token:'02',context:'noncalendar',expected:null}],gate=review(fitted.model,validation);
  const result=promoteContext(binary,fitted.model,validation,[{token:'03',context:'noncalendar'}],gate.approval_sha256);
  assert.deepEqual(result.observation.results.map(({processed,value})=>({processed,value})),[{processed:true,value:null}]);
  assert.throws(()=>promoteContext(binary,fitted.model,[{...validation[0],token:'04'}],[{token:'03',context:'noncalendar'}],gate.approval_sha256),/approval/);
});
