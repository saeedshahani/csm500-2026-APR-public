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
  const project=randomUUID(),bytes=Buffer.from('program {}'),sha=hash(bytes);
  const state={schema:'reflective-local-store-v1',identity_namespace:'local-single-owner',projects:[{id:project,name:'Backup'}],datasets:[],jobs:[],
    versions:[{id:randomUUID(),project_id:project,source_sha256:sha}]};
  const data={schema:'reflective-backup-v1',state,artefacts:[{sha256:sha,base64:bytes.toString('base64')}]};
  const archive=gzipSync(JSON.stringify(data));assert.equal(decodeBackup(archive).blobs.size,1);
  await restore(store,archive);assert.equal(store.state.projects[0].name,'Backup');assert.deepEqual(store.read(sha),bytes);
  await assert.rejects(restore(store,archive),/empty/);
  data.artefacts[0].base64=Buffer.from('corrupt').toString('base64');assert.throws(()=>decodeBackup(gzipSync(JSON.stringify(data))),/checksum/);
});
