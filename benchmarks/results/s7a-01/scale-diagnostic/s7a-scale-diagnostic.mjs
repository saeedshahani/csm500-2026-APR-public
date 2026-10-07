import {spawn,execFileSync} from 'node:child_process';
import {writeFileSync} from 'node:fs';
const results=[];
for(const [name,binary] of [
  ['historical-rebuilt','/private/tmp/s7a-scale-baseline.xg59mL/target/release/examples/scale_probe'],
  ['current','./target/release/examples/scale_probe'],
]) {
  const row=await new Promise(resolve=>{
    const p=spawn('/usr/bin/time',['-l',binary,'fixed','100000','pipeline-framed','100000','42'],{detached:true});
    let stdout='',stderr='',peak=0,cause=null;
    const stop=why=>{cause=why;try{process.kill(-p.pid,'SIGKILL');}catch{}};
    const timer=setTimeout(()=>stop('deadline'),600000);
    const sampler=setInterval(()=>{
      const data=execFileSync('ps',['-axo','pgid=,rss='],{encoding:'utf8'}).trim().split('\n').map(s=>s.trim().split(/\s+/).map(Number));
      peak=Math.max(peak,data.filter(([group])=>group===p.pid).reduce((n,[,rss])=>n+rss*1024,0));
      if(peak>6*1024**3)stop('sampled process-group RSS ceiling exceeded');
    },500);
    p.stdout.on('data',b=>stdout+=b);p.stderr.on('data',b=>stderr+=b);
    p.on('close',(code,signal)=>{clearTimeout(timer);clearInterval(sampler);resolve({name,code,signal,peak,cause,stdout,stderr});});
  });
  results.push(row);writeFileSync('output/s7a-scale-diagnostic.json',JSON.stringify(results,null,2)+'\n');
  console.log(name,row.code,row.cause,row.peak);
}
