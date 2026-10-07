import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {plans,validateReceipt} from './integrated-review.mjs';
import {compareWorkflowResults} from './workflow-study.mjs';
import {compareRustPaths,sourceFiles} from './research-source-files.mjs';
const hash=value=>createHash('sha256').update(value).digest('hex');

test('source verification matches Rust component ordering and the current executable',()=>{
  assert.ok(compareRustPaths('src/causal/provenance.rs','src/causal.rs')<0);
  const paths=sourceFiles(['crates/engine/src','crates/graph-core/src']).filter(p=>p.endsWith('.rs')).map(p=>[
    p.startsWith('crates/engine/')?p.slice('crates/engine/'.length):'../graph-core/'+p.slice('crates/graph-core/'.length),p]);
  for(const p of ['build.rs','Cargo.toml','../graph-core/Cargo.toml','../../Cargo.toml','../../Cargo.lock','../../rust-toolchain.toml'])paths.push([p,'crates/engine/'+p]);
  const h=createHash('sha256');
  for(const [name,path] of paths.sort(([a],[b])=>compareRustPaths(a,b)))h.update(name).update('\0').update(readFileSync(path)).update('\0');
  const run=spawnSync('target/release/cli',['export','examples/programs/arithmetic.graph','--max-steps','2'],{encoding:'utf8'});
  assert.equal(run.status,0,run.stderr);
  assert.equal(JSON.parse(run.stdout).implementation,`engine-0.1.0-source-sha256=${h.digest('hex')}`);
});
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
test('aggregate acceptance cannot omit an adopted study or proof family',()=>{
  const plan=plans();
  const commands=plan.research.map(c=>c.join(' ')).join('\n');
  for(const name of ['organization-rust-study.py','numerical-rust-study.py','action-learning-study.mjs',
    'snapshot-cost-study.mjs','selected-provenance-study.mjs','minimality-current',
    's7a-01/verify-measurements.mjs','s7a-02-scale'])assert.ok(commands.includes(name),name);
  assert.ok(plan.proof.some(c=>c.includes('scripts/check-proofs.mjs')));
  const proof=readFileSync('scripts/check-proofs.mjs','utf8');
  for(const name of ['Composition','Publication','Safety','Certificates','RepresentationCosts','SelectedProvenance','LocalCycles','Admission','MinimalityBounds','WorkflowContracts'])assert.ok(proof.includes(`${name}.lean`),name);
  const source=readFileSync('scripts/integrated-review.mjs','utf8');
  assert.ok(source.includes('proofs|models|tests'));
});

test('numerical reference import leaves the frozen evidence inventory unchanged',()=>{
  const result=spawnSync('python3',['-c',
    'import pathlib, hashlib, runpy; root=pathlib.Path("benchmarks/results/numerical-components-01"); snapshot=lambda: {str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob("*") if p.is_file()}; before=snapshot(); runpy.run_path("scripts/numerical-rust-study.py",run_name="reference_import_check"); assert snapshot()==before, "reference import changed frozen evidence"'],{encoding:'utf8'});
  assert.equal(result.status,0,result.stderr);
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
test('submission assessment preserves proposal boundaries and separate finalisation',()=>{
  const paper=readFileSync('docs/dissertation.md','utf8');
  for(const rq of ['RQ1','RQ2','RQ3','RQ4','RQ5'])assert.ok(paper.includes(`**${rq}:**`));
  for(const phrase of ['intrinsic meaning','global minimality','native refinement'])assert.ok(paper.includes(phrase));
  const task=readFileSync('docs/task-list.md','utf8');
  for(const id of ['S6-048','S6-072','S6-084'])assert.ok(task.includes(`- [ ] ${id} —`));
  assert.ok(readFileSync('docs/architecture.md','utf8').includes('closed and explicit-context open weak-head encoding'));
});
