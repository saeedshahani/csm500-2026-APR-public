// Freeze before serial diagnostics. A new directory is mandatory.
import {execFileSync,spawnSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {gzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
import os from 'node:os';
const [destination]=process.argv.slice(2);
if(!destination||process.argv.length!==3)throw Error('Usage: node scripts/collect-loop-f.mjs NEW_DIRECTORY');
const dir=resolve(destination),binary=resolve('target/release/examples/meta_probe');
mkdirSync(dir,{recursive:false});
const sha=b=>createHash('sha256').update(b).digest('hex');
const cells=[];
for(const mode of ['credit-restoration','spawn'])for(let n=0;n<=4;n++)cells.push([mode,n,128]);
for(let n=1;n<=4;n++)cells.push(['guard',n,128]);
for(const mode of ['object-loop','meta-noop','self','finite-then-object-loop','delete-head','restoration','gated','ungated','interleaved'])cells.push([mode,1,128]);
cells.push(['unfunded-spawn',0,8],['spawn',2,1]);
const protocol={schema:'loop-f-diagnostic-v1',cells,timeout_ms:30000,output_bytes:4*1024**2,samples:1,performance_claim:false};
const paths=[...new Set(execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n'))].filter(p=>/^(Cargo\.(toml|lock)|rust-toolchain\.toml)$/.test(p)||/^crates\/.*\.(rs|toml|graph|atoms|lambda|txt)$/.test(p)||p.startsWith('examples/')||p==='scripts/collect-loop-f.mjs'||p==='scripts/verify-loop-f.mjs'||p==='benchmarks/PROTOCOL.md').sort();
const bundle=gzipSync(JSON.stringify({schema:'source-bundle-v1',encoding:'base64',files:Object.fromEntries(paths.map(p=>[p,readFileSync(p).toString('base64')]))}));
writeFileSync(resolve(dir,'sources.json.gz'),bundle,{flag:'wx'});
const record={protocol,source_bundle_sha256:sha(bundle),binary_sha256:sha(readFileSync(binary)),collected_at:new Date().toISOString(),environment:{platform:process.platform,arch:os.arch(),release:os.release(),cpu:os.cpus()[0]?.model},rows:[]};
for(const cell of cells){
  const p=spawnSync(binary,cell.map(String),{encoding:'utf8',timeout:protocol.timeout_ms,maxBuffer:protocol.output_bytes});
  let result=null,error=p.error?.message??null;
  try{if(p.status===0&&!error)result=JSON.parse(p.stdout);}catch{error='invalid output';}
  record.rows.push({cell,status:p.status,signal:p.signal,error,stdout:p.stdout,stderr:p.stderr,result});
  writeFileSync(resolve(dir,'collection.json'),JSON.stringify(record,null,2)+'\n');
  console.log(...cell,result?.result?.total_edits?.verdict??result?.result?.verdict??error??p.signal);
}
