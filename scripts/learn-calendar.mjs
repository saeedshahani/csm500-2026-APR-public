// Explicit local CLI. Native execution requires a receipt digest from review.
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {fit,gate,promote} from '../platform/learning.mjs';
const [command,file]=process.argv.slice(2);
try {
  if(!['fit','run'].includes(command)||!file)throw Error('node scripts/learn-calendar.mjs fit|run REQUEST.json (or -)');
  const bytes=readFileSync(file==='-'?0:file);if(bytes.length>256*1024)throw Error('Request exceeds 256 KiB');
  const request=JSON.parse(bytes);
  let result;
  if(command==='fit') {const model=fit(request.training);result={...model,review:gate(model.selected,request.validation)};}
  else result=promote(resolve('target/release/cli'),request.model,request.validation,request.rows,request.approval_sha256);
  console.log(JSON.stringify(result,null,2));
}catch(e){console.error(e.message);process.exitCode=2;}
