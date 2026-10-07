// Complete-state correctness diagnostics; deliberately not a throughput benchmark.
import {spawn,execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import os from 'node:os';
const [destination]=process.argv.slice(2);
if(!destination||process.argv.length!==3)throw Error('Usage: node scripts/collect-loop-d.mjs NEW_DIRECTORY');
const dir=resolve(destination),binary=resolve('target/release/examples/stability_probe');
const sha=b=>createHash('sha256').update(b).digest('hex');
mkdirSync(dir,{recursive:false});
const cells=[...Array.from({length:8},(_,i)=>['guard',i+1,512]),['guard',7,128],['output',1,128],['gated',1,128],['ungated',1,128],['frame',2,128],['compose',2,128]];
const protocol={schema:'loop-d-diagnostic-v1',cells,timeout_ms:30000,output_bytes:4*1024**2,samples:1,performance_claim:false};
const paths=[...new Set(execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n'))].filter(p=>/^(Cargo\.(toml|lock)|rust-toolchain\.toml)$/.test(p)||/^crates\/.*\.(rs|toml|graph|atoms|lambda|txt)$/.test(p)||p.startsWith('examples/')||p==='scripts/collect-loop-d.mjs'||p==='benchmarks/PROTOCOL.md').sort();
const bundle=gzipSync(JSON.stringify({schema:'source-bundle-v1',encoding:'base64',files:Object.fromEntries(paths.map(p=>[p,readFileSync(p).toString('base64')]))}));
writeFileSync(resolve(dir,'sources.json.gz'),bundle,{flag:'wx'});
const record={protocol,source_bundle_sha256:sha(bundle),binary_sha256:sha(readFileSync(binary)),collected_at:new Date().toISOString(),environment:{platform:process.platform,arch:os.arch(),release:os.release(),cpu:os.cpus()[0]?.model},rows:[]};
for(const cell of cells){
  const row=await new Promise(done=>{
    const child=spawn(binary,cell.map(String),{detached:true,stdio:['ignore','pipe','pipe']});
    let stdout='',stderr='',error=null,bytes=0;
    const kill=message=>{error??=message;try{process.kill(-child.pid,'SIGKILL');}catch(e){if(e.code!=='ESRCH')throw e;}};
    const timer=setTimeout(()=>kill('process-group deadline exceeded'),protocol.timeout_ms);
    child.stdout.on('data',b=>{bytes+=b.length;if(bytes>protocol.output_bytes)kill('output bound exceeded');else stdout+=b;});
    child.stderr.on('data',b=>{bytes+=b.length;if(bytes>protocol.output_bytes)kill('output bound exceeded');else stderr+=b;});
    child.on('error',e=>{error=e.message;});
    child.on('close',(status,signal)=>{clearTimeout(timer);let result=null;try{if(status===0&&!error)result=JSON.parse(stdout);}catch{error='invalid output';}done({cell,status,signal,error,stdout,stderr,result});});
  });
  record.rows.push(row);
  writeFileSync(resolve(dir,'collection.json'),JSON.stringify(record,null,2)+'\n');
  console.log(...cell,row.result?.verdict??row.error??row.signal);
}
