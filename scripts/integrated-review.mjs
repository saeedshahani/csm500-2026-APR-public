// Functional reassessment receipts; durations are not benchmark measurements.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {execFileSync, spawnSync} from 'node:child_process';
import {mkdirSync, readFileSync, writeFileSync, readdirSync} from 'node:fs';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {platform, arch} from 'node:os';

const hash = value => createHash('sha256').update(value).digest('hex');
const node = (...args) => ['node', ...args];
const tests = directory => readdirSync(directory).filter(p => p.endsWith('.test.mjs')).sort().map(p => `${directory}/${p}`);
export function plans() {
  return {
    core: [
      ['cargo','fmt','--all','--','--check'],
      ['cargo','clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings'],
      ['cargo','test','--workspace','--locked','--offline'],
      ['cargo','doc','--workspace','--no-deps','--locked','--offline'],
      ['cargo','build','--release','--locked','--offline','-p','cli','-p','engine','-p','wasm-bridge','--examples'],
      ['cargo','build','--release','--locked','--offline','-p','cli'],
      ['cargo','build','--locked','--offline','-p','cli','-p','wasm-bridge','--examples'],
      ['cargo','build','--locked','--offline','-p','cli'],
      node('scripts/verify-examples.mjs'),
    ],
    research: [
      node('--test', ...tests('scripts')),
      ...['b','b2','c','d','e','f','f2'].map(loop => node(`scripts/verify-loop-${loop}.mjs`)),
      node('scripts/workflow-study.mjs','verify','benchmarks/results/workflow-contracts-01','--live'),
      node('scripts/intervention-study.mjs','verify','benchmarks/results/interventions-03','--live'),
      node('scripts/minimality-study.mjs','verify','benchmarks/results/minimality-03','--live'),
      node('scripts/lambda-open-study.mjs','verify','benchmarks/results/lambda-open-01','--live'),
      node('scripts/admission-study.mjs','verify','benchmarks/results/admission-01','--live'),
      node('scripts/learning-study.mjs','verify','benchmarks/results/learning-02','--live'),
      node('scripts/meaning-study.mjs','verify','benchmarks/results/meaning-01','--live'),
      node('scripts/verify-reproduction.mjs','--check','benchmarks/results/reproduction-m/report.json'),
      node('scripts/verify-benchmark.mjs'),
      node('scripts/verify-scale-study.mjs'),
      node('scripts/verify-recovery-study.mjs'),
    ],
    platform: [node('--test', ...tests('platform/tests')), ['npm','run','test:emulator','--prefix','platform']],
    web: ['build','lint','test:unit','test:parity','test:browser'].map(task => ['npm','--prefix','frontend','run',task]),
    proof: [node('scripts/admission-study.mjs','verify','benchmarks/results/admission-01','--proof'),
      node('scripts/check-minimality-proof.mjs'),
      ['lean','crates/engine/proofs/WorkflowContracts.lean']],
  };
}

export function sourceInventory() {
  const paths = execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n');
  return Object.fromEntries([...new Set(paths)].filter(p =>
    /^(Cargo\.(toml|lock)|rust-toolchain\.toml|rustfmt\.toml|firebase(?:\.hosted)?\.json|firestore\.(rules|indexes\.json)|storage\.rules|\.firebaserc|\.dockerignore|\.gitignore)$/.test(p) ||
    p==='docs/guides/workbench.md' ||
    /^(crates|examples|scripts|platform|frontend|\.github)\//.test(p)
  ).filter(p => !p.endsWith('/.DS_Store')).sort().map(p => [p,hash(readFileSync(p))]));
}

export function validateReceipt(receipt, expected, read, current) {
  assert.equal(receipt.schema,'integrated-review-v1');
  assert.equal(receipt.complete,true,'incomplete acceptance is not a pass');
  assert.deepEqual(receipt.commands,expected,'receipt must contain the entire declared group');
  assert.equal(receipt.results.length,expected.length,'missing command result');
  assert.ok(Object.keys(receipt.sources).length > 0);
  assert.equal(receipt.source_sha256,hash(JSON.stringify(receipt.sources)));
  if (current) assert.deepEqual(receipt.sources,current,'candidate sources changed');
  for (const [i,row] of receipt.results.entries()) {
    assert.equal(row.index,i); assert.equal(row.exit_code,0); assert.equal(row.signal,null); assert.equal(row.error,null);
    assert.equal(row.log,`${i}.log`); // Never follow an arbitrary receipt path.
    assert.equal(hash(read(row.log)),row.sha256,'changed command output');
  }
  return true;
}

export function collect(group,directory) {
  const commands=plans()[group]; assert.ok(commands,`unknown group: ${group}`);
  mkdirSync(directory); // Refuse reuse: failed attempts remain evidence.
  const sources=sourceInventory();
  const receipt={schema:'integrated-review-v1',group,complete:false,started:new Date().toISOString(),
    baseline_commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),
    environment:{platform:platform(),arch:arch(),node:process.version},
    sources,source_sha256:hash(JSON.stringify(sources)),commands,results:[]};
  const save=()=>writeFileSync(join(directory,'receipt.json'),JSON.stringify(receipt,null,2)+'\n');
  save();
  for (const [index,[command,...args]] of commands.entries()) {
    console.log(`[${group} ${index+1}/${commands.length}] ${command} ${args.join(' ')}`);
    const started=Date.now();
    const result=spawnSync(command,args,{encoding:'utf8',timeout:900000,maxBuffer:64*1024*1024,
      env:{...process.env,RUSTDOCFLAGS:'-D warnings',CI:'true',PW_TEST_PORT:process.env.PW_TEST_PORT??'4181'}});
    const log=`${index}.log`,bytes=Buffer.from((result.stdout??'')+'\n[stderr]\n'+(result.stderr??''));
    writeFileSync(join(directory,log),bytes);
    receipt.results.push({index,exit_code:result.status,signal:result.signal,error:result.error?.message??null,
      elapsed_ms:Date.now()-started,log,sha256:hash(bytes)});
    save();
    assert.ok(result.status===0&&!result.signal&&!result.error,`Failed ${group}/${index}; retained ${directory}/${log}`);
  }
  assert.deepEqual(sourceInventory(),sources,'candidate changed during acceptance');
  receipt.complete=true;receipt.finished=new Date().toISOString();save();
  console.log(`Passed ${group}: ${commands.length} commands; ${receipt.source_sha256}`);
}

if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href) {
  const [mode,group,directory,...flags]=process.argv.slice(2);
  assert.ok(['collect','verify'].includes(mode)&&plans()[group]&&directory,'collect|verify GROUP NEW_DIRECTORY [--current]');
  assert.ok(flags.every(f=>f==='--current')&&(mode==='verify'||flags.length===0));
  if(mode==='collect') collect(group,directory);
  else {
    const receipt=JSON.parse(readFileSync(join(directory,'receipt.json')));
    assert.equal(receipt.group,group);
    validateReceipt(receipt,plans()[group],p=>readFileSync(join(directory,p)),flags.includes('--current')?sourceInventory():undefined);
    console.log(`Verified ${group}; ${flags.includes('--current')?'current':'recorded'} source identity. Hashes do not authenticate a producer.`);
  }
}
