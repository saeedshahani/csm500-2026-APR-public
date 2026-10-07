// Controlled native matrix, JSON evidence and summary plot. No concurrent builds/tests.
// Usage: node scripts/benchmark.mjs NEW_OUTPUT_DIRECTORY [--smoke]
import {spawn,execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync,existsSync,readdirSync,copyFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import path from 'node:path';
import os from 'node:os';
const directory=process.argv[2],smoke=process.argv.includes('--smoke');
if(!directory||existsSync(directory))throw Error('Supply a new output directory; existing evidence is never overwritten');
if(!smoke&&process.platform!=='darwin')throw Error('Reported protocol requires macOS; use --smoke elsewhere');
mkdirSync(directory,{recursive:true});
const command=(file,args)=>execFileSync(file,args,{encoding:'utf8'}).trim();
const hash=data=>createHash('sha256').update(data).digest('hex');
const files=command('git',['ls-files','-co','--exclude-standard']).split('\n').filter(f=>/^(Cargo\.(toml|lock)|rust-toolchain\.toml|crates\/.*\.(rs|toml)|scripts\/benchmark\.mjs)$/.test(f)).sort();
const digest=createHash('sha256');for(const f of files)digest.update(f).update('\0').update(readFileSync(f)).update('\0');
const metadata={date:new Date().toISOString(),smoke,platform:process.platform,os:os.release(),cpu:os.cpus()[0].model,logical_cpus:os.cpus().length,total_memory_bytes:os.totalmem(),rust:command('rustc',['-Vv']),cargo:command('cargo',['-V']),node:process.version,commit:command('git',['rev-parse','HEAD']),evaluated_source_sha256:digest.digest('hex'),source_files:files,protocol_sha256:hash(readFileSync('benchmarks/PROTOCOL.md')),binary_sha256:hash(readFileSync('target/release/examples/measure')),build:'cargo build --release --locked -p engine --example measure; default release flags; no RUSTFLAGS',rss_instrument:'/usr/bin/time -l; bytes; benchmark process maximum resident set',timeout_ms:30000,sampled_group_rss_ceiling_bytes:512*1024*1024,repetitions:smoke?1:5,warmup_processes:smoke?0:1,seed:42};
writeFileSync(path.join(directory,'environment.json'),JSON.stringify(metadata,null,2)+'\n');
copyFileSync('benchmarks/PROTOCOL.md',path.join(directory,'protocol.md'));
const matrix=[];
if(smoke){for(const row of [['branch',8,'enabled'],['reject',8,'enabled'],['lambda',1,'run'],['practical',10,'run'],['trace',8,'closure']])matrix.push(row);}
else {
 for(const n of [0,64])matrix.push(['calibration',n,'memory']);
 for(const [family,sizes] of [['atoms',[16,128,1024]],['fields',[16,128,1024]],['rules',[1,8,32]],['pattern',[1,4,8]],['branch',[1,8,32]],['reject',[8]],['practical',[10,100,1000]],['guarded',[10]],['lambda',[1,2,3]],['reflective',[1]]])
  for(const n of sizes)for(const phase of ['parse','load','validate_decode','scan','bindings','enabled','prepare','commit',...(['practical','guarded','lambda','reflective'].includes(family)?['run']:[])])matrix.push([family,n,phase]);
 for(const n of [8,32,64])for(const phase of ['order','closure','publish-1','publish-8','publish-32','replay-1','replay-8','replay-32'])matrix.push(['trace',n,phase]);
}
writeFileSync(path.join(directory,'matrix.json'),JSON.stringify(matrix,null,2)+'\n');
async function sample(args){
 return await new Promise(resolve=>{
  const child=spawn(smoke?'target/release/examples/measure':'/usr/bin/time',smoke?args:['-l','target/release/examples/measure',...args],{detached:true,stdio:['ignore','pipe','pipe']});let stdout='',stderr='',censored=null;
  child.stdout.on('data',b=>stdout+=b);child.stderr.on('data',b=>stderr+=b);
  const kill=reason=>{if(censored)return;censored=reason;try{process.kill(-child.pid,'SIGKILL');}catch{/* already exited */}};
  const timer=setTimeout(()=>kill('Timeout'),30000);
  let checking=false;
  const memory=setInterval(()=>{if(smoke||checking)return;checking=true;const ps=spawn('ps',['-axo','pgid=,rss=']);let data='';ps.stdout.on('data',b=>data+=b);ps.on('close',()=>{checking=false;const kb=data.trim().split('\n').reduce((sum,line)=>{const [group,rss]=line.trim().split(/\s+/).map(Number);return sum+(group===child.pid?rss:0);},0);if(kb*1024>512*1024*1024)kill('MemoryCeiling');});ps.on('error',()=>{checking=false;});},100);
  child.on('error',e=>{stderr+=String(e);});
  child.on('close',(code,signal)=>{clearTimeout(timer);clearInterval(memory);let result=null;try{result=JSON.parse(stdout);}catch{/* retain malformed output */}const rss=/([0-9]+)\s+maximum resident set size/.exec(stderr);resolve({args,exit_code:code,signal,censored,result,peak_rss_bytes:rss?Number(rss[1]):null,stdout,stderr});});
 });
}
const raw=[];
for(const row of matrix){
 for(let repetition=smoke?0:-1;repetition<(smoke?1:5);repetition++){
  const r={family:row[0],size:row[1],phase:row[2],repetition,...await sample(row.map(String))};raw.push(r);
  writeFileSync(path.join(directory,'raw.json'),JSON.stringify(raw,null,2)+'\n');
 }
 process.stdout.write(`${row.join('/')}\n`);
}
const median=values=>{const s=values.toSorted((a,b)=>a-b);return s[Math.floor(s.length/2)];};
const stats=values=>values.length?{n:values.length,min:Math.min(...values),median:median(values),max:Math.max(...values)}:null;
const summary=matrix.map(([family,size,phase])=>{
 const rows=raw.filter(r=>r.repetition>=0&&r.family===family&&r.size===size&&r.phase===phase);
 const good=rows.filter(r=>r.exit_code===0&&!r.censored&&r.result);
 return {family,size,phase,failures:rows.filter(r=>!good.includes(r)).map(r=>({exit_code:r.exit_code,censored:r.censored,stderr:r.stderr})),outcomes:[...new Set(good.map(r=>r.result.outcome))],wall_ns:stats(good.map(r=>r.result.wall_ns).filter(n=>typeof n==='number')),peak_rss_bytes:stats(good.map(r=>r.peak_rss_bytes).filter(n=>typeof n==='number')),details:good[0]?.result??null};
});
writeFileSync(path.join(directory,'summary.json'),JSON.stringify(summary,null,2)+'\n');
const plot=summary.filter(r=>r.phase==='run'&&r.wall_ns);
if(plot.length&&!smoke){const width=960,height=90+plot.length*45,max=Math.max(...plot.map(r=>r.wall_ns.max)),scale=x=>500*x/max;
 const body=plot.map((r,i)=>{const y=65+i*45;return `<text x="12" y="${y+5}">${r.family}/${r.size} (${r.outcomes.join(',')})</text><rect x="340" y="${y-9}" width="${scale(r.wall_ns.median)}" height="18" fill="#285f49"/><path d="M${340+scale(r.wall_ns.min)},${y}H${340+scale(r.wall_ns.max)}" stroke="#111"/><text x="850" y="${y+5}">${(r.wall_ns.median/1e6).toFixed(2)} ms</text>`;}).join('');
 writeFileSync(path.join(directory,'run-times.svg'),`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" role="img"><title>Native recorded-run wall time: medians and observed min–max, five processes per case</title><rect width="100%" height="100%" fill="white"/><g font-family="sans-serif" font-size="13" fill="#111"><text x="12" y="24">Recorded-run wall time · n=5 · bar: median · line: observed min–max · linear scale</text>${body}<text x="340" y="${height-10}">0 — ${(max/1e6).toFixed(2)} ms; ResourceLimit is failed search, not throughput.</text></g></svg>`);
}
const criterion=path.join('target','criterion');
if(!smoke&&existsSync(criterion)){
 const copy=(from,to)=>{mkdirSync(to,{recursive:true});for(const e of readdirSync(from,{withFileTypes:true})){if(['base','change','report'].includes(e.name))continue;if(e.isDirectory())copy(path.join(from,e.name),path.join(to,e.name));else if(e.name.endsWith('.json'))copyFileSync(path.join(from,e.name),path.join(to,e.name));}};copy(criterion,path.join(directory,'criterion'));
}
const failed=summary.filter(r=>r.failures.length);console.log(`Completed ${matrix.length} cells; ${failed.length} cells contain process failures; retained in raw.json.`);
if(failed.length)process.exitCode=1;
