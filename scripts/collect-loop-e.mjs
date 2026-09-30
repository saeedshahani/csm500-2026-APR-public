// Freeze inputs before serial correctness/cost diagnostics; never overwrite a collection.
import {execFileSync,spawnSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {gzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
import os from 'node:os';
const [destination]=process.argv.slice(2);
if(!destination||process.argv.length!==3)throw Error('Usage: node scripts/collect-loop-e.mjs NEW_DIRECTORY');
const dir=resolve(destination),binary=resolve('target/release/examples/stabilisation_probe');
mkdirSync(dir,{recursive:false});
const sha=b=>createHash('sha256').update(b).digest('hex');
const cells=[];
for(let n=1;n<=3;n++)for(let code=0;code<6**n;code++)cells.push(['analyse',n,code]);
for(const mode of ['missing','duplicate'])for(let code=0;code<6;code++)cells.push([mode,1,code]);
const protocol={schema:'loop-e-diagnostic-v1',cells,timeout_ms:30000,output_bytes:1024**2,samples:1,performance_claim:false};
const paths=[...new Set(execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n'))].filter(p=>/^(Cargo\.(toml|lock)|rust-toolchain\.toml)$/.test(p)||/^crates\/.*\.(rs|toml|graph|atoms|lambda|txt)$/.test(p)||p.startsWith('examples/')||p==='scripts/collect-loop-e.mjs'||p==='scripts/verify-loop-e.mjs'||p==='docs/research/reflective-recovery.md').sort();
const bundle=gzipSync(JSON.stringify({schema:'source-bundle-v1',encoding:'base64',files:Object.fromEntries(paths.map(p=>[p,readFileSync(p).toString('base64')]))}));
writeFileSync(resolve(dir,'sources.json.gz'),bundle,{flag:'wx'});
const record={protocol,source_bundle_sha256:sha(bundle),binary_sha256:sha(readFileSync(binary)),collected_at:new Date().toISOString(),environment:{platform:process.platform,arch:os.arch(),release:os.release(),cpu:os.cpus()[0]?.model},rows:[]};
for(const cell of cells){
  const p=spawnSync(binary,cell.map(String),{encoding:'utf8',timeout:protocol.timeout_ms,maxBuffer:protocol.output_bytes});
  let result=null,error=p.error?.message??null;
  try{if(p.status===0&&!error)result=JSON.parse(p.stdout);}catch{error='invalid output';}
  record.rows.push({cell,status:p.status,signal:p.signal,error,stdout:p.stdout,stderr:p.stderr,result});
  writeFileSync(resolve(dir,'collection.json'),JSON.stringify(record,null,2)+'\n');
  console.log(...cell,result?.verdict??error??p.signal);
}
