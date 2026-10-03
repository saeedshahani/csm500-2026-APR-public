// Fresh-build research harness. Network provisioning is separate from offline builds.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {execFileSync,spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {gzipSync,gunzipSync} from 'node:zlib';
import {pathToFileURL} from 'node:url';
import {sourceFiles} from './research-source-files.mjs';
import {nativePanel,wasmPanel,call} from './reproduction-panel.mjs';
export const sha=b=>createHash('sha256').update(b).digest('hex');
export const PROTOCOL={schema:'cross-environment-reproduction-v1',rust:'1.85.0',epoch:'1770000000',variants:['plain-a','plain-b','remap-a','remap-b'],warmups:1,repetitions:5,time_relative:0.25,time_absolute_ns:500000,rss_relative:0.25,rss_absolute_bytes:4194304,comparison:'exact semantic JSON; no ignored trace fields',performance:[['lambda-closed','lambda_open_probe',['closed-cbn-v1','4']],['lambda-open','lambda_open_probe',['open-cbn-v1','4']],['framed-1000','scale_probe',['fixed','1000','pipeline-framed','1000','42']]]};
export function safePath(p){assert.ok(typeof p==='string'&&p.length>0&&!path.isAbsolute(p)&&!p.includes('\\')&&p.split('/').every(x=>x&&x!=='.'&&x!=='..'),'Unsafe bundle path');return p;}
export function median(xs){assert.ok(xs.length>0&&xs.every(Number.isFinite));const s=[...xs].sort((a,b)=>a-b),n=s.length;return n%2?s[(n-1)/2]:(s[n/2-1]+s[n/2])/2;}
export function agreement(a,b,relative,absolute){const x=median(a),y=median(b);return {a:x,b:y,difference:Math.abs(x-y),allowance:Math.max(relative*Math.min(x,y),absolute),accepted:Math.abs(x-y)<=Math.max(relative*Math.min(x,y),absolute)};}
export function binaryInfo(bytes,root='',cargo='') {
  const info={sha256:sha(bytes),size:bytes.length,source_path_present:!!root&&bytes.includes(Buffer.from(root)),dependency_path_present:!!cargo&&bytes.includes(Buffer.from(cargo)),format:'other',sections:{}};
  if(bytes.length>=32&&bytes.readUInt32LE(0)===0xfeedfacf){info.format='Mach-O-64';let at=32;
    for(let i=0;i<bytes.readUInt32LE(16);i++){assert.ok(at+8<=bytes.length);const command=bytes.readUInt32LE(at),size=bytes.readUInt32LE(at+4);assert.ok(size>=8&&at+size<=bytes.length);
      if(command===0x1b)info.uuid=bytes.subarray(at+8,at+24).toString('hex');
      if(command===0x19){for(let j=0;j<bytes.readUInt32LE(at+64);j++){const s=at+72+j*80;assert.ok(s+80<=at+size);const name=bytes.subarray(s,s+16).toString().split('\0')[0],seg=bytes.subarray(s+16,s+32).toString().split('\0')[0],offset=bytes.readUInt32LE(s+48),length=Number(bytes.readBigUInt64LE(s+40));
        // Zero-fill sections have no file bytes; record their declared size only.
        info.sections[`${seg},${name}`]={size:length,sha256:offset&&offset+length<=bytes.length?sha(bytes.subarray(offset,offset+length)):null};}}
      at+=size;
    }
  }else if(bytes.subarray(0,4).equals(Buffer.from([0x7f,69,76,70])))info.format='ELF';
  return info;
}
function execute(command,args,options={}) {
  const p=spawnSync(command,args,{encoding:'utf8',timeout:600000,maxBuffer:64*1024*1024,...options});
  if(p.error||p.signal)throw Error(`${command}: ${p.error?.message??p.signal}`);
  assert.equal(p.status,0,`${command} ${args.join(' ')}\n${p.stdout}\n${p.stderr}`);return p;
}
function files() {
  const native=sourceFiles(['crates','examples']).filter(p=>/\.(rs|toml|txt|atoms|graph|json|lambda|observer|lean|md)$/.test(p));
  return [...new Set(['Cargo.toml','Cargo.lock','rust-toolchain.toml','scripts/reproducibility.mjs','scripts/reproduction-panel.mjs','scripts/reproducibility.test.mjs','scripts/research-source-files.mjs','scripts/learning-corpus.mjs','scripts/meaning-corpus.mjs','scripts/minimality-codecs.mjs','platform/learning.mjs','platform/meaning.mjs','docs/research/evaluation.md',...native])].sort();
}
function snapshot(){return Object.fromEntries(files().map(p=>[p,fs.readFileSync(p).toString('base64')]));}
function unpack(bundle,dir){for(const[p,b]of Object.entries(bundle)){safePath(p);const dest=path.join(dir,p);fs.mkdirSync(path.dirname(dest),{recursive:true});fs.writeFileSync(dest,Buffer.from(b,'base64'),{flag:'wx'});}}
function environment(){return {platform:process.platform,release:os.release(),arch:process.arch,cpu:os.cpus()[0]?.model,memory_bytes:os.totalmem(),node:process.version,rust:execute('rustc',['-Vv']).stdout,cargo:execute('cargo',['-V']).stdout,linker:execute('cc',['--version']).stdout,sdk:process.platform==='darwin'?execute('xcrun',['--show-sdk-version']).stdout:null,runner_image:process.env.ImageVersion??null,runner_os:process.env.RUNNER_OS??null,github_run:process.env.GITHUB_RUN_ID??null,github_attempt:process.env.GITHUB_RUN_ATTEMPT??null};}
function buildEnv(root,remap) {
  const env={...process.env};for(const k of Object.keys(env))if(k==='RUSTFLAGS'||k==='CARGO_ENCODED_RUSTFLAGS'||k.startsWith('CARGO_PROFILE_')||k.startsWith('CARGO_BUILD_')||k.startsWith('RUSTC_'))delete env[k];
  const cargo=process.env.CARGO_HOME??path.join(os.homedir(),'.cargo');
  Object.assign(env,{CARGO_TARGET_DIR:path.join(root,'target'),CARGO_INCREMENTAL:'0',TZ:'UTC',LC_ALL:'C',SOURCE_DATE_EPOCH:PROTOCOL.epoch});
  if(remap)env.CARGO_ENCODED_RUSTFLAGS=[`--remap-path-prefix=${root}=/source`,`--remap-path-prefix=${cargo}=/cargo`].join('\x1f');
  return {env,cargo};
}
function performanceRun(root,cell){const[id,exe,args]=cell,binary=path.join(root,'target/release/examples',exe),p=execute('/usr/bin/time',[process.platform==='darwin'?'-l':'-v',binary,...args]);
  const value=JSON.parse(p.stdout),match=process.platform==='darwin'?p.stderr.match(/(\d+)\s+maximum resident set size/):p.stderr.match(/Maximum resident set size \(kbytes\): (\d+)/);assert.ok(match,'Missing RSS observation');
  const rss=Number(match[1])*(process.platform==='darwin'?1:1024);assert.ok(rss>0);
  if(id==='framed-1000'){assert.equal(value.outcome,'NormalForm');assert.equal(value.commits,1000);assert.equal(value.domain_verified,true);assert.equal(value.replay_verified,true);}
  else {assert.equal(value.terminal,true);assert.equal(value.beta,4);assert.equal(value.readback,id==='lambda-open'?'database':'(λ.0)');}
  return {cell:id,value,peak_rss_bytes:rss,raw_time:p.stderr};
}
export function measureSummary(rows) {return PROTOCOL.performance.map(([id])=>{
  const a=rows.filter(r=>r.cell===id&&r.build==='remap-a'&&!r.warmup),b=rows.filter(r=>r.cell===id&&r.build==='remap-b'&&!r.warmup);assert.equal(a.length,PROTOCOL.repetitions);assert.equal(b.length,PROTOCOL.repetitions);
  const time=r=>id==='framed-1000'?r.value.phase_ns:r.value.execution_ns;
  return {cell:id,time:agreement(a.map(time),b.map(time),PROTOCOL.time_relative,PROTOCOL.time_absolute_ns),rss:agreement(a.map(r=>r.peak_rss_bytes),b.map(r=>r.peak_rss_bytes),PROTOCOL.rss_relative,PROTOCOL.rss_absolute_bytes)};
});}
export function verify(directory) {
  const m=JSON.parse(fs.readFileSync(path.join(directory,'manifest.json')));assert.equal(m.complete,true);assert.deepEqual(m.protocol,PROTOCOL);
  for(const[p,hash]of Object.entries(m.artifacts))assert.equal(sha(fs.readFileSync(path.join(directory,safePath(p)))),hash,`Changed artifact ${p}`);
  const sources=JSON.parse(gunzipSync(fs.readFileSync(path.join(directory,'sources.json.gz'))));
  assert.deepEqual(Object.keys(sources).sort(),Object.keys(m.sources).sort());for(const[p,b]of Object.entries(sources)){safePath(p);assert.equal(sha(Buffer.from(b,'base64')),m.sources[p]);}
  assert.deepEqual(m.builds.map(b=>b.name),PROTOCOL.variants);
  const panels=m.builds.map(b=>JSON.parse(gunzipSync(fs.readFileSync(path.join(directory,`${b.name}-panel.json.gz`)))));
  for(const p of panels.slice(1))assert.deepEqual(p,panels[0]);
  if(m.historical){assert.equal(panels[0].runs.length,3);assert.equal(m.measurements.length,0);}
  else {assert.equal(panels[0].runs.length,14);assert.equal(panels[0].proposals.calendar.length,768);assert.equal(panels[0].proposals.context.length,144);assert.equal(panels[0].corrupt_trace_rejected,true);
    for(const r of panels[0].runs){const replies=[{ok:true},{ok:true,trace:r.trace,state:{lambda:{term:r.case.id==='open-lambda'?'(λ.x)':null}}}];assert.deepEqual(checkPanelRecord(r.case,replies),{trace:r.trace,observation:r.observation,codecs:r.codecs});}
    assert.equal(m.measurements.length,36);assert.deepEqual(measureSummary(m.measurements),m.performance);
    assert.equal(m.wasm.length,2);for(const w of m.wasm){assert.equal(w.results.length,14);assert.deepEqual(w.results,m.wasm[0].results);for(const row of w.results)assert.equal(row.trace_sha256,sha(JSON.stringify(panels[0].runs.find(r=>r.case.id===row.id).trace)));}
  }
  assert.equal(m.binary_equal.plain,m.builds[0].binary.sha256===m.builds[1].binary.sha256);assert.equal(m.binary_equal.remap,m.builds[2].binary.sha256===m.builds[3].binary.sha256);
  return {verified:true,historical:m.historical,builds:m.builds.length,binary_equal:m.binary_equal,panel_cases:panels[0].runs.length,performance:m.performance,source_digest:sha(JSON.stringify(m.sources))};
}
import {checkReplies as checkPanelRecord} from './reproduction-panel.mjs';

