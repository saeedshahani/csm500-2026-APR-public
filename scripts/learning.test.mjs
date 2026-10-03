import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fit,proposal,predict,gate,compile,execute,inspect,promote,validateProposal,digest} from '../platform/learning.mjs';
import {corpus,corrupt} from './learning-corpus.mjs';
import {campaign,counterfactuals} from './learning-study.mjs';
const binary='target/release/cli';

test('strict proposal grammar forbids source injection, duplicates and oversized requests',()=>{
  for(const model of [null,{schema:'other',masks:[0]},{schema:'calendar-subsets-v1',masks:[-1]},{schema:'calendar-subsets-v1',masks:[0,0]},{schema:'calendar-subsets-v1',masks:[1,0]},{schema:'calendar-subsets-v1',masks:[64]},{schema:'calendar-subsets-v1',masks:[0],source:'rule malicious {}'}])assert.throws(()=>validateProposal(model));
  assert.throws(()=>fit([]));assert.throws(()=>fit([{input:'x',expected:'13'}]));assert.throws(()=>compile(proposal([0]),Array(129).fill('jan')));
  assert.throws(()=>predict(proposal([0]),'x'.repeat(129)));assert.throws(()=>predict(proposal([0]),'jan\n'));
  assert.equal(predict(proposal([63]),'month=janitor'),null);
});
test('training updates policy; labels and operation sequence are not ignored',()=>{
  assert.equal(fit([{input:'jan',expected:'01'}]).selected.masks[0],0);
  assert.equal(fit([{input:' MONTH=JAN. ',expected:'01'}]).selected.masks[0],15);
  const cf=counterfactuals();assert.equal(cf.shifted.validation.accepted,true);assert.equal(cf.shifted.test.unresolved,1);assert.equal(cf.shifted.human.correct_positive,1);
  assert.equal(cf.contradictory_best_errors,1);assert.deepEqual(cf.contradictory_version_space,[]);assert.equal(cf.shuffled_best_errors,2);
  assert.equal(cf.outside_grammar.unresolved,1);
});
test('all category rotations are disjoint; complete designed grammar and method ledger',()=>{
  const tasks=corpus();assert.equal(tasks.length,768);
  for(const t of tasks){const sets=[t.training,t.validation,t.test].map(rows=>new Set(rows.filter(r=>r.expected!==null).map(r=>r.expected)));
    assert.equal(new Set(sets.flatMap(s=>[...s])).size,12);
    for(const r of [...t.training,...t.validation,...t.test])if(r.expected!==null)assert.equal(predict(proposal([63]),r.input),r.expected);}
  const cells=campaign();assert.equal(cells.length,768);assert.ok(cells.every(c=>c.results.length===8));
  for(const c of cells){const best=Math.min(...c.trained.scores.map(s=>s.errors));assert.equal(best,0);assert.equal(c.trained.scores.length,64);}
});
test('version-space unanimous predictions agree with every surviving hypothesis',()=>{
  for(const t of corpus()){
    const trained=fit(t.training);const probes=Array.from({length:12},(_,m)=>corrupt(m,(t.corruption+17)%64));
    for(const input of probes){const output=predict(trained.consensus,input);if(output!==null)for(const mask of trained.consensus.masks)assert.equal(output,predict(proposal([mask]),input));}
  }
});
test('approval binds proposal and validation; empty-positive and wrong-label gates reject',()=>{
  const m=proposal([3]),v=[{input:' MAR ',expected:'03'}],g=gate(m,v);
  assert.equal(g.accepted,true);assert.equal(g.approval_sha256,digest({model:m,validation:v}));
  assert.equal(gate(m,[{input:' MAR ',expected:'04'}]).accepted,false);
  assert.equal(gate(m,[{input:'unknown',expected:null}]).accepted,false);
  assert.throws(()=>promote(binary,m,v,[' APR '],'wrong'),/approval digest/);
  assert.throws(()=>promote(binary,proposal([0]),v,[' APR '],g.approval_sha256),/gate rejected/);
});
test('native graph promotion preserves duplicates, payloads and raw data with actual revision edits',()=>{
  const rows=[' MONTH=JAN. ','month=feb','unknown',' MONTH=JAN. '],m=proposal([63]);
  const disabled=execute(binary,compile(m,rows));assert.equal(disabled.trace.events.length,0);
  const v=[{input:'MAR',expected:'03'}],g=gate(m,v);
  const result=promote(binary,m,v,rows,g.approval_sha256);
  assert.deepEqual(result.observation.results.map(r=>r.output),['01','02',null,'01']);assert.equal(result.observation.commits,5);
  const edits=result.trace.events.filter(e=>Object.values(e.fresh).some(a=>a.kind.Record?.role==='Revision'));
  assert.equal(edits.length,2);
  const before=result.trace.initial.atoms;
  for(const e of edits)assert.ok(e.footprint.consumed.some(id=>before[id]?.kind.Record?.role==='Revision'));
  const bad=structuredClone(result.trace);const row=Object.values(bad.initial.atoms).find(a=>a.kind.Constructor?.startsWith('Row:'));row.kind.Constructor='Row:[9,"bad"]';
  assert.throws(()=>inspect(bad,rows));
});
test('wrong native result can be valid; zero-step execution remains inconclusive',()=>{
  const rows=['jan'],compiled=compile(proposal([0]),rows,true);
  const bad={...compiled,source:compiled.source.replace('MonthNumber:01','MonthNumber:02')};
  const valid=execute(binary,bad);assert.equal(valid.code,0);assert.equal(inspect(valid.trace,rows).results[0].output,'02');
  const stopped=execute(binary,compiled,0);assert.equal(stopped.code,3);assert.equal(stopped.trace.events.length,0);assert.equal(inspect(stopped.trace,rows).results[0].output,null);
});
test('quoted user text is data, never executable source or namespace substitution',()=>{
  const rows=['jan','raw " }; rule attack {','p0_row'];
  const result=execute(binary,compile(proposal([0]),rows,true));
  assert.deepEqual(inspect(result.trace,rows).results.map(r=>r.output),['01',null,null]);
});
test('local CLI exposes review before execution and refuses missing approval',()=>{
  const request={training:[{input:' JAN ',expected:'01'}],validation:[{input:' FEB ',expected:'02'}]};
  const invoke=(command,value)=>spawnSync(process.execPath,['scripts/learn-calendar.mjs',command,'-'],{input:JSON.stringify(value),encoding:'utf8'});
  const fitted=invoke('fit',request);assert.equal(fitted.status,0,fitted.stderr);const result=JSON.parse(fitted.stdout);assert.equal(result.review.accepted,true);
  const run={model:result.selected,validation:request.validation,rows:[' MAR ']};assert.equal(invoke('run',run).status,2);
  run.approval_sha256=result.review.approval_sha256;const accepted=invoke('run',run);assert.equal(accepted.status,0,accepted.stderr);assert.equal(JSON.parse(accepted.stdout).observation.results[0].output,'03');
});
