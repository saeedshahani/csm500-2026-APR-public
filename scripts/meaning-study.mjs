import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {gzipSync,gunzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {SCHEMA,policy,fitContexts,review,compileContext,runContext,observeContext} from '../platform/meaning.mjs';
import {digest,native} from '../platform/learning.mjs';
import {meaningTasks,contexts} from './meaning-corpus.mjs';
import {sourceFiles} from './research-source-files.mjs';
import {assertSameExecution} from './trace-comparison.mjs';
const sha=x=>createHash('sha256').update(x).digest('hex');
const binary=resolve('target/release/cli');
export const unlabel=rows=>rows.map(({token,context})=>({token,context}));
export function methods(task) {
  const fitted=fitContexts(task.training);
  const fixed=policy({schema:SCHEMA,contexts:[{name:'calendar',offset:0},{name:'fiscal',offset:task.start},{name:'noncalendar',offset:null}]});
  const blind=policy({schema:SCHEMA,contexts:contexts.map(name=>({name,offset:0}))});
  const shuffled=fitContexts(task.training.map(r=>({...r,context:contexts[(contexts.indexOf(r.context)+1)%3]})));
  return {fitted,shuffled,methods:[['adaptive',fitted.model,{}],['fixed',fixed,{}],['blind',blind,{}],['shuffled',shuffled.model,{}],['rule-free',fixed,{ruleFree:true}]].map(([name,model,options])=>({name,model,options,review:review(model,task.validation)}))};
}
export function assess(observation,expected) {
  assert.equal(observation.results.length,expected.length);
  let correct=0,processed=0,negative=0,unresolved=0;
  observation.results.forEach((r,i)=>{assert.equal(r.token,expected[i].token);assert.equal(r.context,expected[i].context);if(r.processed){processed++;if(r.value===expected[i].expected)correct++;if(r.value===null)negative++;}else unresolved++;});
  return {total:expected.length,correct,processed,negative,unresolved,normal:observation.outcome==='NormalForm',complete:processed===expected.length};
}
function run(compiled,maxSteps=512){return {compiled,...runContext(binary,compiled,maxSteps)};}
function checkRun(record,live=false) {
  assert.equal(record.code,record.observation.outcome==='NormalForm'?0:3);
  assert.deepEqual(observeContext(record.trace,record.compiled),record.observation);
  if(live)assertSameExecution(run(record.compiled,record.code===3?0:512),record,[['trace']],trace=>assert.equal(native(binary,['trace-check','-'],JSON.stringify(trace)).code,0));
}
export function interventions(task) {
  const model=methods(task).fitted.model,input=unlabel(task.test);
  const changed=policy({...model,contexts:model.contexts.map(c=>({...c,offset:c.offset===null?null:(c.offset+1)%12}))});
  const switched=input.map(r=>({...r,context:r.context==='calendar'?'fiscal':r.context}));
  return [
    ['baseline',compileContext(model,input)],['context-switch',compileContext(model,switched)],
    ['declaration-shift',compileContext(changed,input)],['context-removed',compileContext(model,input,{dropContext:true})],
    ['declarations-removed',compileContext(model,input,{dropDeclarations:true})],['inert',compileContext(model,input,{inert:true})],
    ['renamed',compileContext(model,input,{renamed:true})],['duplicate',compileContext(model,[...input,input[0]])],
    ['unknown-context',compileContext(model,[...input,{token:input[0].token,context:'unknown'}])],
    ['zero-step',compileContext(model,input)]
  ];
}
export function checkInterventions(records,task) {
  for(const record of records)assert.equal(record.code,record.name==='zero-step'?3:0);
  const cells=new Map(records.map(r=>[r.name,r])),base=cells.get('baseline').observation.results;
  for(const name of ['inert','renamed'])assert.deepEqual(cells.get(name).observation.results,base);
  for(const name of ['context-removed','declarations-removed']){assert.equal(cells.get(name).observation.outcome,'NormalForm');assert.ok(cells.get(name).observation.results.every(r=>!r.processed));}
  const switched=cells.get('context-switch').observation.results;
  assert.equal(switched.filter((r,i)=>r.value!==base[i].value).length,task.start===0?0:3);
  const shifted=cells.get('declaration-shift').observation.results;
  assert.equal(shifted.filter((r,i)=>r.value!==base[i].value).length,6);
  // Counts and normality stay unchanged despite six incorrect interpretations.
  for(const k of ['commits','outcome'])assert.equal(cells.get('declaration-shift').observation[k],cells.get('baseline').observation[k]);
  assert.equal(Object.keys(cells.get('declaration-shift').trace.initial.atoms).length,Object.keys(cells.get('baseline').trace.initial.atoms).length);
  const dup=cells.get('duplicate').observation.results;assert.deepEqual(dup.slice(0,9),base);assert.deepEqual({...dup[9],row:0},base[0]);
  assert.deepEqual(cells.get('unknown-context').observation.results.slice(0,9),base);assert.equal(cells.get('unknown-context').observation.results[9].processed,false);
  assert.equal(cells.get('zero-step').code,3);assert.equal(cells.get('zero-step').observation.commits,0);
  const compiled=structuredClone(cells.get('baseline').compiled),values=compiled.lexicon.values;
  compiled.lexicon.values=[...values.slice(1,12),values[0],values[12]];
  const alternate=observeContext(cells.get('baseline').trace,compiled);
  assert.equal(alternate.results.filter((r,i)=>r.value!==base[i].value).length,6);
  assert.throws(()=>observeContext(cells.get('renamed').trace,cells.get('baseline').compiled),'Untransported observer must reject unknown symbols');
  return {observer_shift:alternate,context_changes:task.start===0?0:3,declaration_changes:6};
}
function summary(data) {
  const result={};for(const cell of data.tasks)for(const method of cell.methods) {
    const record=data.runs[method.run],metrics=assess(record.observation,cell.task.test);
    const s=result[method.name]??={tasks:0,rows:0,correct:0,processed:0,unresolved:0,validation_rejected:0,by_start:Array.from({length:12},()=>({correct:0,total:0}))};
    s.tasks++;s.rows+=metrics.total;s.correct+=metrics.correct;s.processed+=metrics.processed;s.unresolved+=metrics.unresolved;s.validation_rejected+=Number(!method.review.accepted);
    s.by_start[cell.task.start].correct+=metrics.correct;s.by_start[cell.task.start].total+=metrics.total;
  }
  return result;
}
function main() {
  const [mode,dir,...flags]=process.argv.slice(2);assert.ok(['collect','verify'].includes(mode)&&dir,'collect|verify DIRECTORY [--live]');
  if(mode==='collect') {
    assert.ok(!existsSync(dir),'Immutable collection; choose a new directory');mkdirSync(dir,{recursive:true});
    const data={tasks:[],runs:{},interventions:[]},started=performance.now();
    try {for(const task of meaningTasks()){
      const {fitted,shuffled,methods:list}=methods(task);const cell={task,fitted,shuffled,methods:[]};
      for(const method of list){const compiled=compileContext(method.model,unlabel(task.test),method.options),key=digest(compiled);
        if(!data.runs[key]){data.runs[key]=run(compiled);assert.equal(data.runs[key].code,0,'Native resource-inconclusive baseline');}
        cell.methods.push({...method,run:key});}
      data.tasks.push(cell);
      if(task.rotation===0){const variants=interventions(task).map(([name,c])=>({name,...run(c,name==='zero-step'?0:512)}));data.interventions.push({task:task.id,runs:variants,checks:checkInterventions(variants,task)});}
      if(data.tasks.length%12===0)console.error(`Collected ${data.tasks.length}/144 tasks`);
    }} catch(error) {
      writeFileSync(`${dir}/incomplete.json.gz`,gzipSync(JSON.stringify(data)));
      writeFileSync(`${dir}/failure.json`,JSON.stringify({error:error.message,completed_tasks:data.tasks.length,date:new Date().toISOString()},null,2)+'\n');
      throw error;
    }
    const paths=['Cargo.toml','Cargo.lock','rust-toolchain.toml','platform/meaning.mjs','platform/learning.mjs','scripts/meaning-corpus.mjs','scripts/meaning-study.mjs','scripts/meaning.test.mjs','scripts/interpret-context.mjs','scripts/research-source-files.mjs','scripts/trace-comparison.mjs','benchmarks/PROTOCOL.md',...['cli','engine','parser','graph-core'].flatMap(c=>[`crates/${c}/Cargo.toml`,...sourceFiles([`crates/${c}/src`])])];
    const sources=Buffer.from(JSON.stringify(Object.fromEntries(paths.sort().map(p=>[p,readFileSync(p,'utf8')])))),bytes=Buffer.from(JSON.stringify(data));
    const report={schema:'meaning-study-v1',date:new Date().toISOString(),revision:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),source_sha256:sha(sources),data_sha256:sha(bytes),binary_sha256:sha(readFileSync(binary)),node:process.version,platform:process.platform,arch:process.arch,summary:summary(data),native_runs:Object.keys(data.runs).length,main_commits:Object.values(data.runs).reduce((s,r)=>s+r.trace.events.length,0),intervention_runs:data.interventions.reduce((s,c)=>s+c.runs.length,0),intervention_commits:data.interventions.reduce((s,c)=>s+c.runs.reduce((t,r)=>t+r.trace.events.length,0),0),collection_ms:performance.now()-started};
    writeFileSync(`${dir}/sources.json.gz`,gzipSync(sources));writeFileSync(`${dir}/outcomes.json.gz`,gzipSync(bytes));writeFileSync(`${dir}/report.json`,JSON.stringify(report,null,2)+'\n');
  }
  const report=JSON.parse(readFileSync(`${dir}/report.json`));assert.equal(report.schema,'meaning-study-v1');
  const sources=gunzipSync(readFileSync(`${dir}/sources.json.gz`)),bytes=gunzipSync(readFileSync(`${dir}/outcomes.json.gz`));assert.equal(sha(sources),report.source_sha256);assert.equal(sha(bytes),report.data_sha256);
  const data=JSON.parse(bytes),tasks=meaningTasks();assert.equal(data.tasks.length,tasks.length);const keys=new Set();
  data.tasks.forEach((cell,i)=>{assert.deepEqual(cell.task,tasks[i]);const expected=methods(tasks[i]);assert.deepEqual(cell.fitted,expected.fitted);assert.deepEqual(cell.shuffled,expected.shuffled);assert.equal(cell.methods.length,expected.methods.length);
    cell.methods.forEach((method,j)=>{const {run:key,...rest}=method;assert.deepEqual(rest,expected.methods[j]);const c=compileContext(method.model,unlabel(tasks[i].test),method.options);assert.equal(key,digest(c));keys.add(key);assert.deepEqual(data.runs[key].compiled,c);});});
  assert.deepEqual(Object.keys(data.runs).sort(),[...keys].sort());assert.equal(keys.size,report.native_runs);
  Object.values(data.runs).forEach(r=>checkRun(r,flags.includes('--live')));
  const selected=tasks.filter(t=>t.rotation===0);assert.equal(data.interventions.length,selected.length);
  data.interventions.forEach((cell,i)=>{const task=selected[i];assert.equal(cell.task,task.id);const variants=interventions(task);assert.equal(cell.runs.length,variants.length);cell.runs.forEach((r,j)=>{const {name,...record}=r;assert.equal(name,variants[j][0]);assert.deepEqual(record.compiled,variants[j][1]);checkRun(record,flags.includes('--live'));});assert.deepEqual(cell.checks,checkInterventions(cell.runs,task));});
  assert.deepEqual(summary(data),report.summary);
  assert.equal(report.main_commits,Object.values(data.runs).reduce((s,r)=>s+r.trace.events.length,0));
  assert.equal(report.intervention_runs,data.interventions.reduce((s,c)=>s+c.runs.length,0));assert.equal(report.intervention_commits,data.interventions.reduce((s,c)=>s+c.runs.reduce((t,r)=>t+r.trace.events.length,0),0));
  console.log(JSON.stringify({verified:true,...report,summary:report.summary}));
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main();
