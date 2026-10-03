// Follow-up to the retained cross-host WASM byte mismatch. No timing claims.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {gzipSync,gunzipSync} from 'node:zlib';
import {verify,safePath,sha,PROTOCOL} from './reproducibility.mjs';
import {wasmPanel} from './reproduction-panel.mjs';

const [source,out]=process.argv.slice(2);assert.ok(source&&out);verify(source);
assert.ok(!fs.existsSync(out));fs.mkdirSync(out,{recursive:true});
const original=JSON.parse(fs.readFileSync(path.join(source,'manifest.json')));
const bundle=JSON.parse(gunzipSync(fs.readFileSync(path.join(source,'sources.json.gz'))));
const expected=JSON.parse(gunzipSync(fs.readFileSync(path.join(source,'remap-a-panel.json.gz'))));
const temp=fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(),'reflective-wasm-diff-')));
const run=(command,args,env=process.env,cwd=temp)=>{
  const p=spawnSync(command,args,{cwd,env,encoding:'utf8',timeout:600000,maxBuffer:64*1024*1024});
  assert.ok(!p.error&&!p.signal,p.error?.message??p.signal);assert.equal(p.status,0,p.stdout+p.stderr);return p;
};
function sections(bytes) {
  assert.equal(bytes.subarray(0,8).toString('hex'),'0061736d01000000');let at=8;
  const leb=()=>{let n=0,shift=0,b;do{assert.ok(at<bytes.length&&shift<=28);b=bytes[at++];n+=(b&127)*2**shift;shift+=7;}while(b&128);return n;};
  const result=[];while(at<bytes.length){const id=bytes[at++],length=leb(),start=at,end=at+length;assert.ok(end<=bytes.length);let name=null;
    if(id===0){const size=leb();assert.ok(at+size<=end);name=bytes.subarray(at,at+size).toString();}
    result.push({id,name,length,sha256:sha(bytes.subarray(start,end))});at=end;
  }return result;
}
const binder=process.env.REPRO_BINDGEN??path.resolve('target/tools/bin/wasm-bindgen');
const cargo=process.env.CARGO_HOME??path.join(os.homedir(),'.cargo');
const sysroot=run('rustc',['--print','sysroot']).stdout.trim();
const report={schema:'wasm-build-diagnostic-v1',complete:false,environment:{platform:process.platform,arch:process.arch,rust:run('rustc',['-Vv']).stdout,bindgen:run(binder,['--version']).stdout,node:process.version,sysroot,github_run:process.env.GITHUB_RUN_ID??null},source_digest:sha(JSON.stringify(original.sources)),results:[],failure:null};
const save=()=>fs.writeFileSync(path.join(out,'report.json'),JSON.stringify(report,null,2)+'\n');save();
try {
  for(const variant of ['source-cargo','source-cargo-sysroot']){
    const cwd=path.join(temp,variant);fs.mkdirSync(cwd);
    for(const[p,b]of Object.entries(bundle)){safePath(p);const dest=path.join(cwd,p);fs.mkdirSync(path.dirname(dest),{recursive:true});fs.writeFileSync(dest,Buffer.from(b,'base64'),{flag:'wx'});}
    const env={...process.env};for(const k of Object.keys(env))if(k==='RUSTFLAGS'||k==='CARGO_ENCODED_RUSTFLAGS'||k.startsWith('CARGO_PROFILE_')||k.startsWith('CARGO_BUILD_')||k.startsWith('RUSTC_'))delete env[k];
    const flags=[`--remap-path-prefix=${cwd}=/source`,`--remap-path-prefix=${cargo}=/cargo`,...(variant.endsWith('sysroot')?[`--remap-path-prefix=${sysroot}=/toolchain`]:[])];
    Object.assign(env,{CARGO_TARGET_DIR:path.join(cwd,'target'),CARGO_INCREMENTAL:'0',TZ:'UTC',LC_ALL:'C',SOURCE_DATE_EPOCH:PROTOCOL.epoch,CARGO_ENCODED_RUSTFLAGS:flags.join('\x1f')});
    const build=run('cargo',['build','--release','--locked','--offline','-p','wasm-bridge','--target','wasm32-unknown-unknown'],env,cwd);
    fs.writeFileSync(path.join(out,`${variant}-build.log`),build.stdout+build.stderr);
    const rawPath=path.join(cwd,'target/wasm32-unknown-unknown/release/wasm_bridge.wasm'),pkg=path.join(cwd,'pkg-node');
    run(binder,[rawPath,'--target','nodejs','--out-dir',pkg]);
    const raw=fs.readFileSync(rawPath),bound=fs.readFileSync(path.join(pkg,'wasm_bridge_bg.wasm')),js=fs.readFileSync(path.join(pkg,'wasm_bridge.js'));
    const result={variant,flags,raw_sha256:sha(raw),bound_sha256:sha(bound),binding_sha256:sha(js),raw_sections:sections(raw),bound_sections:sections(bound),semantic:wasmPanel(path.join(pkg,'wasm_bridge.js'),expected)};
    for(const[name,bytes]of [['raw.wasm',raw],['bound.wasm',bound],['binding.js',js]])fs.writeFileSync(path.join(out,`${variant}-${name}.gz`),gzipSync(bytes));
    report.results.push(result);save();
  }
  report.complete=true;save();
}catch(e){report.failure=String(e.stack);save();throw e;}
finally{fs.rmSync(temp,{recursive:true,force:true});}
console.log(JSON.stringify({complete:report.complete,results:report.results.map(r=>({variant:r.variant,raw:r.raw_sha256,bound:r.bound_sha256,cases:r.semantic.length}))},null,2));
