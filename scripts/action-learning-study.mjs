#!/usr/bin/env node
// Current-source acceptance, not a frozen historical timing benchmark.
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';

const args=process.argv.slice(2);
if(args.length!==2 && args.length!==4 || args[0]!=='--output' || (args.length===4 && args[2]!=='--lean')) {
  throw new Error('usage: node scripts/action-learning-study.mjs --output NEW_FILE [--lean LEAN_4_19_BINARY]');
}
const commands=[];
function run(command, argv) {
  const stdout=execFileSync(command,argv,{encoding:'utf8',timeout:180000,maxBuffer:16*1024*1024});
  commands.push({command,arguments:argv,stdout}); return stdout;
}
run('rustc',['--version']);
run('cargo',['--version']);
const tests=run('cargo',['test','--locked','-p','engine','--test','action_learning','--test','observation_certificates','-p','cli','--test','action_learning','--','--nocapture']);
for(const marker of ['ACTION_EVIDENCE prefixes=7776 classifications=31104 safe_predictions=9000',
  'ACTION_NATIVE executions=324 admitted_steps=144','OBSERVER_EVIDENCE configurations=32768']) {
  if(!tests.includes(marker))throw new Error(`missing evidence: ${marker}`);
}
const proofFiles=['proofs/action_learning/Safety.lean','proofs/observation/Certificates.lean'];
if(args.length===4) {
  if(!run(resolve(args[3]),['--version']).includes('version 4.19.0')) throw new Error('Lean 4.19.0 required');
  for(const file of proofFiles)run(resolve(args[3]),[file]);
}
const files=['Cargo.lock','rust-toolchain.toml','crates/engine/src/action_learning.rs',
  'crates/engine/src/action_learning/carrier.rs','crates/engine/tests/action_learning.rs',
  'crates/engine/tests/observation_certificates.rs','crates/cli/tests/action_learning.rs',
  'crates/cli/src/commands.rs','examples/action-learning/doors.json','scripts/action-learning-study.mjs',...proofFiles];
const sha256=Object.fromEntries(files.map(file=>[file,createHash('sha256').update(readFileSync(file)).digest('hex')]));
const report={schema:'action-learning-acceptance-v1',scope:'Current-source finite correspondence and abstract proof checks; no timing, native refinement or external replication claim.',
  environment:{platform:process.platform,architecture:process.arch,node:process.version},
  source_sha256:sha256,proofs_checked:args.length===4,commands};
writeFileSync(args[1],JSON.stringify(report,null,2)+'\n',{flag:'wx'});
console.log(`Wrote ${args[1]}`);
