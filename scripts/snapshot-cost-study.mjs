#!/usr/bin/env node
// Native storage measurements checked against an independent UTF-8 byte oracle.
import {execFileSync} from 'node:child_process';
import {readFileSync, writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import assert from 'node:assert/strict';
import {sourceFiles} from './research-source-files.mjs';

const args=process.argv.slice(2);
if(![4,6].includes(args.length) || args[0]!=='--cli' || args[2]!=='--output' || (args.length===6 && args[4]!=='--lean'))
  throw new Error('usage: node scripts/snapshot-cost-study.mjs --cli CLI --output NEW_JSON [--lean LEAN_4_19]');
const cli=resolve(args[1]);
const sha=x=>createHash('sha256').update(x).digest('hex');
function run(argv,input) {
  return execFileSync(cli,argv,{input,encoding:'utf8',timeout:60000,maxBuffer:16*1024*1024});
}
const rows=[];
function check(name,source) {
  const traceText=run(['export','-','--max-steps','0'],source);
  const trace=JSON.parse(traceText);
  const result=JSON.parse(run(['trace-cost','-','--at','0'],traceText));
  const graph=JSON.stringify(trace.initial);
  const envelope=`{"schema":"identity-snapshot-json-v1","graph":${graph}}`;
  assert.equal(result.analysis.payload_bytes,String(Buffer.byteLength(graph)));
  assert.equal(result.analysis.total_bytes,String(Buffer.byteLength(envelope)));
  assert.equal(result.analysis.total_bits,String(8*Buffer.byteLength(envelope)));
  assert.equal(result.analysis.snapshot_sha256,sha(envelope));
  assert.equal(result.source_trace_sha256,sha(traceText));
  rows.push({name,source_sha256:sha(source),analysis:result.analysis});
}
for(const count of [1,8,64,512,2048]) {
  for(const label of ['x','x'.repeat(64),'λ🦀"\\\n']) {
    for(const topology of ['isolated','ring','shared']) {
      const lines=['program {'];
      for(let i=0;i<count;i++)lines.push(`atom n${i} = Constructor(${JSON.stringify(label)});`);
      if(topology!=='isolated')for(let i=0;i<count;i++)
        lines.push(`atom e${i} = Record[ObjectLink]("next", n${i}, n${topology==='ring'?(i+1)%count:0});`);
      lines.push('}');
      check(`${count}/${Buffer.byteLength(label)}/${topology}`,lines.join('\n'));
    }
  }
}
check('unowned-cycles','program { atom a = Nat(b); atom b = Nat(a); atom c = Nat(c); }');
check('empty','program {}');
// Exporting active programs with zero fuel reports StepLimit (exit 3), so the
// study's structural cases deliberately have no executable rules. Native tests
// separately cover rule metadata and all recorded states of a two-step trace.
const short=rows.find(r=>r.name==='2048/1/ring').analysis;
const long=rows.find(r=>r.name==='2048/64/ring').analysis;
assert.equal(short.atoms,long.atoms);
assert.equal(short.fields,long.fields);
assert.equal(Number(long.total_bytes)-Number(short.total_bytes),2048*63);
const files=[...sourceFiles(['crates/engine/src/trace']),
  'crates/engine/tests/snapshot_cost.rs','crates/cli/tests/trace_cost.rs','crates/cli/src/commands.rs',
  'crates/engine/proofs/RepresentationCosts.lean','scripts/snapshot-cost-study.mjs',
  'Cargo.lock','rust-toolchain.toml'];
let proof={checked:false};
if(args.length===6) {
  const lean=resolve(args[5]);
  const version=execFileSync(lean,['--version'],{encoding:'utf8'}).trim();
  assert.ok(version.includes('version 4.19.0'));
  const path='crates/engine/proofs/RepresentationCosts.lean';
  const source=readFileSync(path,'utf8');
  assert.doesNotMatch(source,/\bsorry\b|^axiom /m);
  const names=[...source.matchAll(/^theorem (\w+)/gm)].map(m=>m[1]);
  assert.equal(names.length,14);
  const axioms=execFileSync(lean,[path],{encoding:'utf8',timeout:60000});
  for(const name of names)assert.ok(axioms.includes(`'RepresentationCosts.${name}'`));
  assert.ok(!axioms.includes('sorryAx'));
  proof={checked:true,version,theorems:names,axioms,scope:'Conditional abstract lemmas; no executed Rust refinement or unrestricted enumerator proof.'};
}
const report={schema:'snapshot-cost-study-v1',
  scope:'47 complete native initial-state measurements; independent compact-JSON UTF-8 accounting. Storage only: evaluator, decoder, history and work are excluded. No optimality, timing or throughput claim.',
  environment:{platform:process.platform,architecture:process.arch,node:process.version},
  binary_sha256:sha(readFileSync(cli)),source_sha256:Object.fromEntries(files.map(f=>[f,sha(readFileSync(f))])),
  counts:{cases:rows.length,byte_oracle_agreements:rows.length},proof,
  counterexample:{atoms:short.atoms,fields:short.fields,short_bytes:short.total_bytes,long_bytes:long.total_bytes,extra_bytes:String(2048*63)},rows};
assert.equal(rows.length,47);
writeFileSync(args[3],JSON.stringify(report,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({counts:report.counts,counterexample:report.counterexample}));
