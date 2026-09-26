// Emulator-first remote persistence. No ambient/default cloud project is accepted.
// One fenced service owner is intentional; this is not a multi-worker scheduler.
import {randomUUID} from 'node:crypto';
import {initializeApp, deleteApp} from 'firebase-admin/app';
import {getFirestore} from 'firebase-admin/firestore';
import {getStorage} from 'firebase-admin/storage';
import {validate, hash, fail} from './store.mjs';
import {emulatorConfig, productionConfig} from './identity.mjs';

const collections=['projects','datasets','versions','jobs'];
const capacities={projects:32,datasets:128,versions:128,jobs:256};
const LEASE_MS=60000;

export class FirebaseStore {
  static async open({projectId,namespace,storeId='default',production=false,bucket,verifyArtefacts=true}) {
    if(production){
      productionConfig(projectId);
      if(bucket!==`${projectId}.firebasestorage.app`)throw Error('Production bucket must be the pinned project default');
    }else{
      emulatorConfig(projectId,process.env.FIRESTORE_EMULATOR_HOST);
      emulatorConfig(projectId,process.env.FIREBASE_STORAGE_EMULATOR_HOST);
    }
    if(namespace!==`firebase-${production?'production':'emulator'}:${projectId}` || !/^[a-zA-Z0-9_-]{1,64}$/.test(storeId))
      throw Error('Firebase storage requires matching identity and a simple store ID');
    const app=initializeApp({projectId,storageBucket:production?bucket:`${projectId}.appspot.com`},`graph-store-${randomUUID()}`);
    const store=new FirebaseStore(app,storeId,namespace);
    try { await store.load({verifyArtefacts});return store; }
    catch(e){await store.close();throw e;}
  }
  constructor(app,id,namespace) {
    this.app=app;this.db=getFirestore(app);this.bucket=getStorage(app).bucket();
    this.ref=this.db.doc(`reflectiveStores/${id}`);this.prefix=`reflective/${id}/`;
    this.namespace=namespace;this.owner=randomUUID();this.queue=Promise.resolve();this.fault=false;
  }
  serial(operation) {
    const result=this.queue.then(()=>{if(this.fault)throw Error('Store lease/persistence unavailable; restart required');return operation();});
    this.queue=result.catch(()=>{});return result;
  }
  assertOwner(value) {
    if(!value || value.owner!==this.owner || value.generation!==this.generation || value.expires_at<=Date.now())
      throw Error('Store fencing rejected an expired or superseded owner');
  }
  async checkLease() {
    if(this.fault)throw Error('Store unavailable');
    try {this.assertOwner((await this.ref.get()).data());}
    catch(e){this.fault=true;throw e;}
  }
  async load({verifyArtefacts=true}={}) {
    await this.db.runTransaction(async tx=>{
      const prior=(await tx.get(this.ref)).data();
      if(prior && prior.namespace!==this.namespace)throw Error('Identity namespace mismatch');
      if(prior?.expires_at>Date.now())fail(503,'store_busy','Store owner lease is still active; retry shortly');
      this.generation=(prior?.generation??0)+1;
      if(!Number.isSafeInteger(this.generation))throw Error('Lease generation exhausted');
      if(prior && (!Number.isSafeInteger(prior.used_bytes)||prior.used_bytes<0||prior.used_bytes>64*1024*1024))throw Error('Invalid storage reservation');
      tx.set(this.ref,{namespace:this.namespace,owner:this.owner,generation:this.generation,
        expires_at:Date.now()+LEASE_MS,used_bytes:prior?.used_bytes??0});
    });
    this.timer=setInterval(()=>{void this.serial(async()=>{
      await this.db.runTransaction(async tx=>{const value=(await tx.get(this.ref)).data();this.assertOwner(value);tx.update(this.ref,{expires_at:Date.now()+LEASE_MS});});
    }).catch(()=>{this.fault=true;});},15000);this.timer.unref();
    this.state={schema:'reflective-local-store-v1',identity_namespace:this.namespace};
    for(const collection of collections) {
      const snapshot=await this.ref.collection(collection).orderBy('position').limit(capacities[collection]+1).get();
      this.state[collection]=snapshot.docs.map((d,index)=>{
        if(d.data().position!==index)throw Error('Stored ordering is invalid');
        const value=JSON.parse(d.data().json);if(value.id!==d.id)throw Error('Stored identity mismatch');return value;
      });
    }
    validate(this.state);
    if(verifyArtefacts){
    for(const v of this.state.versions)await this.read(v.source_sha256);
    for(const d of this.state.datasets){await this.read(d.document_sha256);await this.read(d.source_sha256);}
    for(const j of this.state.jobs)for(const key of ['trace_sha256','checkpoint_sha256'])if(j[key]){
      const trace=JSON.parse(await this.read(j[key]));
      if(trace.source_sha256!==j.source_sha256 || trace.implementation!==j.implementation
          || trace.status!==(key==='trace_sha256'?'finished':'yielded')
          || trace.events.length!==(key==='trace_sha256'?j.commits:j.checkpoint_commits)
          || (key==='trace_sha256'&&trace.report?.outcome!==j.outcome))throw Error('Stored trace metadata mismatch');
    }
    }
    await this.update(state=>{
      for(const j of state.jobs)if(j.status==='running'){
        const attempt=j.attempts.at(-1), delta=attempt?Date.now()-Date.parse(attempt.started_at):j.settings.deadline_ms;
        j.elapsed_ms+=Math.min(Math.max(0,j.settings.deadline_ms-j.elapsed_ms),delta<0?j.settings.deadline_ms:Math.ceil(delta));
        if(attempt){attempt.status='interrupted';attempt.finished_at=new Date().toISOString();}
        j.status=j.cancellation_requested_at?'cancelled':'interrupted';j.finished_at=new Date().toISOString();
        j.failure='Previous service owner stopped; recover only a verified checkpoint.';
      }
    });
  }
  update(change) {
    return this.serial(async()=>{
      const next=structuredClone(this.state),result=change(next);validate(next);
      if(Buffer.byteLength(JSON.stringify(next))>8*1024*1024)fail(507,'storage_limit','Metadata capacity reached');
      const writes=[];
      for(const collection of collections)if(next[collection].length<this.state[collection].length
          || this.state[collection].some((value,index)=>value.id!==next[collection][index]?.id))
        throw Error('Record deletion/reordering requires a separate retention operation');
      for(const collection of collections)for(const [position,value] of next[collection].entries()){
        const json=JSON.stringify(value);
        if(Buffer.byteLength(json)>800*1024)fail(507,'storage_limit','Metadata record too large');
        if(json!==JSON.stringify(this.state[collection][position]))writes.push([this.ref.collection(collection).doc(value.id),{position,json}]);
      }
      if(writes.length>450)throw Error('Transaction write capacity exceeded');
      try{
        await this.db.runTransaction(async tx=>{
          this.assertOwner((await tx.get(this.ref)).data());
          for(const [ref,value]of writes)tx.set(ref,value);
          tx.update(this.ref,{expires_at:Date.now()+LEASE_MS});
        });
      }catch(e){this.fault=true;throw e;}
      this.state=next;return result;
    });
  }
  put(bytes) {
    return this.serial(async()=>{
      const sha=hash(bytes),size=Buffer.byteLength(bytes),manifest=this.ref.collection('artefacts').doc(sha);
      if(size>16*1024*1024)fail(507,'storage_limit','Artefact exceeds 16 MiB');
      // Reservations include failed uploads/orphans in the quota; never silently free them.
      await this.db.runTransaction(async tx=>{
        const owner=(await tx.get(this.ref)).data(),prior=(await tx.get(manifest)).data();this.assertOwner(owner);
        if(prior){if(prior.size!==size)throw Error('Artefact manifest mismatch');return;}
        if(owner.used_bytes+size>64*1024*1024)fail(507,'storage_limit','Artefact capacity reached');
        tx.set(manifest,{sha256:sha,size,status:'reserved'});tx.update(this.ref,{used_bytes:owner.used_bytes+size});
      });
      const file=this.bucket.file(this.prefix+sha);
      try{await file.save(bytes,{resumable:false,preconditionOpts:{ifGenerationMatch:0},metadata:{contentType:'application/octet-stream',cacheControl:'private, no-store'}});}
      catch(e){if(Number(e.code)!==412)throw e;}
      const stored=await this.bytes(sha);if(hash(stored)!==sha || stored.length!==size)throw Error('Uploaded artefact mismatch');
      await this.db.runTransaction(async tx=>{this.assertOwner((await tx.get(this.ref)).data());tx.update(manifest,{status:'ready'});});
      return sha;
    });
  }
  async bytes(sha) {
    if(!/^[a-f0-9]{64}$/.test(sha))throw Error('Invalid artefact digest');
    const stream=this.bucket.file(this.prefix+sha).createReadStream();const chunks=[];let size=0;
    try{for await(const chunk of stream){size+=chunk.length;if(size>16*1024*1024)throw Error('Artefact too large');chunks.push(chunk);}}
    finally{stream.destroy();}
    return Buffer.concat(chunks);
  }
  async read(sha) {
    await this.checkLease();
    if(!/^[a-f0-9]{64}$/.test(sha))throw Error('Invalid artefact digest');
    const manifest=(await this.ref.collection('artefacts').doc(sha).get()).data();
    if(manifest?.status!=='ready')throw Error('Artefact has no completed upload manifest');
    const bytes=await this.bytes(sha);
    if(hash(bytes)!==sha || bytes.length!==manifest.size)throw Error('Artefact checksum mismatch');return bytes;
  }
  async close() {
    clearInterval(this.timer);await this.queue;
    try{if(this.generation)await this.db.runTransaction(async tx=>{const value=(await tx.get(this.ref)).data();if(value?.owner===this.owner&&value.generation===this.generation)tx.update(this.ref,{expires_at:0});});}
    finally{await this.db.terminate();await deleteApp(this.app);}
  }
}
