import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {randomUUID} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import {taskRequest,admission} from '../hosted.mjs';
import {Store,hash} from '../store.mjs';
import {decodeBackup,restore} from '../backup.mjs';

test('task identity is deduplicated by job, checked batch and reconciliation minute',()=>{
  const job={id:randomUUID(),attempts:[]},base='https://graph-api-463455802523.europe-west2.run.app';
  const a=taskRequest(base,job,100),b=taskRequest(base,job,100);
  assert.deepEqual(a,b);assert.notEqual(a.task.name,taskRequest(base,job,101).task.name);
  job.attempts.push({});assert.notEqual(a.task.name,taskRequest(base,job,100).task.name);
  assert.deepEqual(JSON.parse(Buffer.from(a.task.httpRequest.body,'base64')),{job_id:job.id});
  assert.equal(a.task.httpRequest.oidcToken.audience,base);
  assert.throws(()=>taskRequest(base,{id:'../bad',attempts:[]}));
});

test('persistent request limits and kill switch reject mutation without preventing reads',async()=>{
  const values=new Map(),db={doc:name=>name,runTransaction:async fn=>fn({get:async ref=>({data:()=>values.get(ref)}),set:(ref,x)=>values.set(ref,x)})};
  const now=new Date('2026-09-26T12:00:00Z');
  values.set('reflectiveControl/pilot',{disabled:true});
  await assert.rejects(admission(db,'user',true,now),{code:'execution_disabled'});
  await admission(db,'user',false,now);
  values.set('reflectiveControl/pilot',{disabled:false});
  for(let i=0;i<100;i++)await admission(db,'user',true,new Date(now.getTime()+i*60000));
  await assert.rejects(admission(db,'user',true,new Date(now.getTime()+101*60000)),{code:'request_limit'});
  await admission(db,'user',true,new Date('2026-09-27T12:00:00Z'));
});

test('backup validates hashes and restores only into an empty identity-matched store',async t=>{
  const directory=fs.mkdtempSync(path.join(os.tmpdir(),'graph-backup-'));
  t.after(()=>fs.rmSync(directory,{recursive:true,force:true}));
  const store=new Store(directory);t.after(()=>store.close());
  const project=randomUUID(),version=randomUUID(),bytes=Buffer.from('program {}'),sha=hash(bytes);
  const jobs=['queued','running','paused'].map(status=>({id:randomUUID(),project_id:project,version_id:version,
    source_sha256:sha,request_sha256:sha,engine_sha256:sha,trace_sha256:null,checkpoint_sha256:null,
    status,elapsed_ms:10,settings:{deadline_ms:1000},attempts:status==='running'
      ?[{number:1,status:'running',started_at:'2026-09-26T12:00:00Z',finished_at:null}]:[]}));
  const state={schema:'reflective-local-store-v1',identity_namespace:'local-single-owner',projects:[{id:project,name:'Backup'}],datasets:[],jobs,
    versions:[{id:version,project_id:project,source_sha256:sha}]};
  const data={schema:'reflective-backup-v1',state,artefacts:[{sha256:sha,base64:bytes.toString('base64')}]};
  const archive=gzipSync(JSON.stringify(data));assert.equal(decodeBackup(archive).blobs.size,1);
  await restore(store,archive);assert.equal(store.state.projects[0].name,'Backup');assert.deepEqual(store.read(sha),bytes);
  assert.deepEqual(store.state.jobs.map(j=>j.status),['interrupted','interrupted','paused']);
  assert.equal(store.state.jobs[1].attempts[0].status,'interrupted');
  assert.equal(store.state.jobs[1].elapsed_ms,1000); // Unknown execution is charged conservatively.
  assert.equal(store.state.jobs[0].elapsed_ms,10);
  await assert.rejects(restore(store,archive),/empty/);
  data.artefacts[0].base64=Buffer.from('corrupt').toString('base64');assert.throws(()=>decodeBackup(gzipSync(JSON.stringify(data))),/checksum/);
});
