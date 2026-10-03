import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {fitContexts,review,promoteContext} from '../platform/meaning.mjs';
try {
  const [command,path]=process.argv.slice(2);
  if(!['fit','run'].includes(command)||!path)throw Error('node scripts/interpret-context.mjs fit|run REQUEST.json (or -)');
  const bytes=readFileSync(path==='-'?0:path);if(bytes.length>128*1024)throw Error('Request exceeds 128 KiB');
  const req=JSON.parse(bytes);
  if(command==='fit'){const fitted=fitContexts(req.training);console.log(JSON.stringify({...fitted,review:review(fitted.model,req.validation)},null,2));}
  else console.log(JSON.stringify(promoteContext(resolve('target/release/cli'),req.model,req.validation,req.rows,req.approval_sha256),null,2));
}catch(e){console.error(e.message);process.exitCode=2;}
