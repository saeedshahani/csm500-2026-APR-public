import test from 'node:test';
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {FirebaseStore} from '../../firebase-store.mjs';
import {start} from '../../server.mjs';
import {hash,terminal} from '../../store.mjs';
import {Worker} from '../../worker.mjs';
import {backup,decodeBackup} from '../../backup.mjs';
const projectId='demo-reflective-graph', namespace=`firebase-emulator:${projectId}`;

test('request-scoped storage releases idle leases, executes one batch and preserves duplicate deliveries',async t=>{
  const options={projectId,namespace,storeId:randomUUID(),verifyArtefacts:false};
  let store=await FirebaseStore.open(options);
  t.after(async()=>{await store.close();});
  const project=randomUUID(),version=randomUUID(),jobId=randomUUID(),source=fs.readFileSync('examples/programs/arithmetic.graph');
  const worker=new Worker(store,path.resolve('target/release/cli')),sha=await store.put(source);
  await store.update(s=>{
    s.projects.push({id:project,name:'Request-scoped',owner_uid:'test-owner',members:[]});
    s.versions.push({id:version,project_id:project,source_sha256:sha});
    s.jobs.push({id:jobId,project_id:project,version_id:version,source_sha256:sha,engine_sha256:worker.identity,request_sha256:hash('request'),
      submitted_by:'test-owner',settings:{max_steps:20,max_work:1000000,max_candidates:1000,seed:null,detect_cycles:false,deadline_ms:30000},
      status:'queued',attempts:[],elapsed_ms:0,checkpoint_sha256:null,trace_sha256:null,created_at:new Date().toISOString()});
  });
  await store.close();store=await FirebaseStore.open(options);
  const w=new Worker(store,path.resolve('target/release/cli'));
  await w.execute(store.state.jobs[0],new AbortController().signal);
  assert.equal(store.state.jobs[0].status,'completed');const trace=store.state.jobs[0].trace_sha256;
  await store.close();store=await FirebaseStore.open(options);
  // Duplicate cloud delivery sees a terminal record and has no execution authority.
  assert.notEqual(store.state.jobs[0].status,'queued');assert.equal(store.state.jobs[0].attempts.length,1);
  assert.equal(store.state.jobs[0].trace_sha256,trace);
  const saved=await backup(store);assert.equal(saved.created,true);
  const [archive]=await store.bucket.file(saved.name).download();
  const restored=decodeBackup(archive);assert.equal(restored.state.jobs[0].trace_sha256,trace);assert.ok(restored.blobs.has(trace));
  assert.equal((await backup(store)).created,false);
});

test('Firestore/Storage adapters fence stale owners, reserve uploads and reject corruption',async t=>{
  const options={projectId,namespace,storeId:randomUUID()};
  let store=await FirebaseStore.open(options);
  t.after(async()=>{await store.close();});
  const bytes=Buffer.from('immutable evidence'),sha=await store.put(bytes);
  assert.equal(sha,hash(bytes));assert.deepEqual(await store.read(sha),bytes);
  assert.equal(await store.put(bytes),sha);
  assert.equal((await store.ref.get()).data().used_bytes,bytes.length);
  await assert.rejects(FirebaseStore.open(options),/lease is still active/);
  const original=store.bucket.file.bind(store.bucket);
  store.bucket.file=()=>({save:async()=>{throw Error('injected upload failure');}});
  await assert.rejects(store.put('failed upload'),/injected/);
  store.bucket.file=original;
  const failed=hash('failed upload');
  assert.equal((await store.ref.collection('artefacts').doc(failed).get()).data().status,'reserved');
  await assert.rejects(store.read(failed),/completed upload/);
  assert.equal(await store.put('failed upload'),failed); // Reservation is reconciled, not charged twice.
  await store.ref.update({owner:'superseding-owner',generation:store.generation+1});
  await assert.rejects(store.update(s=>s.projects.push({id:randomUUID(),name:'stale',owner_uid:'u',members:[]})),/fencing/);
  assert.equal((await store.ref.collection('projects').get()).size,0);
  await store.ref.update({expires_at:0});await store.close();
  store=await FirebaseStore.open(options);
  await store.bucket.file(store.prefix+sha).save('corrupt',{resumable:false});
  await assert.rejects(store.read(sha),/checksum/);
});

test('real API uses emulator metadata and artefacts across restart and concurrent idempotent requests',async t=>{
  const options={binary:path.resolve('target/release/cli'),authMode:'firebase-emulator',projectId,
    authHost:process.env.FIREBASE_AUTH_EMULATOR_HOST,storageMode:'firebase-emulator',storeId:randomUUID()};
  let app=await start(options);
  t.after(async()=>{await app.close();});
  const response=await fetch(`http://${options.authHost}/identitytoolkit.googleapis.com/v1/accounts:signUp?key=emulator-only`,{
    method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email:`${randomUUID()}@example.test`,password:'emulator-test-only',returnSecureToken:true})});
  assert.equal(response.status,200);const user=await response.json();
  async function call(route,data){const r=await fetch(app.url+route,{method:data===undefined?'GET':'POST',headers:{Authorization:`Bearer ${user.idToken}`,'Content-Type':'application/json'},...(data===undefined?{}:{body:JSON.stringify(data)})});assert.ok(r.ok,`${route}: ${r.status} ${r.ok?'':await r.text()}`);return r.json();}
  const p=await call('/v1/projects',{name:'Firestore-backed'});
  const v=await call(`/v1/projects/${p.id}/versions`,{name:'arithmetic',source:fs.readFileSync('examples/programs/arithmetic.graph','utf8')});
  const data={version_id:v.id,idempotency_key:'concurrent'};
  const jobs=await Promise.all([call(`/v1/projects/${p.id}/jobs`,data),call(`/v1/projects/${p.id}/jobs`,data)]);
  assert.equal(jobs[0].id,jobs[1].id);
  let job;
  for(let i=0;i<500;i++){job=await call(`/v1/jobs/${jobs[0].id}`);if(terminal.has(job.status))break;await delay(25);}
  assert.equal(job.status,'completed');assert.equal(job.outcome,'NormalForm');
  const trace=await call(`/v1/jobs/${job.id}/trace`);
  await app.close();app=await start(options);
  assert.deepEqual(await call(`/v1/jobs/${job.id}/trace`),trace);
  assert.equal((await call(`/v1/projects/${p.id}/jobs`,data)).id,job.id);
});
