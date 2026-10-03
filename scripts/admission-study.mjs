// Frozen proof/model/native conformance evidence. This is not a Rust verification claim.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {spawnSync,execFileSync} from 'node:child_process';
import {gzipSync,gunzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {abstractInputs,countermodels,checkAbstract,nativeInputs,checkNative,boundaryInputs,checkBoundary} from './admission-model.mjs';
import {checkProof} from './check-admission-proof.mjs';
import {sourceFiles} from './research-source-files.mjs';
const [mode,dir,...flags]=process.argv.slice(2);
assert.ok(['collect','verify'].includes(mode)&&dir,'collect|verify DIR [--live] [--proof]');
const hash=b=>createHash('sha256').update(b).digest('hex');
const native='target/release/examples/admission_audit';
function batch(binary,args,rows,env=process.env) {
  const result=spawnSync(binary,args,{input:rows.map(r=>JSON.stringify(r)).join('\n')+'\n',encoding:'utf8',env,timeout:180000,maxBuffer:128*1024*1024});
  assert.equal(result.status,0,result.error?.message??result.stderr);
  const output=result.stdout.trim().split('\n').map(s=>JSON.parse(s));assert.equal(output.length,rows.length);return output;
}
function campaign(proof) {
  const abstract=abstractInputs(),nativeQueries=nativeInputs();
  const leanRows=batch(proof.lean,['--run','crates/engine/proofs/AdmissionRunner.lean'],[...abstract,...countermodels],proof.env);
  const nativeRows=batch(native,[],[...nativeQueries,...boundaryInputs]);
  return {abstract:leanRows.slice(0,abstract.length),countermodels:leanRows.slice(abstract.length),native:nativeRows.slice(0,nativeQueries.length),boundaries:nativeRows.slice(nativeQueries.length)};
}
function verify(rows) {
  const inputs=abstractInputs(),queries=nativeInputs();
  assert.equal(rows.abstract.length,inputs.length);assert.equal(rows.native.length,queries.length);assert.equal(rows.countermodels.length,countermodels.length);assert.equal(rows.boundaries.length,boundaryInputs.length);
  let closureAccepted=0;
  rows.abstract.forEach((r,i)=>{if(checkAbstract(inputs[i],r).accepted)closureAccepted++;});
  rows.countermodels.forEach((r,i)=>{
    const c=countermodels[i];checkAbstract(c,r,false);
    for(const [field,wanted] of Object.entries({closed:c.closed,fresh:c.fresh_ok,accepted:c.accepted,post_closed:c.post_closed})) assert.equal(r[field],wanted,c.name);
  });
  const counts={candidates:0,admitted:0,rejected:0};
  rows.native.forEach((r,i)=>{assert.deepEqual(r.input,queries[i]);const c=checkNative(r);for(const k of Object.keys(counts))counts[k]+=c[k];});
  rows.boundaries.forEach(checkBoundary);
  return {abstract_cases:inputs.length,closure_accepted:closureAccepted,countermodels:countermodels.length,native_queries:queries.length,...counts,boundary_queries:rows.boundaries.length};
}
if(mode==='collect') {
  assert.ok(!existsSync(dir),'collections are immutable; choose a new path');
  const proof=checkProof();const start=performance.now();const rows=campaign(proof);const counts=verify(rows);
  const paths=['Cargo.lock','rust-toolchain.toml','crates/engine/examples/admission_audit.rs','crates/engine/tests/support/mod.rs','crates/engine/proofs/Admission.lean','crates/engine/proofs/AdmissionRunner.lean','scripts/admission-model.mjs','scripts/check-admission-proof.mjs','scripts/admission-study.mjs','scripts/admission.test.mjs'];
  paths.push('scripts/research-source-files.mjs',...sourceFiles(['crates/engine/src','crates/graph-core/src']));
  const sources=Buffer.from(JSON.stringify(Object.fromEntries(paths.sort().map(p=>[p,readFileSync(p,'utf8')]))));
  const data=Buffer.from(JSON.stringify(rows));
  const report={schema:'admission-study-v1',date:new Date().toISOString(),revision:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),counts,proof:proof.evidence,source_sha256:hash(sources),data_sha256:hash(data),binary_sha256:hash(readFileSync(native)),compiler:execFileSync('rustc',['--version'],{encoding:'utf8'}).trim(),hardware:{platform:os.platform(),release:os.release(),arch:os.arch(),cpu:os.cpus()[0].model,memory_bytes:os.totalmem()},campaign_wall_ms:performance.now()-start};
  mkdirSync(dir,{recursive:true});writeFileSync(`${dir}/sources.json.gz`,gzipSync(sources));writeFileSync(`${dir}/outcomes.json.gz`,gzipSync(data));writeFileSync(`${dir}/report.json`,JSON.stringify(report,null,2)+'\n');
}
const report=JSON.parse(readFileSync(`${dir}/report.json`));assert.equal(report.schema,'admission-study-v1');
const sources=gunzipSync(readFileSync(`${dir}/sources.json.gz`)),data=gunzipSync(readFileSync(`${dir}/outcomes.json.gz`));
assert.equal(hash(sources),report.source_sha256);assert.equal(hash(data),report.data_sha256);assert.deepEqual(verify(JSON.parse(data)),report.counts);
if(flags.includes('--live')) {
  const rows=JSON.parse(data);const current=batch(native,[],[...nativeInputs(),...boundaryInputs]);
  assert.deepEqual(current,[...rows.native,...rows.boundaries]);
}
if(flags.includes('--proof')) {
  const proof=checkProof();assert.deepEqual(proof.evidence.axioms,report.proof.axioms);
  const current=campaign(proof);assert.deepEqual(verify(current),report.counts);assert.deepEqual(current,JSON.parse(data));
}
console.log(JSON.stringify({verified:report.counts,source_sha256:report.source_sha256}));
