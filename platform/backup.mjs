import {gzipSync,gunzipSync} from 'node:zlib';
import {hash,validate} from './store.mjs';

export function referenced(state){
  return [...new Set([...state.versions.flatMap(v=>[v.source_sha256]),...state.datasets.flatMap(d=>[d.document_sha256,d.source_sha256]),
    ...state.jobs.flatMap(j=>[j.trace_sha256,j.checkpoint_sha256]).filter(Boolean)])].sort();
}
export async function backup(store,date=new Date()){
  const day=date.toISOString().slice(0,10),name=`backups/${store.namespace.startsWith('firebase-production:')?'pilot':'emulator'}/${day}.json.gz`;
  const file=store.bucket.file(name);
  if((await file.exists())[0])return {name,created:false};
  const state=structuredClone(store.state),artefacts=[];let bytes=0;
  for(const sha of referenced(state)){const value=await store.read(sha);bytes+=value.length;if(bytes>64*1024*1024)throw Error('Backup exceeds source quota');artefacts.push({sha256:sha,base64:value.toString('base64')});}
  const raw=Buffer.from(JSON.stringify({schema:'reflective-backup-v1',state,artefacts}));
  if(raw.length>100*1024*1024)throw Error('Backup exceeds encoded quota');
  const archive=gzipSync(raw);await store.checkLease();
  try{await file.save(archive,{resumable:false,preconditionOpts:{ifGenerationMatch:0},metadata:{contentType:'application/gzip',cacheControl:'private, no-store',metadata:{sha256:hash(archive)}}});}
  catch(e){if(Number(e.code)!==412)throw e;}
  return {name,created:true,sha256:hash(archive),bytes:archive.length};
}
export function decodeBackup(archive){
  if(archive.length>100*1024*1024)throw Error('Backup too large');
  const data=JSON.parse(gunzipSync(archive,{maxOutputLength:100*1024*1024}));
  if(data.schema!=='reflective-backup-v1'||!Array.isArray(data.artefacts))throw Error('Invalid backup schema');
  validate(data.state);const expected=referenced(data.state),blobs=new Map();let size=0;
  if(data.artefacts.length!==expected.length)throw Error('Backup manifest size mismatch');
  for(const item of data.artefacts){
    if(typeof item.base64!=='string')throw Error('Invalid backup artefact');
    const bytes=Buffer.from(item.base64,'base64');size+=bytes.length;
    if(size>64*1024*1024||bytes.length>16*1024*1024||hash(bytes)!==item.sha256||blobs.has(item.sha256))throw Error('Backup checksum or quota failure');
    blobs.set(item.sha256,bytes);
  }
  if(expected.some(sha=>!blobs.has(sha)))throw Error('Backup missing referenced artefact');
  return {state:data.state,blobs};
}
// Restore is deliberately separate from deployment and only targets an empty store.
export async function restore(store,archive){
  const {state,blobs}=decodeBackup(archive);
  if(store.state.identity_namespace!==state.identity_namespace)throw Error('Backup identity namespace mismatch');
  if(['projects','datasets','versions','jobs'].some(k=>store.state[k].length))throw Error('Restore requires an empty store');
  for(const bytes of blobs.values())await store.put(bytes);
  await store.update(s=>{for(const key of ['projects','datasets','versions','jobs'])s[key]=state[key];
    for(const job of s.jobs)if(['queued','running'].includes(job.status)){
      const attempt=job.attempts.at(-1);if(attempt?.status==='running'){attempt.status='interrupted';attempt.finished_at=new Date().toISOString();job.elapsed_ms=job.settings.deadline_ms;}
      job.status='interrupted';job.finished_at=new Date().toISOString();job.failure='Restored snapshot: explicit recovery or new execution required';
    }
  });
}
