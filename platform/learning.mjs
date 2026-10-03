// Local, finite program induction. No providers, dynamic code or network access.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';

export const MODEL = 'calendar-subsets-v1';
export const operations = ['trim','lowercase','month-prefix','terminal-dot','full-name','numeric'];
const short = ['jan','feb','mar','apr','may','jun','jul','aug','sep','oct','nov','dec'];
const full = ['january','february','march','april','may','june','july','august','september','october','november','december'];
const q = JSON.stringify;
export const digest = value => createHash('sha256').update(q(value)).digest('hex');
const bits = mask => operations.reduce((n,_,i)=>n+((mask>>i)&1),0);
// Source names are file-wide. Never rewrite words inside quoted user tokens.
function scoped(source,prefix) {
  const keywords=new Set(['rule','atom','rank','mode','revision','active','lhs','rhs']);
  return source.replace(/"(?:\\.|[^"\\])*"|\b[a-z_][a-z_0-9]*\b/g,s=>s.startsWith('"')||keywords.has(s)?s:prefix+s);
}
function keys(value, required) {
  assert.ok(value && typeof value==='object' && !Array.isArray(value),'Expected an object');
  assert.deepEqual(Object.keys(value).sort(),[...required].sort(),'Unexpected or missing fields');
}
export function proposal(masks) { return validateProposal({schema:MODEL,masks:[...masks].sort((a,b)=>a-b)}); }
export function validateProposal(value) {
  keys(value,['schema','masks']);
  assert.equal(value.schema,MODEL,'Unknown model');
  assert.ok(Array.isArray(value.masks)&&value.masks.length<=64,'At most 64 hypotheses');
  assert.ok(value.masks.every((v,i)=>Number.isInteger(v)&&v>=0&&v<64&&(i===0||v>value.masks[i-1])),'Masks must be sorted, distinct integers in 0..63');
  return {schema:MODEL,masks:[...value.masks]};
}
function token(value) { assert.ok(typeof value==='string'&&value.length<=128&&!/[\x00-\x1f\x7f]/.test(value),'Token must be at most 128 characters without controls'); }
export function predictMask(mask, value) {
  token(value);assert.ok(Number.isInteger(mask)&&mask>=0&&mask<64,'Invalid mask');
  let s=value;
  if(mask&1)s=s.trim();
  if(mask&2)s=s.toLowerCase();
  if(mask&4)s=s.replace(/^month=/,'');
  if(mask&8)s=s.replace(/\.$/,'');
  if(mask&16){const i=full.indexOf(s);if(i>=0)s=short[i];}
  if((mask&32)&&/^(0[1-9]|1[0-2])$/.test(s))return s;
  const i=short.indexOf(s);return i<0?null:String(i+1).padStart(2,'0');
}
export function predict(model, value) {
  const {masks}=validateProposal(model);token(value);
  if(!masks.length)return null;
  const values=masks.map(m=>predictMask(m,value));return values.every(v=>v===values[0])?values[0]:null;
}
export function examples(rows) {
  assert.ok(Array.isArray(rows)&&rows.length>0&&rows.length<=128,'Expected 1–128 examples');
  for(const r of rows){keys(r,['input','expected']);token(r.input);assert.ok(r.expected===null||/^(0[1-9]|1[0-2])$/.test(r.expected),'Expected a month number or null');}
  return rows.map(r=>({...r}));
}
export function fit(training) {
  training=examples(training);
  const scores=Array.from({length:64},(_,mask)=>({mask,errors:training.filter(r=>predictMask(mask,r.input)!==r.expected).length,operations:bits(mask)}));
  scores.sort((a,b)=>a.errors-b.errors||a.operations-b.operations||a.mask-b.mask);
  return {schema:'calendar-fit-v1',training_sha256:digest(training),selected:proposal([scores[0].mask]),consensus:proposal(scores.filter(s=>s.errors===0).map(s=>s.mask)),scores};
}
export function score(model, rows) {
  rows=examples(rows);validateProposal(model);
  const counts={total:rows.length,correct:0,positives:0,correct_positive:0,unresolved:0,wrong_positive:0,negatives:0,false_acceptance:0};
  for(const r of rows){const p=predict(model,r.input);if(p===r.expected)counts.correct++;
    if(r.expected!==null){counts.positives++;if(p===r.expected)counts.correct_positive++;else if(p===null)counts.unresolved++;else counts.wrong_positive++;}
    else{counts.negatives++;if(p!==null)counts.false_acceptance++;}}
  return counts;
}
export function gate(model, validation) {
  model=validateProposal(model);validation=examples(validation);
  const result=score(model,validation);
  // A list of only negative examples cannot authorise a processing policy.
  return {schema:'calendar-gate-v1',accepted:result.positives>0&&result.correct===result.total,
    approval_sha256:digest({model,validation}),model,validation,result};
}
function inputs(rows) {
  assert.ok(Array.isArray(rows)&&rows.length>0&&rows.length<=128,'Expected 1–128 rows');
  rows.forEach(token);return [...rows];
}

