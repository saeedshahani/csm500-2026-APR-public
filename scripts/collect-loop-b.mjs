// Serial, bounded diagnostic collection. Never overwrite a historical result.
import {spawn,execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import os from 'node:os';
const [destination,baselineRoot]=process.argv.slice(2);
if(!destination||!baselineRoot||process.argv.length!==4)throw Error('Usage: node scripts/collect-loop-b.mjs NEW_DIRECTORY BASELINE_CHECKOUT');
const dir=resolve(destination),reference=resolve(baselineRoot,'target/release/examples/scale_probe'),candidate=resolve('target/release/examples/scale_probe');
const sha=b=>createHash('sha256').update(b).digest('hex');
const protocol={schema:'loop-b-diagnostic-v1',baseline_commit:'b650e4f3b55370b9fcc533f2d91cadc02819a689',warmups:1,samples:3,timeout_ms:30000,hard_memory_cap:null,seed:42,
  phases:['clone','prepare','enabled-compact','compare','compare-indexed','compare-permuted','pipeline-reference','pipeline-indexed'],
  comparison_work:{identity:10000000,permuted:100000000},
  initial_sizes:[10,100,1000],comparison_sizes:[1000,10000,100000],full_run_ladder:[10,100,1000,10000,100000,1000000],
  gate:'Only complete NormalForm + verified domain + portable replay advances full-run size. A failed cell retains all three samples; larger cells are not attempted.',
  note:'Diagnostic medians only. Pipeline includes recording, recurrence observation, portable export and independently checked import. Combined candidate changes are not a causal isolation of persistence versus validation reuse.'};
mkdirSync(dir,{recursive:false});
writeFileSync(resolve(dir,'protocol.json'),JSON.stringify(protocol,null,2)+'\n',{flag:'wx'});
const paths=[...new Set(execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n'))].filter(p=>/^(Cargo\.(toml|lock)|rust-toolchain\.toml)$/.test(p)||/^crates\/.*\.(rs|toml|graph|atoms|lambda|txt)$/.test(p)||p.startsWith('examples/')||/^benchmarks\/results\/s6-loop-b-compatibility\/.*\.json$/.test(p)||['scripts/collect-loop-b.mjs','scripts/check-loop-b-compatibility.mjs'].includes(p)).sort();
const bundle=gzipSync(JSON.stringify({schema:'source-bundle-v1',encoding:'base64',files:Object.fromEntries(paths.map(p=>[p,readFileSync(p).toString('base64')]))}));
writeFileSync(resolve(dir,'sources.json.gz'),bundle,{flag:'wx'});
const rows=[],gates=[];
const record={protocol,source_bundle_sha256:sha(bundle),binary_sha256:{baseline:sha(readFileSync(reference)),candidate:sha(readFileSync(candidate))},
  collected_at:new Date().toISOString(),environment:{platform:process.platform,arch:os.arch(),release:os.release(),cpu:os.cpus()[0]?.model,memory:os.totalmem()},rows,gates};
const save=()=>writeFileSync(resolve(dir,'collection.json'),JSON.stringify(record,null,2)+'\n');
function processRun(binary,args){
 return new Promise(resolve=>{
  const p=spawn(binary,args,{detached:true,stdio:['ignore','pipe','pipe']});let stdout='',stderr='',error=null;
  const kill=message=>{error=message;try{process.kill(-p.pid,'SIGKILL');}catch(e){if(e.code!=='ESRCH')throw e;}};
  const timer=setTimeout(()=>kill('process-group deadline exceeded'),protocol.timeout_ms);
  p.stdout.on('data',b=>{stdout+=b;if(stdout.length+stderr.length>4*1024*1024)kill('output byte bound exceeded');});
  p.stderr.on('data',b=>{stderr+=b;if(stdout.length+stderr.length>4*1024*1024)kill('output byte bound exceeded');});
  p.on('error',e=>{error=e.message;});
  p.on('close',(status,signal)=>{clearTimeout(timer);resolve({stdout,stderr,status,signal,error});});
 });
}
async function cell(build,family,size,phase,events){
 const binary=build==='baseline'?reference:candidate,results=[];
 for(let sample=-1;sample<protocol.samples;sample++){
  const args=[family,String(size),phase,String(events),'42'];
  const timed=process.platform==='darwin';
  const p=await processRun(timed?'/usr/bin/time':binary,timed?['-l',binary,...args]:args);
  let result=null;try{if(p.status===0)result=JSON.parse(p.stdout);}catch{}
  const row={build,family,size,phase,events,sample,warmup:sample<0,status:p.status,signal:p.signal,error:p.error,stdout:p.stdout,stderr:p.stderr,result,
    peak_rss_bytes:timed?Number(p.stderr.match(/(\d+)\s+maximum resident set size/)?.[1])||null:null};
  rows.push(row);if(sample>=0)results.push(row);save();
  console.log(build,family,size,phase,sample,result?.outcome??row.error??row.signal);
 }
 return results.every(r=>r.result?.outcome==='NormalForm'&&r.result?.domain_verified===true&&r.result?.replay_verified===true);
}
for(const size of [10,100,1000])for(const build of ['baseline','candidate'])for(const phase of ['clone','prepare','enabled-compact'])await cell(build,'fixed',size,phase,1);
for(const family of ['fixed','symmetry'])for(const size of protocol.comparison_sizes){
 if(size===1000)await cell('baseline',family,size,'compare',1);
 await cell('candidate',family,size,'compare-indexed',1);
 await cell('candidate',family,size,'compare-permuted',1);
}
for(const phase of ['pipeline-reference','pipeline-indexed']){
 for(const size of protocol.full_run_ladder){
  if(size>1000){gates.push({phase,size,outcome:'NotAttempted',reason:'Probe event/graph envelope requires a separately frozen extension after the 1,000-item gate.'});break;}
  if(!await cell('candidate','fixed',size,phase,size)){gates.push({phase,size,outcome:'GateFailed',remaining:protocol.full_run_ladder.filter(n=>n>size)});break;}
 }
}
await cell('candidate','guarded',10,'pipeline-indexed',11);
await cell('candidate','oscillating',10,'pipeline-indexed',20);
save();
