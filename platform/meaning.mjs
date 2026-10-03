// Restricted contextual interpretation. Core matching/validation remain external.
import assert from 'node:assert/strict';
import {digest,execute} from './learning.mjs';
export const SCHEMA='context-calendar-v1';
const q=JSON.stringify;
const number=i=>String(i+1).padStart(2,'0');
function scoped(source,prefix) {
  const words=new Set(['rule','atom','rank','mode','revision','active','lhs','rhs']);
  return source.replace(/"(?:\\.|[^"\\])*"|\b[a-z_][a-z_0-9]*\b/g,s=>s.startsWith('"')||words.has(s)?s:prefix+s);
}
function exact(value,fields) {
  assert.ok(value&&typeof value==='object'&&!Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(),[...fields].sort(),'Unexpected or missing fields');
}
function name(s){assert.ok(typeof s==='string'&&/^[a-z][a-z0-9-]{0,31}$/.test(s),'Invalid context name');}
export function policy(value) {
  exact(value,['schema','contexts']);assert.equal(value.schema,SCHEMA);
  assert.ok(Array.isArray(value.contexts)&&value.contexts.length>0&&value.contexts.length<=8);
  const seen=new Set();
  for(const c of value.contexts){exact(c,['name','offset']);name(c.name);assert.ok(!seen.has(c.name),'Duplicate context');seen.add(c.name);assert.ok(c.offset===null||(Number.isInteger(c.offset)&&c.offset>=0&&c.offset<12),'Invalid offset');}
  return {schema:SCHEMA,contexts:value.contexts.map(c=>({...c})).sort((a,b)=>a.name<b.name?-1:a.name>b.name?1:0)};
}
export function rows(input,labelled=false) {
  assert.ok(Array.isArray(input)&&input.length>0&&input.length<=64,'Expected 1–64 rows');
  for(const r of input){exact(r,labelled?['token','context','expected']:['token','context']);name(r.context);assert.ok(typeof r.token==='string'&&/^(0[1-9]|1[0-2])$/.test(r.token),'Expected ordinal token 01..12');
    if(labelled)assert.ok(r.expected===null||(typeof r.expected==='string'&&/^(0[1-9]|1[0-2])$/.test(r.expected)));}
  return input.map(r=>({...r}));
}
export function infer(model,row) {
  model=policy(model);rows([row]);const c=model.contexts.find(c=>c.name===row.context);
  return c?{processed:true,value:c.offset===null?null:number((Number(row.token)-1+c.offset)%12)}:{processed:false,value:null};
}
export function fitContexts(training) {
  training=rows(training,true);const contexts=[...new Set(training.map(r=>r.context))].sort();const candidates=[];
  const selected=contexts.map(context=>{
    const examples=training.filter(r=>r.context===context);
    const scores=[...Array(12).keys(),null].map(offset=>({offset,errors:examples.filter(r=>infer({schema:SCHEMA,contexts:[{name:context,offset}]},{token:r.token,context}).value!==r.expected).length}));
    scores.sort((a,b)=>a.errors-b.errors||(a.offset??12)-(b.offset??12));candidates.push({context,scores});return {name:context,offset:scores[0].offset};
  });
  return {schema:'context-fit-v1',training_sha256:digest(training),model:policy({schema:SCHEMA,contexts:selected}),candidates};
}
export function review(model,validation) {
  model=policy(model);validation=rows(validation,true);
  const predictions=validation.map(({token,context})=>infer(model,{token,context}));
  const errors=validation.filter((r,i)=>!predictions[i].processed||predictions[i].value!==r.expected).length;
  return {schema:'context-review-v1',accepted:errors===0,errors,approval_sha256:digest({model,validation})};
}
export function vocabulary(model,input,renamed=false) {
  const contexts=[...new Set([...model.contexts.map(c=>c.name),...input.map(r=>r.context)])].sort();
  return {roles:Object.fromEntries(['input','output','context','key','scope','value'].map((s,i)=>[s,renamed?`relation-${i}`:s])),
    tokens:Array.from({length:12},(_,i)=>renamed?`t${i}`:`Token:${number(i)}`),
    contexts:contexts.map((s,i)=>({name:s,label:renamed?`c${i}`:`Context:${s}`})),
    values:[...Array.from({length:12},(_,i)=>renamed?`v${i}`:`Month:${number(i)}`),renamed?'v12':'NotApplicable']};
}
export function compileContext(model,input,options={}) {
  model=policy(model);input=rows(input);
  assert.ok(Object.keys(options).every(k=>['ruleFree','renamed','dropContext','dropDeclarations','inert'].includes(k)));
  assert.ok(Object.values(options).every(v=>typeof v==='boolean'));
  const lexicon=vocabulary(model,input,options.renamed),r=lexicon.roles;
  let source='program {\n';
  // Specialise one rule schema per context; table values remain graph data.
  if(!options.ruleFree)model.contexts.forEach((context,i)=>{source+=scoped(`rule interpret {
 atom iz = Nat(); rank ir = iz; mode im = ModeObject; revision iv;
 atom row = PatternVariable[Constructor] Free;
 atom tok = PatternVariable[Constructor] Free;
 atom ctx = Constructor(${q(lexicon.contexts.find(c=>c.name===context.name).label)});
 atom val = PatternVariable[Constructor] Free;
 atom old = Record[ObjectLink](${q(r.input)}, row, tok);
 atom rc = Record[ObjectLink](${q(r.context)}, row, ctx);
 atom mapping = Record[ObjectLink](${q(r.value+':'+i)}, tok, val);
 atom out = Record[ObjectLink](${q(r.output)}, row, val);
 lhs { lrow: row; ltok: tok; lctx: ctx; lval: val; lold: old; lrc: rc; lm: mapping; }
 rhs { rrow: row; rtok: tok; rctx: ctx; rval: val; rout: out; rrc: rc; rm: mapping; }
}\n`,'p'+i+'_');});
  if(!options.ruleFree)source+=`rule approve {
 atom az = Nat(); atom ao = Nat(az); rank ar = ao; mode am = ModeMeta; revision av; active aa;
 atom target = PatternVariable[RuleHead] Free;
 atom tz = Nat(); atom tr = Record[Rank](target, tz); atom tm = Record[ModeObject](target);
 atom oldrev = Record[Revision](target); atom newrev = Record[Revision](target); atom activation = Record[Active](target);
 lhs { at: target; alz: tz; alr: tr; alm: tm; alv: oldrev; }
 rhs { bt: target; brz: tz; brr: tr; brm: tm; brv: newrev; bra: activation; }
}\n`;
  lexicon.tokens.forEach((s,i)=>{source+=`atom token_${i} = Constructor(${q(s)});\n`;});
  lexicon.values.forEach((s,i)=>{source+=`atom value_${i} = Constructor(${q(s)});\n`;});
  lexicon.contexts.forEach((c,i)=>{source+=`atom context_${i} = Constructor(${q(c.label)});\n`;});
  if(!options.dropDeclarations)model.contexts.forEach((c,j)=>{
    for(let i=0;i<12;i++)source+=`atom meaning_${j}_${i} = Record[ObjectLink](${q(r.value+':'+j)}, token_${i}, value_${c.offset===null?12:(i+c.offset)%12});\n`;
  });
  input.forEach((row,i)=>{
    source+=`atom data_${i} = Constructor(${q('Row:'+q([i,row.token,row.context]))});\natom input_${i} = Record[ObjectLink](${q(r.input)}, data_${i}, token_${Number(row.token)-1});\n`;
    if(!options.dropContext)source+=`atom context_link_${i} = Record[ObjectLink](${q(r.context)}, data_${i}, context_${lexicon.contexts.findIndex(c=>c.name===row.context)});\n`;
  });
  if(options.inert)source+='atom unrelated = Constructor("unrelated-note");\n';
  return {schema:'context-compiled-v1',source:source+'}\n',lexicon,model,input,options};
}
export function observeContext(trace,compiled) {
  const original=rows(compiled.input),lex=compiled.lexicon,initial=trace.initial.atoms,after=new Map(Object.entries(initial));
  for(const event of trace.events){for(const id of event.footprint.consumed)assert.ok(after.delete(id));for(const [id,a]of Object.entries(event.fresh)){assert.ok(!after.has(id));after.set(id,a);}}
  const starts=Object.entries(initial).filter(([,a])=>a.kind.Constructor?.startsWith('Row:'));
  assert.equal(starts.length,original.length);const seen=new Set(),results=[];
  for(const [id,a]of starts){assert.deepEqual(after.get(id),a,'Payload frame changed');const [i,token,context]=JSON.parse(a.kind.Constructor.slice(4));assert.ok(Number.isInteger(i)&&i>=0&&i<original.length&&!seen.has(i));seen.add(i);assert.deepEqual({token,context},original[i]);
    const links=[...after.values()].filter(x=>x.kind.Record?.role==='ObjectLink'&&[lex.roles.input,lex.roles.output].includes(x.kind.Record.label)&&x.fields[0]===id);
    assert.equal(links.length,1,'Singleton status required');const link=links[0],processed=link.kind.Record.label===lex.roles.output;
    const target=after.get(link.fields[1])?.kind.Constructor;let value=null;
    if(processed){const index=lex.values.indexOf(target);assert.ok(index>=0,'Unknown output symbol');value=index===12?null:number(index);}
    else assert.equal(target,lex.tokens[Number(token)-1]);
    results.push({row:i,token,context,processed,value});
  }
  assert.equal([...after.values()].filter(a=>a.kind.Constructor?.startsWith('Row:')).length,starts.length);
  // No initial atom except input links and the processor's revision may disappear.
  for(const [id,a]of Object.entries(initial))if(!after.has(id))assert.ok(a.kind.Record?.role==='Revision'||(a.kind.Record?.role==='ObjectLink'&&a.kind.Record.label===lex.roles.input),'Protected atom deleted');else assert.deepEqual(after.get(id),a,'Protected atom altered');
  return {outcome:trace.report.outcome,commits:trace.events.length,results:results.sort((a,b)=>a.row-b.row)};
}
export function runContext(binary,compiled,maxSteps=512) {
  const run=execute(binary,compiled,maxSteps);return {...run,observation:observeContext(run.trace,compiled)};
}
export function promoteContext(binary,model,validation,input,approval) {
  model=policy(model);validation=rows(validation,true);input=rows(input);
  const gate=review(model,validation);assert.ok(gate.accepted,'Context validation rejected');assert.equal(approval,gate.approval_sha256,'Explicit matching approval required');
  const compiledValidation=compileContext(model,validation.map(({token,context})=>({token,context})));
  const checked=runContext(binary,compiledValidation);assert.equal(checked.code,0,'Validation execution inconclusive');
  assert.deepEqual(checked.observation.results.map(r=>({processed:r.processed,value:r.value})),validation.map(r=>({processed:true,value:r.expected})));
  const compiled=compileContext(model,input);
  return {schema:'context-run-v1',review:gate,compiled,validation:checked,...runContext(binary,compiled)};
}