export function compile(model, rows, approved=false) {
  model=validateProposal(model);rows=inputs(rows);
  assert.equal(typeof approved,'boolean');
  const mapping=[...new Set(rows)].map(input=>({input,output:predict(model,input)})).filter(r=>r.output!==null);
  let source='program {\n';
  mapping.forEach((r,i)=>{source+=scoped(`rule process {
 atom z = Nat(); rank rk = z; mode md = ModeObject; revision rev;
 atom row = PatternVariable[Constructor] Free;
 atom raw = Constructor(${q('Raw:'+r.input)});
 atom old = Record[ObjectLink]("input", row, raw);
 atom value = Constructor(${q('MonthNumber:'+r.output)});
 atom result = Record[ObjectLink]("output", row, value);
 lhs { lr: row; lw: raw; li: old; }
 rhs { rr: row; rw: raw; rv: value; ro: result; }
}\n`,`p${i}_`);});
  if(approved&&mapping.length)source+=scoped(`rule approve {
 atom z = Nat(); atom one = Nat(z); rank rk = one;
 mode md = ModeMeta; revision rev; active enabled;
 atom target = PatternVariable[RuleHead] Free;
 atom tz = Nat(); atom tr = Record[Rank](target, tz);
 atom tm = Record[ModeObject](target);
 atom oldrev = Record[Revision](target); atom newrev = Record[Revision](target);
 atom activation = Record[Active](target);
 lhs { lt: target; lz: tz; lr: tr; lm: tm; lv: oldrev; }
 rhs { rt: target; rz: tz; rr: tr; rm: tm; rv: newrev; ra: activation; }
}\n`,'a_');
  [...new Set(rows)].forEach((s,i)=>{source+=`atom raw_${i} = Constructor(${q('Raw:'+s)});\n`;});
  const unique=[...new Set(rows)];
  rows.forEach((s,i)=>{source+=`atom row_${i} = Constructor(${q('Row:'+q([i,s]))});\natom input_${i} = Record[ObjectLink]("input", row_${i}, raw_${unique.indexOf(s)});\n`;});
  return {source:source+'}\n',processors:mapping.length,rows:rows.length,mapping};
}

export function native(binary,args,input) {
  const r=spawnSync(binary,args,{input,encoding:'utf8',timeout:30000,maxBuffer:32*1024*1024});
  if(r.error||r.signal)throw Error(`Native execution inconclusive: ${r.error?.message??r.signal}`);
  assert.ok([0,3].includes(r.status),r.stderr||`Native exit ${r.status}`);
  return {code:r.status,stdout:r.stdout,stderr:r.stderr};
}
export function execute(binary,compiled,maxSteps=512) {
  assert.ok(Number.isInteger(maxSteps)&&maxSteps>=0&&maxSteps<=512);
  const checked=native(binary,['check','-'],compiled.source);assert.equal(checked.code,0);
  const r=native(binary,['export','-','--max-steps',String(maxSteps),'--execution-profile','indexed-v2','--detect-cycles','false'],compiled.source);
  assert.ok(r.stdout.trim(),`Native execution inconclusive: ${r.stderr}`);
  const trace=JSON.parse(r.stdout);
  const replay=native(binary,['trace-check','-'],q(trace));assert.equal(replay.code,0);
  return {code:r.code,trace};
}
// The oracle inspects immutable row payloads and final graph links, not the model.
export function inspect(trace, original) {
  original=inputs(original);
  const before=trace.initial.atoms,after=new Map(Object.entries(before));
  for(const e of trace.events){for(const id of e.footprint.consumed)assert.ok(after.delete(id),'Missing consumed atom');for(const [id,a] of Object.entries(e.fresh)){assert.ok(!after.has(id),'Identity reuse');after.set(id,a);}}
  const rows=Object.entries(before).filter(([,a])=>a.kind.Constructor?.startsWith('Row:'));
  assert.equal(rows.length,original.length);
  const seen=new Set();const results=[];
  for(const [id,old] of rows){assert.deepEqual(after.get(id),old,'Payload frame violation');const [i,raw]=JSON.parse(old.kind.Constructor.slice(4));
    assert.ok(Number.isInteger(i)&&i>=0&&i<original.length&&!seen.has(i),'Row identity mismatch');seen.add(i);assert.equal(raw,original[i]);
    const links=[...after.values()].filter(a=>a.kind.Record?.role==='ObjectLink'&&['input','output'].includes(a.kind.Record.label)&&a.fields[0]===id);
    assert.equal(links.length,1,'Exactly one status per row');const link=links[0],value=after.get(link.fields[1])?.kind.Constructor;
    if(link.kind.Record.label==='input'){assert.equal(value,'Raw:'+raw);results.push({row:i,input:raw,output:null});}
    else{assert.match(value??'',/^MonthNumber:(0[1-9]|1[0-2])$/);results.push({row:i,input:raw,output:value.slice(12)});}}
  for(const [id,a] of Object.entries(before))if(a.kind.Constructor?.startsWith('Raw:'))assert.deepEqual(after.get(id),a,'Raw token changed');
  assert.equal([...after.values()].filter(a=>a.kind.Constructor?.startsWith('Row:')).length,rows.length,'Unexpected row creation');
  return {outcome:trace.report.outcome,results:results.sort((a,b)=>a.row-b.row),commits:trace.events.length};
}

export function promote(binary, model, validation, rows, approval) {
  const checked=gate(model,validation);
  assert.equal(checked.accepted,true,'Validation gate rejected proposal');
  assert.equal(approval,checked.approval_sha256,'Explicit matching approval digest required');
  const v=compile(model,validation.map(r=>r.input),true),vr=execute(binary,v);
  assert.equal(vr.code,0,'Validation execution inconclusive');
  const observation=inspect(vr.trace,validation.map(r=>r.input));
  assert.deepEqual(observation.results.map(r=>r.output),validation.map(r=>r.expected),'Native validation disagrees');
  const compiled=compile(model,rows,true),run=execute(binary,compiled);
  return {schema:'calendar-promotion-v1',approval_sha256:checked.approval_sha256,source_sha256:digest(compiled.source),compiled,validation_trace:vr.trace,...run,observation:inspect(run.trace,rows)};
}
