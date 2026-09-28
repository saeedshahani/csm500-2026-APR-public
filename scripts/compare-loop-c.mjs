import fs from 'node:fs';import path from 'node:path';import {execFileSync} from 'node:child_process';
import {compileMonths,monthResults} from '../platform/months.mjs';import {native} from '../platform/worker.mjs';import {hash} from '../platform/store.mjs';
const destination=process.argv[2];if(!destination)throw Error('Usage: node scripts/compare-loop-c.mjs NEW_RESULT.json');if(fs.existsSync(destination))throw Error('Refusing overwrite');
const csv=execFileSync('unzip',['-p','output/s6-datasets/shoppers.zip','online_shoppers_intention.csv'],{encoding:'utf8',maxBuffer:4000000}),lines=csv.trim().split(/\r?\n/);
const binary=path.resolve('target/release/cli');const report={schema:'loop-c-alternatives-v1',binary_sha256:hash(fs.readFileSync(binary)),criterion:'complete output, not a prefix; 10 second per-process diagnostic ceiling',samples:[]};
for(const n of [10,100,1000])for(const profile of ['indexed-v2','framed-v3','joined-v4']){
const source=compileMonths(lines.slice(0,n+1).join('\n')).source,t=performance.now();const r=await native(binary,['export','-','--max-steps',String(n+1),'--execution-profile',profile,'--max-work','10000000','--max-candidates','20000','--checkpoint-interval','20000'],source,{large:true,deadline:10000,maximum:128*1024*1024});
const sample={n,profile,elapsed_ms:performance.now()-t,code:r.code,cause:r.cause,stderr:r.stderr,source_sha256:hash(source)};
if(r.stdout.length&&r.code===0){const trace=JSON.parse(r.stdout);sample.commits=trace.events.length;sample.oracle_verified=monthResults(trace).complete;sample.trace_sha256=hash(r.stdout);}report.samples.push(sample);fs.writeFileSync(destination,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(sample));}
