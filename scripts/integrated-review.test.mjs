import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {plans,validateReceipt} from './integrated-review.mjs';
import {compareWorkflowResults} from './workflow-study.mjs';
const hash=value=>createHash('sha256').update(value).digest('hex');
const commands=[['node','fixture.mjs']],sources={'fixture.mjs':hash('fixture')};
const fixture=()=>({schema:'integrated-review-v1',complete:true,commands,sources,source_sha256:hash(JSON.stringify(sources)),results:[{index:0,exit_code:0,signal:null,error:null,log:'0.log',sha256:hash('ok')}]});
test('integrated receipt requires complete, successful, unchanged evidence',()=>{
  assert.equal(validateReceipt(fixture(),commands,()=>Buffer.from('ok'),sources),true);
  for(const mutate of [r=>r.complete=false,r=>r.results=[],r=>r.commands=[],r=>r.results[0].exit_code=1,r=>r.results[0].signal='SIGTERM',r=>r.results[0].error='timeout',r=>r.results[0].log='../secret',r=>r.sources={},r=>r.source_sha256='false']) {
    const r=fixture();mutate(r);assert.throws(()=>validateReceipt(r,commands,()=>Buffer.from('ok'),sources));
  }
  assert.throws(()=>validateReceipt(fixture(),commands,()=>Buffer.from('tampered'),sources));
  assert.throws(()=>validateReceipt(fixture(),commands,()=>Buffer.from('ok'),{'fixture.mjs':hash('new')}));
});
test('integrated plan separates functional acceptance from hosted deployment and benchmarks',()=>{
  const p=plans();assert.deepEqual(Object.keys(p),['core','research','platform','web','proof']);
  for(const commands of Object.values(p))for(const command of commands){assert.ok(command.length>1);assert.ok(!command.includes('deploy'));}
  assert.ok(p.web.some(c=>c.includes('test:browser')));
  assert.ok(p.platform.some(c=>c.includes('test:emulator')));
  assert.ok(p.research.some(c=>c.includes('--live')));
  assert.ok(p.proof.some(c=>c.includes('--proof')));
});
test('workflow verification refuses misspelled and ambiguous compatibility modes',()=>{
  for(const flags of [['--lve'],['--native','--live']]) {
    const r=spawnSync(process.execPath,['scripts/workflow-study.mjs','verify','benchmarks/results/workflow-contracts-01',...flags],{encoding:'utf8'});
    assert.notEqual(r.status,0);assert.match(r.stderr,/unknown or incompatible verification option/);
  }
});
test('workflow compatibility excludes only a well-formed producer identity',()=>{
  const old={outcome:'Completed',trace:{implementation:`engine-0.1.0-source-sha256=${'a'.repeat(64)}`,events:[{source:'1'}],settings:{bound:2}}};
  const current=structuredClone(old);current.trace.implementation=`engine-0.1.0-source-sha256=${'b'.repeat(64)}`;
  assert.deepEqual(compareWorkflowResults(current,old),{recorded:old.trace.implementation,current:current.trace.implementation});
  for(const mutate of [r=>r.outcome='StepLimit',r=>r.trace.events[0].source='2',r=>r.trace.settings.bound=3,r=>r.trace.implementation='unknown',r=>r.extra=true]) {
    const changed=structuredClone(current);mutate(changed);assert.throws(()=>compareWorkflowResults(changed,old));
  }
});
test('integrated assessment preserves proposal boundaries and separate finalisation',()=>{
  const audit=readFileSync('docs/research/claim-audit.md','utf8');
  for(const rq of ['RQ1','RQ2','RQ3','RQ4','RQ5'])assert.ok(audit.includes(`| ${rq}:`));
  for(const loop of 'ABCDEFGHIJKLMN')assert.ok(audit.includes(`| ${loop} |`));
  for(const phrase of ['not independent-team replication','intrinsic meaning','Sprint 7','machine-checked native refinement'])assert.ok(audit.includes(phrase));
  const task=readFileSync('docs/task-list.md','utf8');
  for(const id of ['S6-048','S6-072','S6-084'])assert.ok(task.includes(`- [ ] ${id} —`));
  assert.ok(readFileSync('docs/architecture.md','utf8').includes('closed and explicit-context open weak-head encoding'));
});
