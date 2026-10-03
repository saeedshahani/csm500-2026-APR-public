// One declared semantic panel. No timing field is erased from an execution trace.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {digest,fit,compile,inspect} from '../platform/learning.mjs';
import {fitContexts,compileContext,observeContext} from '../platform/meaning.mjs';
import {corpus} from './learning-corpus.mjs';
import {meaningTasks} from './meaning-corpus.mjs';
import {encode,decode} from './minimality-codecs.mjs';

export function call(binary,args,input,cwd=process.cwd()) {
  const p=spawnSync(binary,args,{input,cwd,encoding:'utf8',timeout:120000,maxBuffer:64*1024*1024});
  assert.ok(!p.error&&!p.signal,p.error?.message??p.signal);return {code:p.status,stdout:p.stdout,stderr:p.stderr};
}
function success(p){assert.equal(p.code,0,p.stderr);return JSON.parse(p.stdout);}
export function proposals() {
  return {calendar:corpus().map(t=>({id:t.id,fit:fit(t.training)})),context:meaningTasks().map(t=>({id:t.id,fit:fitContexts(t.training)}))};
}
export function panel(binary,root=process.cwd()) {
  const source=p=>readFileSync(resolve(root,`examples/${p}.graph`),'utf8');
  const cases=[
    ['arithmetic','programs/arithmetic',4,'reference-v1'],
    ['self-modification','programs/self-modify',3,'reference-v1'],
    ['staging','programs/staging',4,'reference-v1'],
    ['oscillation','behaviour/oscillating',8,'reference-v1'],
    ['independent','independent/shared-read',2,'reference-v1'],
    ['framed','practical/work-items',4,'framed-v3'],
    ['joined','practical/month-normalisation',8,'joined-v4'],
    ['body-revision','practical/revise-guard',8,'indexed-v2'],
    ['credit-spawn','practical/credit-spawn',8,'indexed-v2'],
    ['inert-log','practical/inert-log',8,'indexed-v2']
  ].map(([id,file,steps,profile])=>({id,source:source(file),steps,profile}));
  const term='((\\z. (\\x. z)) x)',lambda=call(binary,['lambda-compile','-','--profile','open-cbn-v1','--free','x'],term,root);assert.equal(lambda.code,0,lambda.stderr);
  cases.push({id:'open-lambda',source:lambda.stdout,steps:20,profile:'indexed-v2'});
  const kt=corpus().find(t=>t.corruption===63),km=fit(kt.training).selected,ki=[' MONTH=JAN. ','month=feb','unknown'];
  cases.push({id:'learned-policy',...compile(km,ki,true),input:ki,steps:20,profile:'indexed-v2'});
  const lt=meaningTasks().find(t=>t.id==='r0-s3'),lm=fitContexts(lt.training).model;
  for(const unknown of [false,true]){
    const input=['calendar','fiscal','noncalendar',...(unknown?['unknown']:[])].map(context=>({token:'03',context}));
    const compiled=compileContext(lm,input);cases.push({id:unknown?'context-unknown':'context',source:compiled.source,compiled,steps:20,profile:'indexed-v2'});
  }
  return cases;
}
export function commands(c) {return [{command:'load',source:c.source,config:{max_steps:String(c.steps),seed:'42',detect_cycles:false,execution_profile:c.profile}},{command:'advance',batch:String(c.steps+1)}];}
export function checkReplies(c,replies) {
  assert.equal(replies.length,2);assert.ok(replies.every(r=>r.ok),JSON.stringify(replies));
  const trace=typeof replies[1].trace==='string'?JSON.parse(replies[1].trace):replies[1].trace;assert.ok(trace);assert.notEqual(trace.report.outcome,'ResourceLimit');
  const count=trace.events.length;assert.ok(count>0&&count<=c.steps);
  if(c.id==='open-lambda')assert.equal(replies[1].state.lambda.term,'(λ.x)');
  const observation=c.id==='learned-policy'?inspect(trace,c.input):c.compiled?observeContext(trace,c.compiled):null;
  if(c.id==='learned-policy')assert.deepEqual(observation.results.map(r=>r.output),['01','02',null]);
  if(c.compiled)assert.deepEqual(observation.results.map(r=>[r.processed,r.value]),[[true,'03'],[true,'06'],[true,null],...(c.id==='context-unknown'?[[false,null]]:[])]);
  // The codec oracle preserves every atom after transporting decoded identities.
  const codecs={};for(const variant of ['M','T','P']){const {wire,mapping}=encode(trace.initial,variant),decoded=decode(wire,variant);const atoms=decoded.atoms??decoded;
    for(const[id,a]of Object.entries(trace.initial.atoms))assert.deepEqual(atoms[mapping[id]],{...a,fields:a.fields.map(x=>mapping[x])});
    codecs[variant]=digest(wire);}
  return {trace,observation,codecs};
}
export function nativePanel(binary,protocol,root=process.cwd()) {
  const cases=panel(binary,root),runs=[];
  for(const c of cases){const p=call(protocol,[],commands(c).map(JSON.stringify).join('\n')+'\n',root);assert.equal(p.code,0,p.stderr);
    const replies=p.stdout.trim().split('\n').map(JSON.parse),checked=checkReplies(c,replies);
    assert.equal(call(binary,['trace-check','-'],JSON.stringify(checked.trace),root).code,0);runs.push({case:c,...checked});}
  const analyse=(args,input)=>success(call(binary,args,input,root));
  const analyses={};
  for(const name of ['revise-guard','revise-output'])analyses[name]=analyse(['stability-check',`examples/practical/${name}.graph`,`examples/practical/${name}.contract.json`]);
  analyses.finite=analyse(['meta-check','examples/practical/credit-spawn.graph','finite-edits','none']);
  analyses.inert=analyse(['meta-abstract','examples/practical/inert-log.graph','finite-edits','none','Log']);
  const independent=runs.find(r=>r.case.id==='independent').trace;
  for(const query of ['exact','source-revision'])analyses[query]=analyse(['trace-explain','-','--event','1','--query',query],JSON.stringify(independent));
  const request=analyse(['workflow-example','branching','2','persistent']);request.max_steps=100;request.meta_burst=1;
  analyses.workflow=analyse(['workflow-run','-'],JSON.stringify(request));assert.equal(analyses.workflow.outcome,'Completed');
  assert.equal(call(binary,['trace-check','-'],JSON.stringify(analyses.workflow.trace),root).code,0);
  const forged=structuredClone(independent);forged.events[0].source='18446744073709551614';
  assert.notEqual(call(binary,['trace-check','-'],JSON.stringify(forged),root).code,0);
  return {schema:'reproduction-panel-v1',runs,analyses,proposals:proposals(),corrupt_trace_rejected:true};
}
export function wasmPanel(modulePath,expected) {
  const require=createRequire(import.meta.url),{WorkbenchHandle}=require(resolve(modulePath)),results=[];
  for(const r of expected.runs){const h=new WorkbenchHandle();try{
    const replies=commands(r.case).map(c=>h.dispatch(JSON.stringify(c))),actual=checkReplies(r.case,replies);
    assert.deepEqual(actual,{trace:r.trace,observation:r.observation,codecs:r.codecs});
    const other=new WorkbenchHandle();try{assert.equal(other.dispatch(JSON.stringify({command:'import',trace:JSON.stringify(r.trace)})).ok,true);}finally{other.free();}
    results.push({id:r.case.id,trace_sha256:digest(actual.trace)});
  }finally{h.free();}}
  return results;
}
