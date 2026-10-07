import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {gzipSync,gunzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import os from 'node:os';
import {fit,proposal,predict,gate,score,compile,execute,inspect,promote,digest,native} from '../platform/learning.mjs';
import {corpus} from './learning-corpus.mjs';
import {sourceFiles} from './research-source-files.mjs';
import {assertSameExecution} from './trace-comparison.mjs';
const hash=b=>createHash('sha256').update(b).digest('hex');
const binary=resolve('target/release/cli');
export function methods(task,pilotLinear=false) {
  const trained=fit(task.training),zero=trained.scores.filter(s=>s.errors===0).map(s=>s.mask).sort((a,b)=>a-b);
  const shuffled=task.training.map((r,i)=>({...r,expected:task.training[(i+1)%task.training.length].expected}));
  const randomMask=pilotLinear?(task.rotation*37+task.corruption*17+11)%64:parseInt(digest({seed:42,task:task.id}).slice(0,2),16)%64;
  return {trained,models:{learned:trained.selected,consensus:trained.consensus,exact:proposal([0]),human:proposal([63]),search:proposal(zero.length?[zero[0]]:[]),random:proposal([randomMask]),shuffled:fit(shuffled).selected}};
}
export function campaign(pilotLinear=false) {
  return corpus().map(task=>{
    const {trained,models}=methods(task,pilotLinear);
    const results=Object.entries(models).map(([method,model])=>({method,model,gate:gate(model,task.validation),test:score(model,task.test)}));
    const table=new Map(task.training.map(r=>[r.input,r.expected]));
    const memory=rows=>rows.filter(r=>(table.get(r.input)??null)===r.expected).length;
    results.push({method:'memory',model:null,gate:{accepted:memory(task.validation)===task.validation.length},test:{total:task.test.length,correct:memory(task.test),positives:3,correct_positive:0,unresolved:3,wrong_positive:0,negatives:3,false_acceptance:0}});
    return {task,trained,results};
  });
}
export function summarise(cells) {
  const summary={};
  for(const cell of cells)for(const r of cell.results) {
    const s=summary[r.method]??={tasks:0,accepted:0,positive_correct:0,positives:0,false_acceptance:0,unresolved:0,accepted_test_failures:0,by_rotation:Array.from({length:12},()=>({correct:0,total:0}))};
    s.tasks++;s.accepted+=Number(r.gate.accepted);s.positive_correct+=r.test.correct_positive;s.positives+=r.test.positives;s.false_acceptance+=r.test.false_acceptance;s.unresolved+=r.test.unresolved;
    s.accepted_test_failures+=Number(r.gate.accepted&&r.test.correct!==r.test.total);
    s.by_rotation[cell.task.rotation].correct+=r.test.correct_positive;s.by_rotation[cell.task.rotation].total+=r.test.positives;
  }
  return summary;
}
function nativeCase(cell,model) {
  const rows=cell.task.test.map(r=>r.input),receipt=gate(model,cell.task.validation);
  const disabled=execute(binary,compile(model,rows));assert.equal(disabled.trace.events.length,0);
  const result=promote(binary,model,cell.task.validation,rows,receipt.approval_sha256);
  assert.equal(result.code,0,'Resource-inconclusive native cell');
  assert.deepEqual(result.observation.results.map(r=>r.output),cell.task.test.map(r=>r.expected));
  assert.equal(result.observation.commits,result.compiled.processors+3);
  return {task_id:cell.task.id,model,disabled:disabled.trace,result};
}
function checkNative(row,cell,live=false) {
  assert.equal(row.task_id,cell.task.id);assert.equal(row.result.approval_sha256,gate(row.model,cell.task.validation).approval_sha256);
  assert.deepEqual(row.result.compiled,compile(row.model,cell.task.test.map(r=>r.input),true));
  assert.equal(row.result.source_sha256,digest(row.result.compiled.source));
  assert.equal(row.disabled.events.length,0);
  assert.deepEqual(inspect(row.disabled,cell.task.test.map(r=>r.input)).results.map(r=>r.output),Array(6).fill(null));
  assert.deepEqual(inspect(row.result.validation_trace,cell.task.validation.map(r=>r.input)).results.map(r=>r.output),cell.task.validation.map(r=>r.expected));
  const view=inspect(row.result.trace,cell.task.test.map(r=>r.input));assert.deepEqual(view,row.result.observation);
  assert.equal(view.outcome,'NormalForm');assert.deepEqual(view.results.map(r=>r.output),cell.task.test.map(r=>r.expected));
  assert.equal(view.commits,row.result.compiled.processors+3);
  if(live)assertSameExecution(nativeCase(cell,row.model),row,[['disabled'],['result','validation_trace'],['result','trace']],verifyTrace);
}
function verifyTrace(trace){assert.equal(native(binary,['trace-check','-'],JSON.stringify(trace)).code,0);}
export function counterfactuals() {
  const clean=fit([{input:'jan',expected:'01'},{input:'feb',expected:'02'}]);
  const shuffled=fit([{input:'jan',expected:'02'},{input:'feb',expected:'01'}]);
  const contradictory=fit([{input:'jan',expected:'01'},{input:'jan',expected:'02'}]);
  const fullAmbiguity=fit([{input:'may',expected:'05'}]);
  return {
    shifted:{model:clean.selected,validation:gate(clean.selected,[{input:'mar',expected:'03'}]),test:score(clean.selected,[{input:' APR ',expected:'04'}]),human:score(proposal([63]),[{input:' APR ',expected:'04'}])},
    outside_grammar:score(proposal([63]),[{input:'fiscal=jan',expected:'01'}]),
    unseen_full_name:{model:fullAmbiguity.selected,consensus:fullAmbiguity.consensus,output:predict(fullAmbiguity.selected,'january'),consensus_output:predict(fullAmbiguity.consensus,'january')},
    shuffled_best_errors:shuffled.scores[0].errors,contradictory_best_errors:contradictory.scores[0].errors,contradictory_version_space:contradictory.consensus.masks,
    indistinguishable:{input:'03',calendar:'03',fiscal_start_april:'06',conclusion:'A token-only deterministic policy cannot satisfy both contextual meanings.'}
  };
}
function nativeChallenges() {
  const m=proposal([63]),validation=[{input:'jan',expected:'01'}],g=gate(m,validation);
  const shifted=promote(binary,fit(validation).selected,validation,[' APR '],gate(fit(validation).selected,validation).approval_sha256);
  const ambiguous=promote(binary,m,validation,['may'],g.approval_sha256);
  const compiled=compile(proposal([0]),['jan'],true);
  const corrupted={...compiled,source:compiled.source.replace('MonthNumber:01','MonthNumber:02')};
  const wrong=execute(binary,corrupted),stopped=execute(binary,compiled,0);
  return {shifted,ambiguous,wrong:{...wrong,observation:inspect(wrong.trace,['jan'])},stopped:{...stopped,observation:inspect(stopped.trace,['jan'])}};
}
function checkChallenges(c,live) {
  assert.equal(c.shifted.observation.outcome,'NormalForm');assert.equal(c.shifted.observation.results[0].output,null);
  assert.equal(c.ambiguous.observation.results[0].output,'05'); // Wrong if 'may' is a modal verb, not a calendar category.
  assert.equal(c.wrong.code,0);assert.equal(c.wrong.observation.results[0].output,'02');
  assert.equal(c.stopped.code,3);assert.equal(c.stopped.observation.commits,0);assert.equal(c.stopped.observation.results[0].output,null);
  for(const [name,row] of Object.entries(c)) {
    const inputs=name==='shifted'?[' APR ']:name==='ambiguous'?['may']:['jan'];
    assert.deepEqual(inspect(row.trace,inputs),row.observation);
  }
  if(live){
    const actual=nativeChallenges();
    const paths=Object.entries(c).flatMap(([name,row])=>[[name,'trace'],...(row.validation_trace?[[name,'validation_trace']]:[])]);
    assertSameExecution(actual,c,paths,verifyTrace);
  }
}
const [mode,dir,...flags]=process.argv.slice(2);
if(['collect','verify'].includes(mode)) {
  assert.ok(dir,'collect|verify DIRECTORY [--live]');
  assert.ok(!flags.includes('--pilot-linear')||mode==='verify','Legacy baseline is verification-only');
  const cells=campaign(flags.includes('--pilot-linear'));
  if(mode==='collect') {
    assert.ok(!existsSync(dir),'Collections are immutable');mkdirSync(dir,{recursive:true});
    const rows=[];const started=performance.now();
    for(const cell of cells.filter(c=>c.task.rotation===0)) {
      const accepted=new Map(cell.results.filter(r=>r.gate.accepted).map(r=>[digest(r.model),r.model]));
      for(const model of accepted.values())rows.push(nativeCase(cell,model));
    }
    const outcomes={cells,counterfactuals:counterfactuals(),native:rows,challenges:nativeChallenges()};
    const paths=['Cargo.lock','Cargo.toml','rust-toolchain.toml','platform/learning.mjs','scripts/learn-calendar.mjs','scripts/learning-corpus.mjs','scripts/learning-study.mjs','scripts/learning.test.mjs','scripts/research-source-files.mjs','scripts/trace-comparison.mjs','benchmarks/PROTOCOL.md',...sourceFiles(['crates/cli/src','crates/engine/src','crates/parser/src','crates/graph-core/src'])];
    const sources=Buffer.from(JSON.stringify(Object.fromEntries(paths.sort().map(p=>[p,readFileSync(p,'utf8')]))));
    const bytes=Buffer.from(JSON.stringify(outcomes));
    const report={schema:'learning-study-v1',date:new Date().toISOString(),revision:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),source_sha256:hash(sources),data_sha256:hash(bytes),binary_sha256:hash(readFileSync(binary)),model:'calendar-subsets-v1',provider:null,prompts:null,inference_cost:0,node:process.version,hardware:{platform:os.platform(),arch:os.arch(),cpu:os.cpus()[0].model},summary:summarise(cells),native_cases:rows.length,native_commits:rows.reduce((n,r)=>n+r.result.trace.events.length+r.result.validation_trace.events.length,0),native_wall_ms:performance.now()-started};
    writeFileSync(`${dir}/sources.json.gz`,gzipSync(sources));writeFileSync(`${dir}/outcomes.json.gz`,gzipSync(bytes));writeFileSync(`${dir}/report.json`,JSON.stringify(report,null,2)+'\n');
  }
  const report=JSON.parse(readFileSync(`${dir}/report.json`));assert.equal(report.schema,'learning-study-v1');
  const source=gunzipSync(readFileSync(`${dir}/sources.json.gz`)),bytes=gunzipSync(readFileSync(`${dir}/outcomes.json.gz`));
  assert.equal(hash(source),report.source_sha256);assert.equal(hash(bytes),report.data_sha256);
  const data=JSON.parse(bytes);assert.deepEqual(data.cells,cells);assert.deepEqual(data.counterfactuals,counterfactuals());assert.deepEqual(report.summary,summarise(cells));assert.equal(data.native.length,report.native_cases);
  checkChallenges(data.challenges,flags.includes('--live'));
  const expected=cells.filter(c=>c.task.rotation===0).flatMap(c=>[...new Map(c.results.filter(r=>r.gate.accepted).map(r=>[digest(r.model),r.model])).values()].map(model=>[c,model]));
  assert.equal(data.native.length,expected.length);
  data.native.forEach((row,i)=>{const [cell,model]=expected[i];assert.deepEqual(row.model,model);checkNative(row,cell,flags.includes('--live'));});
  assert.equal(report.native_commits,data.native.reduce((n,r)=>n+r.result.trace.events.length+r.result.validation_trace.events.length,0));
  console.log(JSON.stringify({verified:true,tasks:cells.length,native_cases:report.native_cases,native_commits:report.native_commits,summary:report.summary}));
}
