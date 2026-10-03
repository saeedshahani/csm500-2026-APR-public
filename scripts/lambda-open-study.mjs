// Immutable, source-pinned descriptive costs; see lambda-encoding.md §8.4.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,existsSync,mkdirSync} from 'node:fs';
import {spawnSync,execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {sourceFiles} from './research-source-files.mjs';
const [mode,dir,...flags]=process.argv.slice(2);
assert.ok(['collect','verify'].includes(mode)&&dir,'collect|verify DIR [--live]');
const sha=x=>createHash('sha256').update(x).digest('hex');
const binary='target/release/examples/lambda_open_probe';
function check(r) {
 assert.equal(r.schema,'lambda-open-cost-v1');assert.ok(['closed-cbn-v1','open-cbn-v1'].includes(r.profile));
 assert.ok([0,4,16].includes(r.depth));assert.equal(r.terminal,true);assert.equal(r.beta,r.depth);
 assert.equal(r.commits,3*r.depth+Number(r.profile==='open-cbn-v1'));
 assert.equal(r.final_atoms-r.initial_atoms,8*r.depth);
 assert.equal(r.readback,r.profile==='open-cbn-v1'?'database':'(λ.0)');
 for(const key of ['compile_ns','execution_ns','readback_ns'])assert.ok(Number.isFinite(r[key])&&r[key]>=0);
}
function run(profile,depth,timed) {
 const args=[profile,String(depth)];
 const p=timed?spawnSync('/usr/bin/time',[os.platform()==='darwin'?'-l':'-v',binary,...args],{encoding:'utf8',timeout:120000}):spawnSync(binary,args,{encoding:'utf8',timeout:120000});
 assert.equal(p.status,0,p.error?.message??p.stderr);const result=JSON.parse(p.stdout);check(result);
 if(!timed)return result;
 const rss=os.platform()==='darwin'?Number(p.stderr.match(/(\d+)\s+maximum resident set size/)[1]):1024*Number(p.stderr.match(/Maximum resident set size \(kbytes\): (\d+)/)[1]);
 assert.ok(rss>0);return {result,peak_rss_bytes:rss,raw_time:p.stderr};
}
if(mode==='collect') {
 assert.ok(!existsSync(dir),'never overwrite a collection');
 assert.ok(['darwin','linux'].includes(os.platform()));
 const paths=['Cargo.lock','rust-toolchain.toml','crates/parser/src/lambda.rs','crates/cli/examples/lambda_open_probe.rs','crates/cli/tests/lambda_open.rs','examples/lambda/open/evidence.json','scripts/lambda-open-study.mjs','scripts/research-source-files.mjs'];
 // Include the complete native dependency implementation, not only the adapter.
 paths.push(...sourceFiles(['crates/engine/src','crates/graph-core/src']));
 mkdirSync(dir,{recursive:true});
 const sources=Object.fromEntries(paths.sort().map(p=>[p,readFileSync(p,'utf8')]));
 const bytes=JSON.stringify(sources);writeFileSync(`${dir}/sources.json`,bytes+'\n');
 const rows=[];
 for(const profile of ['closed-cbn-v1','open-cbn-v1'])for(const depth of [0,4,16])for(let repetition=1;repetition<=3;repetition++){
  const start=performance.now();const row=run(profile,depth,true);rows.push({...row,repetition,wall_ms:performance.now()-start});
 }
 const report={schema:'lambda-open-study-v1',date:new Date().toISOString(),revision:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),source_sha256:sha(bytes+'\n'),binary_sha256:sha(readFileSync(binary)),compiler:execFileSync('rustc',['--version'],{encoding:'utf8'}).trim(),hardware:{platform:os.platform(),release:os.release(),arch:os.arch(),cpu:os.cpus()[0].model,memory_bytes:os.totalmem()},rows};
 writeFileSync(`${dir}/report.json`,JSON.stringify(report,null,2)+'\n');
}
const sourcesBytes=readFileSync(`${dir}/sources.json`),sources=JSON.parse(sourcesBytes),report=JSON.parse(readFileSync(`${dir}/report.json`));
assert.equal(report.schema,'lambda-open-study-v1');assert.equal(sha(sourcesBytes),report.source_sha256);assert.equal(report.rows.length,18);
const cells=new Set();
for(const row of report.rows){check(row.result);assert.ok(row.wall_ms>0&&row.peak_rss_bytes>0);cells.add(`${row.result.profile}:${row.result.depth}:${row.repetition}`);}
assert.equal(cells.size,18);
const evidence=JSON.parse(sources['examples/lambda/open/evidence.json']);assert.equal(evidence.cases.length,450);assert.equal(evidence.cases.reduce((s,r)=>s+r.transitions.length,0),704);
if(flags.includes('--live'))for(const profile of ['closed-cbn-v1','open-cbn-v1'])for(const depth of [0,4,16]){
 const actual=run(profile,depth,false),expected=report.rows.find(r=>r.result.profile===profile&&r.result.depth===depth).result;
 for(const key of ['source','commits','beta','terminal','readback','initial_atoms','final_atoms'])assert.deepEqual(actual[key],expected[key],key);
}
console.log(`Verified ${report.rows.length} process measurements and ${evidence.cases.length} source cases (${report.source_sha256}).`);
