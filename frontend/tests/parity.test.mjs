import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const require=createRequire(import.meta.url);const {WorkbenchHandle}=require('../pkg-node/wasm_bridge.js');
const root=new URL('../../',import.meta.url);
const source=name=>readFileSync(new URL(`examples/${name}`,root),'utf8');
execFileSync('cargo',['build','-p','wasm-bridge','--example','protocol','--locked'],{cwd:root,stdio:'inherit'});
function compare(commands){const native=execFileSync(fileURLToPath(new URL('target/debug/examples/protocol',root)),[],{input:commands.map(c=>JSON.stringify(c)).join('\n')+'\n',encoding:'utf8',maxBuffer:64*1024*1024}).trim().split('\n').map(JSON.parse);const handle=new WorkbenchHandle();try{const wasm=commands.map(c=>handle.dispatch(JSON.stringify(c)));assert.deepEqual(wasm,native);return wasm;}finally{handle.free();}}
for(const name of ['programs/arithmetic.graph','programs/staging.graph','programs/self-modify.graph','programs/restoration.graph','independent/disjoint.graph','independent/shared-read.graph'])test(`exact native/WASM protocol parity: ${name}`,()=>{
  const replies=compare([{command:'load',source:source(name),config:{max_steps:'4',seed:'42'}},...Array.from({length:5},()=>({command:'advance',batch:'1'})),{command:'reset'},{command:'state'},{command:'dispose'},{command:'state'}]);assert.equal(replies[0].ok,true);assert.equal(replies.at(-1).ok,false);
});
test('invalid source, limits, empty graph and disposal agree',()=>{
  compare([{command:'state'},{command:'validate',source:'program {}'},{command:'load',source:'broken'},{command:'load',source:'program {}',config:{max_steps:'4294967296'}},{command:'load',source:source('behaviour/stable.graph'),config:{max_work:'0'}},{command:'advance',batch:'1'},{command:'import',trace:'{}'},{command:'jump',state:'18446744073709551615'},{command:'reset'},{command:'dispose'},{command:'dispose'}]);
});
for(const observer of ['all','value'])test(`multiple batch sizes preserve recurrence and RNG: ${observer}`,()=>{
  let final;
  for(const batch of ['1','2','5']){
    const commands=[{command:'load',source:source('behaviour/oscillating.graph'),config:{max_steps:'6',seed:'42',detect_cycles:true,observer:source(`observers/${observer}.observer`)}},...Array.from({length:4},()=>({command:'advance',batch})),{command:'export'}];
    const result=compare(commands).at(-1);const trace=JSON.parse(result.trace);assert.equal(trace.report.outcome,'CycleDetected');if(final)assert.deepEqual(trace,final);final=trace;
  }
});
test('CLI transfer, import, historical inspection and independent handles',()=>{
  const json=execFileSync('cargo',['run','--quiet','--locked','-p','cli','--','export','examples/behaviour/stable.graph','--max-steps','1'],{cwd:root,encoding:'utf8'});
  const result=compare([{command:'import',trace:json},{command:'jump',state:'0'},{command:'jump',state:'1'},{command:'export'}]);assert.equal(JSON.parse(result.at(-1).trace).source_sha256,JSON.parse(json).source_sha256);
  const checked=execFileSync('cargo',['run','--quiet','--locked','-p','cli','--','trace-check','-'],{cwd:root,input:result.at(-1).trace,encoding:'utf8'});assert.match(checked,/Verified 1 events/);
  const a=new WorkbenchHandle(),b=new WorkbenchHandle();try{assert.equal(a.dispatch(JSON.stringify({command:'load',source:'program {}'})).ok,true);assert.equal(b.dispatch(JSON.stringify({command:'state'})).ok,false);}finally{a.free();b.free();}
});
test('yielded WASM trace resumes identically after native revalidation',()=>{
  const handle=new WorkbenchHandle();const load={command:'load',source:source('behaviour/oscillating.graph'),config:{max_steps:'6',seed:'42',detect_cycles:true}};
  try{handle.dispatch(JSON.stringify(load));const prefix=handle.dispatch(JSON.stringify({command:'advance',batch:'1'}));const replies=compare([{command:'import',trace:prefix.trace},{command:'stop'},{command:'advance',batch:'1'},{command:'advance',batch:'1'}]);assert.equal(replies.at(-1).state.report.recurrence.first,'0');assert.equal(replies.at(-1).state.report.recurrence.repeated,'2');}finally{handle.free();}
});
test('all identity bits survive native/WASM import and replay',()=>{
  const d=JSON.parse(source('traces/stable.trace.json'));const offset=9007199254740999n;const id=s=>(BigInt(s)+offset).toString();
  const atoms=map=>Object.fromEntries(Object.entries(map).map(([key,a])=>[id(key),{...a,fields:a.fields.map(id)}]));
  d.initial.atoms=atoms(d.initial.atoms);d.initial.next_identity=id(d.initial.next_identity);d.source_sha256=null;d.initial_sha256=createHash('sha256').update(JSON.stringify(d.initial)).digest('hex');
  for(const e of d.events){e.redex.source=id(e.redex.source);e.redex.revision=id(e.redex.revision);e.redex.binding=e.redex.binding.map(([a,b])=>[id(a),id(b)]);e.fresh=atoms(e.fresh);for(const key of Object.keys(e.footprint))e.footprint[key]=e.footprint[key].map(id);}
  const replies=compare([{command:'import',trace:JSON.stringify(d)},{command:'jump',state:'0'},{command:'jump',state:'1'},{command:'export'}]);assert.equal(replies[0].ok,true);assert.ok(Object.keys(replies[0].state.graph.atoms).every(k=>BigInt(k)>=offset));assert.equal(replies[2].state.graph.next_identity,id('21'));
});
