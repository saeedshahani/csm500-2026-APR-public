// Read-only target preflight. This module cannot deploy or select an account default.
import fs from 'node:fs';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
export function target(project) {
  const config = JSON.parse(fs.readFileSync(new URL('./deployment-target.json',import.meta.url)));
  if (project !== 'reflective-graph-f1cd4' || config.project_id !== project || config.project_number !== '463455802523')
    throw Error('Explicit --project reflective-graph-f1cd4 is required; default or other projects are refused');
  return config;
}
if(process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const args=process.argv.slice(2);
  if(args.length!==2||args[0]!=='--project')throw Error('Usage: node platform/deployment-target.mjs --project reflective-graph-f1cd4 (read-only preflight)');
  console.log(JSON.stringify(target(args[1]),null,2));
}
