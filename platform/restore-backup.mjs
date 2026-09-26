// Restore a downloaded backup to a new local directory for inspection/recovery.
// This command never overwrites a hosted store or existing directory.
import fs from 'node:fs';
import {resolve} from 'node:path';
import {Store} from './store.mjs';
import {decodeBackup,restore} from './backup.mjs';
import {native} from './worker.mjs';
const args=process.argv.slice(2);
if(args.length!==4||args[0]!=='--archive'||args[2]!=='--directory')throw Error('Usage: node platform/restore-backup.mjs --archive FILE --directory NEW_DIRECTORY');
const directory=resolve(args[3]);if(fs.existsSync(directory))throw Error('Destination already exists; nothing overwritten');
if(fs.statSync(args[1]).size>100*1024*1024)throw Error('Backup too large');
const archive=fs.readFileSync(args[1]),decoded=decodeBackup(archive);
const checked=new Set();
for(const job of decoded.state.jobs)for(const sha of [job.trace_sha256,job.checkpoint_sha256].filter(Boolean))if(!checked.has(sha)){
  const result=await native(resolve('target/release/cli'),['trace-check','-'],decoded.blobs.get(sha),{deadline:30000});
  if(result.code!==0||result.cause)throw Error('Backup history failed native verification');checked.add(sha);
}
const store=new Store(directory,{namespace:decoded.state.identity_namespace});
try{await restore(store,archive);console.log(JSON.stringify({restored:true,directory,verified_histories:checked.size,projects:store.state.projects.length,jobs:store.state.jobs.length}));}
finally{store.close();}
