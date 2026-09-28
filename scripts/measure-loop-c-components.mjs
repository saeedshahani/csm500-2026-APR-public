// Serial component observations, separate from the frozen complete-platform jobs.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {compileMonths,monthResults} from '../platform/months.mjs';
import {native} from '../platform/worker.mjs';
import {hash} from '../platform/store.mjs';
const destination=process.argv[2];if(!destination||fs.existsSync(destination))throw Error('Supply a new result JSON path');
const csv=execFileSync('unzip',['-p','output/s6-datasets/shoppers.zip','online_shoppers_intention.csv'],{encoding:'utf8',maxBuffer:4000000});
const binary=path.resolve('target/release/cli'),samples=[];
const args=['export','-','--max-steps','13000','--execution-profile','joined-v4','--max-work','10000000','--max-candidates','20000','--checkpoint-interval','20000','--strategy','seeded','--seed','42','--detect-cycles','true'];
const report={schema:'loop-c-components-v1',binary_sha256:hash(fs.readFileSync(binary)),script_sha256:hash(fs.readFileSync(new URL(import.meta.url))),csv_sha256:hash(csv),scope:'Single serial observation per policy. Export includes loading, running, recording and serialization; verification is a separate process. Neither isolates rewrite-only time. No API batching, polling, storage or browser work.',args,samples};
for(const policy of ['fixed','guarded']){
 const source=compileMonths(csv,policy).source,t=performance.now();
 const run=await native(binary,args,source,{large:true,maximum:128*1024*1024,deadline:120000});
 const sample={policy,export_ms:performance.now()-t,source_sha256:hash(source),code:run.code,cause:run.cause};samples.push(sample);
 fs.writeFileSync(destination,JSON.stringify(report,null,2)+'\n');assert.equal(run.code,0,run.stderr);assert.equal(run.cause,null);
 const v=performance.now(),checked=await native(binary,['trace-check','-'],run.stdout,{large:true,deadline:120000});
 sample.verification_ms=performance.now()-v;assert.equal(checked.code,0,checked.stderr);
 const trace=JSON.parse(run.stdout);assert.equal(monthResults(trace,csv).complete,true);
 Object.assign(sample,{trace_sha256:hash(run.stdout),trace_bytes:run.stdout.length,commits:trace.events.length,oracle_verified:true,replay_verified:true});
 fs.writeFileSync(destination,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(sample));
}