export function compare(directories) {
  assert.ok(directories.length>=2);directories.forEach(verify);const manifests=directories.map(d=>JSON.parse(fs.readFileSync(path.join(d,'manifest.json'))));assert.ok(manifests.every(m=>!m.historical));
  for(const m of manifests.slice(1))assert.deepEqual(m.sources,manifests[0].sources,'Different source inventories');
  const panels=directories.map(d=>JSON.parse(gunzipSync(fs.readFileSync(path.join(d,'remap-a-panel.json.gz')))));
  panels.slice(1).forEach(p=>assert.deepEqual(p,panels[0],'Cross-environment semantic mismatch'));
  return {schema:'reproduction-comparison-v1',semantic_equal:true,proposal_equal:true,environments:manifests.map((m,i)=>({directory:directories[i],environment:m.environment,binary:m.builds[2].binary.sha256,wasm:m.wasm[0].binary_sha256,source_digest:sha(JSON.stringify(m.sources)),performance:m.performance})),wasm_bytes_equal:manifests.every(m=>m.wasm[0].binary_sha256===manifests[0].wasm[0].binary_sha256),native_bytes_equal:manifests.every(m=>m.builds[2].binary.sha256===manifests[0].builds[2].binary.sha256)};
}
async function collect(directory,historical=false) {
  assert.ok(!fs.existsSync(directory),'Never overwrite a collection');fs.mkdirSync(directory,{recursive:true});
  const root=process.cwd(),temporary=fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(),'reflective-loop-m-')));
  const old=historical?JSON.parse(fs.readFileSync('benchmarks/results/s6-cycle01-reproduction/manifest.json')):null;
  const bundle=historical?JSON.parse(gunzipSync(fs.readFileSync('benchmarks/results/s6-cycle01-reproduction/sources.json.gz'))):snapshot();
  const m={protocol:PROTOCOL,complete:false,historical,base_commit:execute('git',['rev-parse','HEAD']).stdout.trim(),environment:environment(),sources:Object.fromEntries(Object.entries(bundle).map(([p,b])=>[p,sha(Buffer.from(b,'base64'))])),artifacts:{},builds:[],wasm:[],measurements:[],performance:[],failure:null};
  if(old)assert.deepEqual(m.sources,old.sources,'Historical source identity');
  const save=(name,bytes)=>{safePath(name);fs.writeFileSync(path.join(directory,name),bytes,{flag:'wx'});m.artifacts[name]=sha(bytes);};
  const persist=()=>fs.writeFileSync(path.join(directory,'manifest.json'),JSON.stringify(m,null,2)+'\n');
  save('sources.json.gz',gzipSync(JSON.stringify(bundle)));persist();const roots={};
  try {
    for(const name of PROTOCOL.variants){console.error(`Building ${name} (${historical?'historical':'current'})`);const cwd=path.join(temporary,name.endsWith('a')?name:`longer-source-directory-${name}`);roots[name]=cwd;fs.mkdirSync(cwd);unpack(bundle,cwd);
      const {env,cargo}=buildEnv(cwd,name.startsWith('remap')),log=[];
      const build=args=>{const p=execute('cargo',['build','--release','--locked','--offline',...args],{cwd,env});log.push(p.stdout+p.stderr);};
      build(['-p','cli','--bin','cli']);const binary=path.join(cwd,'target/release/cli');
      const info=binaryInfo(fs.readFileSync(binary),cwd,cargo);m.builds.push({name,binary:info,remapped:name.startsWith('remap'),epoch:PROTOCOL.epoch});
      let outcome;
      if(historical){outcome={runs:old.results[0].runs.map(r=>{const p=call(binary,['export',`examples/${r.fixture}.graph`,'--max-steps',String(r.steps),...r.extra],undefined,cwd);assert.equal(p.code,r.exit);assert.equal(sha(p.stdout),r.sha256,'Historical exact trace differs');assert.equal(call(binary,['trace-check','-'],p.stdout,cwd).code,0);return {fixture:r.fixture,trace:JSON.parse(p.stdout)};})};}
      else {build(['-p','wasm-bridge','--example','protocol']);outcome=nativePanel(binary,path.join(cwd,'target/release/examples/protocol'),cwd);}
      save(`${name}-panel.json.gz`,gzipSync(JSON.stringify(outcome)));save(`${name}-build.log`,log.join('\n'));
      if(name.startsWith('remap')&&!historical){
        const probe=execute('cargo',['build','--release','--locked','--offline','-p','cli','--example','lambda_open_probe','-p','engine','--example','scale_probe'],{cwd,env});save(`${name}-probes.log`,probe.stdout+probe.stderr);
        const wasm=execute('cargo',['build','--release','--locked','--offline','-p','wasm-bridge','--target','wasm32-unknown-unknown'],{cwd,env});save(`${name}-wasm.log`,wasm.stdout+wasm.stderr);
        const binder=process.env.REPRO_BINDGEN??path.join(root,'target/tools/bin/wasm-bindgen');assert.equal(execute(binder,['--version']).stdout.trim(),'wasm-bindgen 0.2.100');
        const wasmRaw=path.join(cwd,'target/wasm32-unknown-unknown/release/wasm_bridge.wasm'),out=path.join(cwd,'pkg-node');execute(binder,[wasmRaw,'--target','nodejs','--out-dir',out]);
        const results=wasmPanel(path.join(out,'wasm_bridge.js'),outcome);m.wasm.push({name,binary_sha256:sha(fs.readFileSync(path.join(out,'wasm_bridge_bg.wasm'))),binding_sha256:sha(fs.readFileSync(path.join(out,'wasm_bridge.js'))),results});
      }
      persist();
    }
    m.binary_equal={plain:m.builds[0].binary.sha256===m.builds[1].binary.sha256,remap:m.builds[2].binary.sha256===m.builds[3].binary.sha256};
    if(!historical)for(const cell of PROTOCOL.performance){for(const name of ['remap-a','remap-b'])m.measurements.push({...performanceRun(roots[name],cell),build:name,warmup:true,repetition:0});
      for(let repetition=1;repetition<=PROTOCOL.repetitions;repetition++)for(const name of repetition%2?['remap-a','remap-b']:['remap-b','remap-a'])m.measurements.push({...performanceRun(roots[name],cell),build:name,warmup:false,repetition});
      persist();
    }
    if(!historical)m.performance=measureSummary(m.measurements);m.complete=true;persist();verify(directory);
  }catch(error){m.complete=false;m.failure=String(error.stack);persist();throw error;}
  finally {fs.rmSync(temporary,{recursive:true,force:true});}
  return verify(directory);
}
async function main(){const[mode,...args]=process.argv.slice(2);let result;
  if(mode==='collect'||mode==='historical'){assert.equal(args.length,1);result=await collect(path.resolve(args[0]),mode==='historical');}
  else if(mode==='verify'){assert.equal(args.length,1);result=verify(args[0]);}
  else if(mode==='compare')result=compare(args);
  else throw Error('collect|historical|verify DIRECTORY; compare DIRECTORY DIRECTORY ...');
  console.log(JSON.stringify(result,null,2));
}
if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href)await main();
