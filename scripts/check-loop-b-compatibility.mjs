// Verify actual pre-profile producer output, not a renamed current-build trace.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const [baselineRoot,destination]=process.argv.slice(2);
if(!baselineRoot||!destination||process.argv.length!==4)throw Error('Usage: node scripts/check-loop-b-compatibility.mjs BASELINE_CHECKOUT NEW_DIRECTORY');
const old=resolve(baselineRoot,'target/release/cli'),current=resolve('target/release/cli'),dir=resolve(destination);
mkdirSync(dir,{recursive:false});
const sha=b=>createHash('sha256').update(b).digest('hex');
function run(binary,args,input,codes=[0,3,6]){
 const p=spawnSync(binary,args,{input,encoding:'utf8',timeout:30000,maxBuffer:16*1024*1024});
 assert.ifError(p.error);assert.ok(codes.includes(p.status),`${args}: ${p.status}: ${p.stderr}`);return p.stdout;
}
const cases=[
 ['normal','examples/programs/arithmetic.graph',['--max-steps','4']],
 ['recurrence','examples/behaviour/oscillating.graph',['--max-steps','4','--detect-cycles','true','--strategy','seeded','--seed','42']],
 ['reflective','examples/programs/self-modify.graph',['--max-steps','4']],
 ['exhausted','examples/behaviour/oscillating.graph',['--max-steps','4','--max-work','0']],
 ['yielded','examples/practical/work-items.graph',['--max-steps','10','--batch-steps','2','--strategy','seeded','--seed','42']],
];
const rows=[];
for(const [name,path,args] of cases){
 const source=readFileSync(path,'utf8');
 const legacy=run(old,['export','-',...args],source);
 assert.equal(JSON.parse(legacy).settings.execution_profile,undefined);
 const migrated=run(current,['trace-migrate','-'],legacy,[0]);
 run(current,['trace-check','-'],migrated,[0]);
 const fresh=JSON.parse(run(current,['export','-',...args],source));
 const result=JSON.parse(migrated),original=JSON.parse(legacy);
 assert.equal(result.implementation,original.implementation);
 fresh.implementation=result.implementation;
 assert.deepEqual(result,fresh,'Migration differs from current reference execution');
 if(name==='yielded'){
  const continued=JSON.parse(run(current,['resume','-','--batch-steps','20'],migrated));
  const full=JSON.parse(run(current,['export','-','--max-steps','10','--strategy','seeded','--seed','42'],source));
  assert.deepEqual(continued,full);
 }
 const forged=structuredClone(original);forged.initial_sha256='0'.repeat(64);
 run(current,['trace-migrate','-'],JSON.stringify(forged),[2]);
 writeFileSync(resolve(dir,`${name}.legacy.json`),legacy,{flag:'wx'});
 writeFileSync(resolve(dir,`${name}.migrated.json`),migrated,{flag:'wx'});
 rows.push({name,source:path,source_sha256:sha(source),legacy_sha256:sha(legacy),migrated_sha256:sha(migrated),outcome:result.report?.outcome??result.status,events:result.events.length});
 console.log('verified',name);
}
writeFileSync(resolve(dir,'manifest.json'),JSON.stringify({schema:'loop-b-migration-evidence-v1',baseline_commit:'b650e4f3b55370b9fcc533f2d91cadc02819a689',baseline_binary_sha256:sha(readFileSync(old)),candidate_binary_sha256:sha(readFileSync(current)),collector_sha256:sha(readFileSync('scripts/check-loop-b-compatibility.mjs')),rows},null,2)+'\n',{flag:'wx'});
