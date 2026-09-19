// Serial, isolated-process extension matrix. Never overwrites prior collections.
import {spawn,execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import {resolve} from 'node:path';
import os from 'node:os';
const [directory, imports]=process.argv.slice(2);
if(!directory||!imports||process.argv.length!==4||existsSync(directory))throw Error('node scripts/scale-study.mjs NEW_OUTPUT_DIRECTORY IMPORT_DIRECTORY');
if(process.platform!=='darwin')throw Error('This measurement protocol requires macOS /usr/bin/time -l');
mkdirSync(directory,{recursive:true});
const hash=x=>createHash('sha256').update(x).digest('hex');
const binary=resolve('target/release/examples/scale_probe');
const protocol=readFileSync('docs/scalability-study.md');
const files=[...new Set(execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n'))].filter(p=>existsSync(p)&&(/^(Cargo\.(lock|toml)|rust-toolchain.toml|rustfmt.toml)$/.test(p)||/^crates\/.*\.(rs|toml)$/.test(p)||/^examples\//.test(p)||['scripts/scale-study.mjs','scripts/import-study-data.mjs','scripts/import-study-data.test.mjs','docs/scalability-study.md'].includes(p))).sort();
const bundle=gzipSync(JSON.stringify({schema:'source-bundle-v1',encoding:'base64',files:Object.fromEntries(files.map(p=>[p,readFileSync(p).toString('base64')]))}));
writeFileSync(resolve(directory,'sources.json.gz'),bundle,{flag:'wx'});
writeFileSync(resolve(directory,'protocol.md'),protocol,{flag:'wx'});
const manifest=JSON.parse(readFileSync(resolve(imports,'manifest.json')));
for(const data of manifest.datasets)for(const subset of data.subsets)if(hash(readFileSync(resolve(imports,subset.file)))!==subset.sha256)throw Error('import checksum mismatch');
writeFileSync(resolve(directory,'datasets.json'),JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
const matrix=[];
const add=(f,n,p,e=1,s=42)=>matrix.push([f,n,p,e,s]);
for(const n of [1000,10000,100000])for(const p of ['validate','clone','inspect','bindings-reference','bindings-indexed','enabled-reference','enabled-compact'])add('fixed',n,p);
for(const p of ['observe','compare','prepare','baseline'])add('fixed',1000,p);
for(const f of ['symmetry','star','degree','rules'])for(const p of ['bindings-reference','bindings-indexed'])add(f,1000,p);
for(const f of ['fixed','guarded','oscillating','restoration'])for(const n of [10,1000])for(const e of [1,20])for(const p of ['run-reference','run-compact'])add(f,n,p,e);
for(const p of ['run-reference','run-compact','record-reference','record-compact','baseline'])add('fixed',100,p,101);
for(const f of ['oscillating','restoration'])for(const e of [20,100])for(const p of ['record-reference','record-compact'])add(f,1000,p,e);
for(const data of manifest.datasets)for(const subset of data.subsets)add(`import:${resolve(imports,subset.file)}`,0,'import',0);
writeFileSync(resolve(directory,'matrix.json'),JSON.stringify(matrix,null,2)+'\n',{flag:'wx'});
writeFileSync(resolve(directory,'environment.json'),JSON.stringify({schema:'scale-study-v1',date:new Date().toISOString(),platform:process.platform,os:os.release(),cpu:os.cpus()[0].model,memory:os.totalmem(),node:process.version,rust:execFileSync('rustc',['-Vv'],{encoding:'utf8'}),commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),dirty_status:execFileSync('git',['status','--porcelain'],{encoding:'utf8'}),source_bundle_sha256:hash(bundle),binary_sha256:hash(readFileSync(binary)),protocol_sha256:hash(protocol),timeout_ms:30000,sampled_group_rss_ceiling_bytes:512*1024*1024,sampling_ms:100,warmups:1,repetitions:5,seed:42},null,2)+'\n',{flag:'wx'});
let activeKill=null,interrupted=false;
process.on('SIGINT',()=>{interrupted=true;activeKill?.('Cancelled');});
async function sample(args){return await new Promise(resolveResult=>{
 const started=performance.now();const child=spawn('/usr/bin/time',['-l',binary,...args.map(String)],{detached:true,stdio:['ignore','pipe','pipe']});
 let stdout='',stderr='',censored=null,checking=false,closed=false,maxSampled=0;
 const kill=why=>{if(censored||closed)return;censored=why;try{process.kill(-child.pid,'SIGKILL');}catch{}};activeKill=kill;
 child.stdout.on('data',b=>{stdout+=b;if(stdout.length>1024*1024)kill('OutputLimit');});child.stderr.on('data',b=>{stderr+=b;if(stderr.length>1024*1024)kill('OutputLimit');});
 const timer=setTimeout(()=>kill('Timeout'),30000);
 const memory=setInterval(()=>{if(checking||closed)return;checking=true;const ps=spawn('ps',['-axo','pgid=,rss=']);let data='';ps.stdout.on('data',b=>data+=b);ps.on('error',e=>{stderr+=String(e);kill('InstrumentationFailure');});ps.on('close',code=>{checking=false;if(closed)return;if(code!==0){kill('InstrumentationFailure');return;}const kb=data.trim().split('\n').reduce((sum,line)=>{const [group,rss]=line.trim().split(/\s+/).map(Number);return sum+(group===child.pid?rss:0);},0);maxSampled=Math.max(maxSampled,kb*1024);if(maxSampled>512*1024*1024)kill('MemoryCeiling');});},100);
 child.on('error',e=>{stderr+=String(e);});
 child.on('close',(exit_code,signal)=>{closed=true;clearTimeout(timer);clearInterval(memory);activeKill=null;let result=null;try{result=JSON.parse(stdout);}catch{}const peak=stderr.match(/(\d+)\s+maximum resident set size/);resolveResult({exit_code,signal,censored,elapsed_ms:performance.now()-started,peak_rss_bytes:peak?Number(peak[1]):null,max_sampled_group_rss_bytes:maxSampled,result,stdout,stderr});});
});}
const raw=[];
outer:for(const args of matrix){
 for(let repetition=-1;repetition<5;repetition++){
   if(interrupted)break outer;
   const row={args,repetition,...await sample(args)};raw.push(row);
   writeFileSync(resolve(directory,'raw.json'),JSON.stringify(raw,null,2)+'\n');
   console.log(args.join('/'),repetition,row.censored??row.result?.outcome??'ProcessFailure');
 }
}
const stats=values=>{const sorted=values.toSorted((a,b)=>a-b);return sorted.length?{n:sorted.length,min:sorted[0],median:sorted[Math.floor(sorted.length/2)],max:sorted.at(-1)}:null;};
const summary=matrix.map(args=>{const rows=raw.filter(r=>r.repetition>=0&&JSON.stringify(r.args)===JSON.stringify(args));const good=rows.filter(r=>r.exit_code===0&&!r.censored&&r.result);return {args,attempts:rows.length,outcomes:rows.map(r=>r.censored??r.result?.outcome??'ProcessFailure'),phase_ns:stats(good.map(r=>r.result.phase_ns)),elapsed_ms:stats(good.map(r=>r.elapsed_ms)),peak_rss_bytes:stats(good.map(r=>r.peak_rss_bytes).filter(Number.isFinite)),commits:good.map(r=>r.result.commits),complete_count:good.map(r=>r.result.complete_count??null)};});
writeFileSync(resolve(directory,'summary.json'),JSON.stringify(summary,null,2)+'\n',{flag:'wx'});
const top=summary.find(r=>r.args[0]==='fixed'&&r.args[1]===100000&&r.args[2]==='enabled-compact');
const eligible=top?.outcomes.length===5&&top.outcomes.every(s=>s==='Completed')&&top.elapsed_ms.max*10<15000&&top.peak_rss_bytes?.max*10<256*1024*1024;
writeFileSync(resolve(directory,'million-gate.json'),JSON.stringify({eligible:Boolean(eligible),decision:eligible?'requires separate explicitly enlarged envelope and collection':'not attempted: predecessor complete-query forecast fails resource/headroom gate',interrupted},null,2)+'\n',{flag:'wx'});
if(interrupted||raw.some(r=>!r.censored&&(r.exit_code!==0||!r.result)))process.exitCode=1;
