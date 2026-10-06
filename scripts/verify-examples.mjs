// Catalogue regression, not an independent semantic oracle. See examples/README.md.
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
const root=new URL('../',import.meta.url);
const read=p=>readFileSync(new URL(p,root),'utf8');
export const catalogue=JSON.parse(read('examples/manifest.json'));
function invoke(binary,args,input,codes=[0]){
 const r=spawnSync(fileURLToPath(new URL(binary,root)),args,{cwd:root,input,encoding:'utf8',maxBuffer:64*1024*1024});
 assert.ifError(r.error);assert.ok(codes.includes(r.status),`${args.join(' ')}: ${r.status}\n${r.stderr}`);return r.stdout;
}
const cli=(args,input,codes)=>invoke('target/debug/cli',args,input,codes);
const native=commands=>invoke('target/debug/examples/protocol',[],commands.map(c=>JSON.stringify(c)).join('\n')+'\n').trim().split('\n').map(JSON.parse);
function canonical(v){if(Array.isArray(v))return v.map(canonical);if(v&&typeof v==='object')return Object.fromEntries(Object.keys(v).sort().map(k=>[k,canonical(v[k])]));return v;}
const digest=v=>createHash('sha256').update(JSON.stringify(canonical(v))).digest('hex');
export function verifyExamples(compare=native,regenerate=false){
 assert.equal(catalogue.length,19);assert.equal(new Set(catalogue.map(e=>e.file)).size,19);
 assert.deepEqual([...new Set(catalogue.map(e=>e.category))].sort(),['arithmetic','collapse','independence','lambda','oscillation','practical','reflection','stability']);
 const profiles=catalogue.map(e=>({...e,key:e.file}));
 for(const e of catalogue.filter(e=>['stability','collapse','oscillation'].includes(e.category))){
  profiles.push({...e,key:e.file+':value',profile:{...e.profile,observer:'observers/value.observer'},result:{...e.result,events:1}});
  if(e.category==='oscillation')profiles.push({...e,key:e.file+':bounded',profile:{...e.profile,detect_cycles:false},result:{outcome:'StepLimit',events:6}});
 }
 const golden={normalisation:'Canonical JSON key ordering; only implementation fingerprint omitted after full live trace verification. Identities, settings, source hash, reports and event evidence retained.',profiles:{}};
 for(const e of profiles){
  for(const p of [e.file,e.authored,e.documentation,e.profile.observer])assert.ok(existsSync(new URL('examples/'+p,root)),p);
  assert.ok(e.purpose&&e.expected&&e.evidence&&e.rqs.length);
  const source=read('examples/'+e.file),{observer,...settings}=e.profile;
  const config={...settings,observer:read('examples/'+observer)};
  if(e.authored.endsWith('.lambda'))assert.equal(cli(['lambda-compile','-'],read('examples/'+e.authored)).trim(),source.trim());
  if(e.file==='reflection/versioned.graph')assert.equal(cli(['version-compile','-'],read('examples/'+e.authored)).trim(),source.trim());
  const replies=compare([{command:'load',source,config},{command:'advance',batch:String(Number(settings.max_steps)+1)}]);
  assert.ok(replies.every(r=>r.ok),`${e.key}: ${JSON.stringify(replies.map(r=>r.error))}`);
  const final=replies.at(-1);assert.equal(final.state.status,e.result.outcome,e.key);assert.equal(final.state.events.length,e.result.events,e.key);
  const trace=JSON.parse(final.trace);
  const exported=cli(['export','-','--max-steps',settings.max_steps,'--strategy','seeded','--seed',settings.seed,'--observer',fileURLToPath(new URL('examples/'+observer,root)),'--detect-cycles',String(settings.detect_cycles)],source,[0,3,6]);
  assert.deepEqual(JSON.parse(exported),trace,e.key+' CLI/protocol trace');
  assert.match(cli(['trace-check','-'],final.trace),new RegExp(`Verified ${e.result.events} events`));
  const history=compare([{command:'import',trace:final.trace},...Array.from({length:e.result.events+1},(_,i)=>({command:'jump',state:String(i)}))]);
  assert.ok(history.every(r=>r.ok),e.key+' replay');
  assert.deepEqual(history[1].state.graph,replies[0].state.graph);
  assert.deepEqual(history.at(-1).state.graph,final.state.graph);
  // CLI source replay is intentionally not an identity-bearing checkpoint.
  for(const i of new Set([0,Math.floor(e.result.events/2),e.result.events]))cli(['check','-'],cli(['replay','-','--at',String(i)],final.trace));
  const {implementation,...stable}=trace;assert.match(implementation,/source-sha256=/);
  golden.profiles[e.key]={outcome:final.state.status,events:e.result.events,trace_sha256:digest(stable),state_sha256:history.slice(1).map(r=>digest(r.state.graph))};
 }
 const file=new URL('examples/catalogue-golden.json',root);
 if(regenerate)writeFileSync(file,JSON.stringify(golden,null,2)+'\n');
 else assert.deepEqual(golden,JSON.parse(readFileSync(file,'utf8')),'Catalogue golden drift; inspect semantics before explicit regeneration.');
 return profiles.length;
}
if(process.argv[1]===fileURLToPath(import.meta.url)){
 assert.ok(process.argv.slice(2).every(a=>a==='--regenerate'),'Unknown argument');
 console.log(`Verified ${verifyExamples(native,process.argv.includes('--regenerate'))} catalogue execution profiles through CLI, public trace validation and native replay.`);
}
