// Full-release recovery and negative publication controls; not timing samples.
import fs from 'node:fs';import path from 'node:path';import assert from 'node:assert/strict';import {execFileSync} from 'node:child_process';import {setTimeout as delay} from 'node:timers/promises';
import {start} from '../platform/server.mjs';import {hash} from '../platform/store.mjs';
const directory=process.argv[2];if(!directory)throw Error('Usage: node scripts/accept-loop-c-recovery.mjs NEW_DIRECTORY');fs.mkdirSync(directory,{recursive:false});
const csv=execFileSync('unzip',['-p','output/s6-datasets/shoppers.zip','online_shoppers_intention.csv'],{encoding:'utf8',maxBuffer:4000000});
const options={directory:path.resolve(directory,'store'),binary:path.resolve('target/release/cli'),token:'recovery_acceptance_token_012345678901234',large:true};let app=await start(options);
const request=async(route,data)=>{const r=await fetch(app.url+route,{method:data===undefined?'GET':'POST',headers:{Authorization:`Bearer ${options.token}`,...(data===undefined?{}:{'Content-Type':'application/json'})},...(data===undefined?{}:{body:JSON.stringify(data)})});return {status:r.status,body:await r.json()};};
const wait=async(id,status)=>{for(let i=0;i<1400;i++){const j=(await request('/v1/jobs/'+id)).body;if(status.includes(j.status))return j;await delay(100);}throw Error('recovery deadline');};
const evidence={schema:'loop-c-recovery-v1',binary_sha256:hash(fs.readFileSync(options.binary)),csv_sha256:hash(csv)};
try {
 const p=(await request('/v1/projects',{name:'full-release recovery'})).body;
 const v=(await request(`/v1/projects/${p.id}/month-imports`,{name:'full',csv,policy:'fixed'})).body;
 const config={execution_profile:'joined-v4',max_steps:13000,max_candidates:20000,max_work:10000000,deadline_ms:120000,detect_cycles:true,seed:'42'};
 const submit=async key=>(await request(`/v1/projects/${p.id}/jobs`,{version_id:v.id,idempotency_key:key,settings:config})).body;
 const j=await submit('restart');await wait(j.id,['running']);await request(`/v1/jobs/${j.id}/pause`,{});
 const paused=await wait(j.id,['paused','failed','timed_out']);assert.equal(paused.status,'paused');assert.equal(paused.commits,5000);assert.ok(paused.checkpoint_sha256);assert.equal((await request(`/v1/jobs/${j.id}/month-results`)).status,409);evidence.paused=paused;
 await app.close();app=await start(options);const saved=(await request(`/v1/jobs/${j.id}`)).body;assert.equal(saved.checkpoint_sha256,paused.checkpoint_sha256);assert.equal(saved.elapsed_ms,paused.elapsed_ms);
 assert.equal((await request(`/v1/jobs/${j.id}/resume`,{})).status,202);const done=await wait(j.id,['completed','failed','timed_out']);assert.equal(done.outcome,'NormalForm');assert.equal(done.commits,12330);assert.equal((await request(`/v1/jobs/${j.id}/month-results`)).body.complete,true);evidence.resumed=done;
 const cancelled=await submit('cancel');await wait(cancelled.id,['running']);await request(`/v1/jobs/${cancelled.id}/cancel`,{});const stopped=await wait(cancelled.id,['cancelled']);assert.equal(stopped.trace_sha256,null);assert.equal((await request(`/v1/jobs/${cancelled.id}/month-results`)).status,409);evidence.cancelled=stopped;
 // Deliberately zero candidate allowance: a checked resource prefix is not a domain result.
 const limited=(await request(`/v1/projects/${p.id}/jobs`,{version_id:v.id,idempotency_key:'limited',settings:{...config,max_candidates:0}})).body;
 const prefix=await wait(limited.id,['completed','failed','timed_out']);assert.equal(prefix.outcome,'ResourceLimit');assert.equal((await request(`/v1/jobs/${limited.id}/month-results`)).body.complete,false);assert.equal((await request(`/v1/jobs/${limited.id}/month-csv`)).status,409);evidence.resource_limited=prefix;
 fs.writeFileSync(path.join(directory,'acceptance.json'),JSON.stringify(evidence,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({pause:paused.commits,resume:done.commits,cancel:stopped.status,resource:prefix.outcome}));
}finally{await app.close();}
