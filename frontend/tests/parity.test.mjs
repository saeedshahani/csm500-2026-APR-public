import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {verifyExamples} from '../../scripts/verify-examples.mjs';
const require=createRequire(import.meta.url);const {WorkbenchHandle}=require('../pkg-node/wasm_bridge.js');
const root=new URL('../../',import.meta.url);
test('all source ranges and related locations agree across native and WASM without changing history',()=>{
 const sources=['program {\r\n atom a = Constructor("λ 🐈");\r atom a = Constructor("duplicate");\n}', 'program { atom x = Nat(missing); atom y = Nat(other); }', 'program { atom x = Constructor("猫");'];
 const replies=compare([{command:'load',source:'program {}'},...sources.flatMap(source=>[{command:'validate',source},{command:'load',source},{command:'state'}])]);
 for(let i=1;i<replies.length;i+=3){assert.equal(replies[i].ok,false);assert.ok(replies[i].error.diagnostics.length);assert.deepEqual(replies[i].error,replies[i+1].error);assert.deepEqual(replies[i+2].state,replies[0].state);}
 assert.ok(replies[1].error.diagnostics.some(e=>e.related.length));assert.equal(replies[4].error.diagnostics.length,2);
});
test('lambda translation, interpretation, history and failures share native/WASM semantics',()=>{
 const input='((\\x. x) (\\y. y))';
 const preview=compare([{command:'lambda_compile',source:input}])[0];assert.equal(preview.ok,true);
 const src=preview.compiled;
 const replies=compare([{command:'load',source:src,config:{max_steps:'20'}},{command:'advance',batch:'20'},{command:'jump',state:'0'},{command:'jump',state:'2'},{command:'lambda_compile',source:'(\\x. y)'},{command:'lambda_compile',source:input,max_work:'0'},{command:'state'}]);
 assert.equal(replies[1].state.status,'NormalForm');assert.equal(replies[1].state.events.length,3);assert.equal(replies[1].state.lambda.term,'(λ.0)');
 assert.equal(replies[2].state.lambda.term,'((λ.0) (λ.0))');assert.equal(replies[3].state.lambda.term,'(λ.0)');
 assert.equal(replies[4].ok,false);assert.equal(replies[5].error.kind,'ResourceExhaustion');assert.deepEqual(replies[6],replies[3]);
 const cli=execFileSync('cargo',['run','--quiet','--locked','-p','cli','--','lambda-compile','-'],{cwd:root,input:input,encoding:'utf8'});assert.equal(cli.trim(),src.trim());
 const trace=compare([{command:'import',trace:replies[1].trace},{command:'jump',state:'1'}]);assert.equal(trace[1].state.lambda.term,'((λ.0) (λ.0))');
});
const source=name=>readFileSync(new URL(`examples/${name}`,root),'utf8');
execFileSync('cargo',['build','-p','wasm-bridge','--example','protocol','--locked'],{cwd:root,stdio:'inherit'});
execFileSync('cargo',['build','-p','cli','--locked'],{cwd:root,stdio:'inherit'});
test('complete catalogue has exact CLI/native/WASM execution, trace and historical parity',()=>{
 assert.equal(verifyExamples(compare),22);
});
function compare(commands){const native=execFileSync(fileURLToPath(new URL('target/debug/examples/protocol',root)),[],{input:commands.map(c=>JSON.stringify(c)).join('\n')+'\n',encoding:'utf8',maxBuffer:64*1024*1024}).trim().split('\n').map(JSON.parse);const handle=new WorkbenchHandle();try{const wasm=commands.map(c=>handle.dispatch(JSON.stringify(c)));assert.deepEqual(wasm,native);return wasm;}finally{handle.free();}}
test('breakpoint pause, revision, resume and reset agree exactly across native and WASM',()=>{
 for(const file of ['behaviour/oscillating.graph','programs/self-modify.graph','practical/policy-enable.graph']){
  const load={command:'load',source:source(file),config:{max_steps:'4',seed:'42',detect_cycles:true}};
  const initial=compare([load])[0],heads=initial.state.rules.map(r=>r.head);
  const replies=compare([load,{command:'breakpoints',heads},...Array.from({length:10},()=>({command:'advance',batch:'1'})),{command:'export'},{command:'reset'}]);
  assert.equal(replies[2].state.status,'Breakpoint');assert.equal(replies[2].trace,initial.trace);
  const expected=compare([load,{command:'advance',batch:'100'}])[1];assert.equal(replies.at(-2).trace,expected.trace);
  assert.deepEqual(replies.at(-1).state.breakpoints,[]);
 }
});
for(const [policy,file,events] of [['fixed','work-items',10],['guarded_enable','policy-enable',11]])test(`practical ${policy} CLI/WASM and project parity`,()=>{
 const preview=compare([{command:'generate_dataset',count:'10',seed:'42',policy}])[0];assert.equal(preview.ok,true);assert.equal(preview.dataset.source,source(`practical/${file}.graph`));
 const src=preview.dataset.source;const commands=[{command:'load',source:src,config:{max_steps:'20'}},{command:'advance',batch:'20'},{command:'inspect',query:'WorkItem:',offset:'5',limit:'5'}];const replies=compare(commands);assert.equal(replies[1].state.status,'NormalForm');assert.equal(replies[1].state.events.length,events);
 const cli=execFileSync('cargo',['run','--quiet','--locked','-p','cli','--','export','-','--max-steps','20'],{cwd:root,input:src,encoding:'utf8'});assert.deepEqual(JSON.parse(replies[1].trace),JSON.parse(cli));
 const project={command:'project',source:src,trace:replies[1].trace,execution:replies[1].state.settings,position:'1'};const result=compare([project,{...project,source:'program {}'},{command:'state'}]);assert.equal(result[0].ok,true);assert.equal(result[1].ok,false);assert.deepEqual(result[2],result[0]);
});
test('larger practical dataset reports a bounded failure without invented events',()=>{
 const preview=compare([{command:'generate_dataset',count:'1000',seed:'42',policy:'fixed'}])[0];const result=compare([{command:'load',source:preview.dataset.source},{command:'advance',batch:'1'}]);assert.equal(result[1].state.status,'ResourceLimit');assert.equal(result[1].state.events.length,0);
});
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
test('read-only rule, historical and local analysis has exact native/WASM parity',()=>{
  const results=compare([{command:'load',source:source('independent/shared-read.graph')},{command:'analyse'},{command:'analyse',max_work:'0'},{command:'advance',batch:'2'},{command:'analyse',event:'0'},{command:'jump',state:'1'},{command:'analyse'},{command:'state'}]);
  assert.equal(results[1].analysis.local.pairs[0].commutes,true);assert.equal(results[2].error.kind,'ResourceExhaustion');assert.equal(results.at(-1).state.position,'1');assert.equal(results[6].analysis.local.alternatives[0].state,'1');
});
