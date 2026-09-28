// Fresh local-platform jobs, full public release, independently checked CSV.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
import {start} from '../platform/server.mjs';
import {hash} from '../platform/store.mjs';
import {gzipSync} from 'node:zlib';

const output=process.argv[2];if(!output)throw Error('Usage: node scripts/collect-loop-c.mjs NEW_OUTPUT_DIRECTORY [pilot]');
fs.mkdirSync(output,{recursive:false});
const archive=fs.readFileSync('output/s6-datasets/shoppers.zip');
assert.equal(hash(archive),'2972e6184d3ad7beaaa831d9fc2b059dc3ee29df69d1ec593c466a5cd8485d14');
const csv=execFileSync('unzip',['-p','output/s6-datasets/shoppers.zip','online_shoppers_intention.csv'],{encoding:'utf8',maxBuffer:4*1024*1024});
const lines=csv.trim().split(/\r?\n/),rows=lines.slice(1).map(l=>l.split(','));assert.equal(rows.length,12330);
// Conventional oracle does not use graph compiler, rule dictionary or interpreter.
const lookup=new Map('Jan:01 Feb:02 Mar:03 Apr:04 May:05 June:06 Jul:07 Aug:08 Sep:09 Oct:10 Nov:11 Dec:12'.split(' ').map(p=>p.split(':')));
const baseline=[];let expected;
for(let i=0;i<4;i++){const t=performance.now();expected=lines[0]+',MonthNumber\n'+rows.map(r=>[...r,lookup.get(r[10])].join(',')).join('\n')+'\n';baseline.push(performance.now()-t);}
const binary=path.resolve('target/release/cli'),token='loop_c_acceptance_token_000000000000000';
const manifest={schema:'loop-c-platform-v1',created_at:new Date().toISOString(),archive_sha256:hash(archive),csv_sha256:hash(csv),binary_sha256:hash(fs.readFileSync(binary)),hardware:{platform:os.platform(),arch:os.arch(),cpus:os.cpus()[0].model,memory:os.totalmem()},baseline_ms:baseline,rows:rows.length,expected_sha256:hash(expected),samples:[]};
const files=[...new Set(execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n'))].filter(p=>/^(Cargo\.(toml|lock)|rust-toolchain.toml)$/.test(p)||/^(crates|examples|platform|frontend\/src|scripts)\/.+\.(rs|toml|graph|atoms|json|mjs|tsx?|css)$/.test(p)).sort();
const bundle=gzipSync(JSON.stringify({schema:'source-bundle-v1',encoding:'base64',files:Object.fromEntries(files.map(p=>[p,fs.readFileSync(p).toString('base64')]))}));
fs.writeFileSync(path.join(output,'sources.json.gz'),bundle,{flag:'wx'});manifest.source_bundle_sha256=hash(bundle);
manifest.protocol={warmups:1,fixed_measurements:3,guarded_measurements:1,whole_release:true,seed:42,deadline_ms:120000,sampled_native_rss_ceiling:2*1024**3,sample_interval_ms:500,hard_rss_cap:null,concurrent_workloads:false,execution_envelope:'large-local-v1',criterion:'NormalForm; complete exact CSV oracle; checked export/replay; full count and final paged event; no partial success'};
fs.writeFileSync(path.join(output,'results.json'),JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
for(const [i,policy] of (process.argv[3]==='pilot'?['fixed']:['fixed','fixed','fixed','fixed','guarded']).entries()){
 const directory=path.resolve(output,'store-'+i);const app=await start({directory,binary,token,large:true});
 let peak=0;const sampler=setInterval(()=>{try{const processes=execFileSync('ps',['-axo','pid=,ppid=,rss=,comm='],{encoding:'utf8'}).trim().split('\n').map(l=>l.trim().split(/\s+/));const native=processes.filter(p=>Number(p[1])===process.pid&&p.slice(3).join(' ').endsWith('/cli'));peak=Math.max(peak,...native.map(p=>Number(p[2])*1024));if(peak>2*1024**3)app.worker?.active?.control.abort('cancelled');}catch(e){app.worker?.active?.control.abort('cancelled');throw e;}},500);
 const request=async(route,data,raw=false)=>{const r=await fetch(app.url+route,{method:data===undefined?'GET':'POST',headers:{Authorization:`Bearer ${token}`,...(data===undefined?{}:{'Content-Type':'application/json'})},...(data===undefined?{}:{body:JSON.stringify(data)})});const value=raw?await r.text():await r.json();assert.ok(r.ok,JSON.stringify(value));return value;};
 try{
  const p=await request('/v1/projects',{name:'Loop C full release'});const t=performance.now();
  const v=await request(`/v1/projects/${p.id}/month-imports`,{name:policy,csv,policy});const import_ms=performance.now()-t;
  const startTime=performance.now();let job=await request(`/v1/projects/${p.id}/jobs`,{version_id:v.id,idempotency_key:'full',settings:{max_steps:13000,max_candidates:20000,max_work:10000000,deadline_ms:120000,detect_cycles:true,execution_profile:'joined-v4',seed:'42'}});
  while(['queued','running'].includes(job.status)){assert.ok(performance.now()-startTime<140000,'platform deadline');await delay(200);job=await request(`/v1/jobs/${job.id}`);}
  const wall_ms=performance.now()-startTime;
  const sample={i,warmup:i===0,policy,import_ms,wall_ms,peak_native_rss:peak,job};manifest.samples.push(sample);
  fs.writeFileSync(path.join(output,'results.json'),JSON.stringify(manifest,null,2)+'\n');
  assert.equal(job.status,'completed',JSON.stringify(job));assert.equal(job.outcome,'NormalForm');assert.equal(job.commits,rows.length+(policy==='guarded'?1:0));
  const pageStart=performance.now(),page=await request(`/v1/jobs/${job.id}/month-results?limit=25`);sample.page_ms=performance.now()-pageStart;
  assert.equal(page.total,rows.length);assert.equal(page.items.length,25);assert.equal(page.next_offset,25);assert.equal(page.complete,true);
  const csvStart=performance.now(),actual=await request(`/v1/jobs/${job.id}/month-csv`,undefined,true);sample.csv_ms=performance.now()-csvStart;assert.equal(actual,expected);sample.csv_sha256=hash(actual);
  const summary=await request(`/v1/jobs/${job.id}/report`);sample.summary=summary;
  assert.equal(summary.altered_rule_identities.length,policy==='guarded'?2:0);
  const traceFile=path.join(directory,'blobs',job.trace_sha256);sample.trace_bytes=fs.statSync(traceFile).size;
  const event=await request(`/v1/jobs/${job.id}/events?offset=${job.commits-1}&limit=1`);assert.equal(event.total,job.commits);assert.equal(event.items.length,1);assert.equal(event.next_offset,null);
  sample.oracle_verified=true;fs.writeFileSync(path.join(output,'results.json'),JSON.stringify(manifest,null,2)+'\n');
  console.log(JSON.stringify({i,policy,wall_ms,peak,trace_bytes:sample.trace_bytes,commits:job.commits}));
 }finally{clearInterval(sampler);await app.close();}
}
