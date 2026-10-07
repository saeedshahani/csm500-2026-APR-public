// Functional evidence, not a throughput benchmark. Never overwrite a collection.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {spawnSync,execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {gzipSync,gunzipSync} from 'node:zlib';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

export function compareWorkflowResults(current,recorded) {
  const strip=result=>{
    const {implementation,...trace}=result.trace;
    assert.match(implementation,/^engine-0\.1\.0-source-sha256=[a-f0-9]{64}$/);
    return {...result,trace};
  };
  assert.deepEqual(strip(current),strip(recorded));
  return {recorded:recorded.trace.implementation,current:current.trace.implementation};
}

if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href) {
const [mode,directory]=process.argv.slice(2);
assert.ok(['collect','verify'].includes(mode)&&directory,'collect|verify DIRECTORY [--native|--live]');
const flags=process.argv.slice(4);
assert.ok(flags.every(f=>['--native','--live'].includes(f))&&flags.length<=1&&(mode==='verify'||flags.length===0),'unknown or incompatible verification option');
const sha=b=>createHash('sha256').update(b).digest('hex');
const binary=resolve('target/release/cli');
function call(args,input){const r=spawnSync(binary,args,{input,encoding:'utf8',timeout:30000,maxBuffer:16*1024*1024});return {code:r.status,signal:r.signal,error:r.error?.message??null,stdout:r.stdout??'',stderr:r.stderr??''};}
function verifyCell(cell,native=false){
  assert.equal(cell.check.code,0,cell.check.stderr); assert.equal(cell.run.code,0,cell.run.stderr); assert.equal(cell.replay.code,0,cell.replay.stderr);
  const initial=JSON.parse(cell.check.stdout),result=JSON.parse(cell.run.stdout),report=result.report;
  assert.equal(initial.verdict,'certified'); assert.equal(result.outcome,'Completed'); assert.equal(report.unfinished,0);
  assert.equal(report.records.length,cell.count*(cell.family==='branching'?3:1));
  for(const old of initial.records){const post=report.records.find(r=>r.ticket===old.ticket); assert.ok(post);for(const k of ['payload_identity','payload','identity_link'])assert.equal(post[k],old[k]);assert.equal(post.stage,'archive');}
  assert.ok(BigInt(result.trace.events.length)<=BigInt(result.completion_step_bound));
  assert.equal(BigInt(result.completion_step_bound),BigInt(cell.burst+1)*BigInt(initial.work_potential));
  assert.equal(initial.all_orders_transaction_bound===null,cell.policy==='persistent');
  if(native){const replay=call(['trace-check','-'],JSON.stringify(result.trace));assert.equal(replay.code,0,replay.stderr);const check=call(['workflow-check','-'],JSON.stringify(cell.request));assert.equal(check.code,0);assert.deepEqual(JSON.parse(check.stdout),initial);}
  return {family:cell.family,count:cell.count,burst:cell.burst,policy:cell.policy,commits:result.trace.events.length,records:report.records.length,bound:result.completion_step_bound};
}
if(mode==='collect'){
  assert.ok(!existsSync(directory),'collection already exists');mkdirSync(directory,{recursive:true});
  const paths=[...new Set(execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n'))].filter(p=>existsSync(p)&&(/^(Cargo\.(toml|lock)|rust-toolchain.toml)$/.test(p)||/^crates\/.*\.(rs|toml|lean)$/.test(p)||p==='scripts/workflow-study.mjs'||p==='benchmarks/PROTOCOL.md')).sort();
  const sources=gzipSync(JSON.stringify({files:Object.fromEntries(paths.map(p=>[p,readFileSync(p).toString('base64')]))}));writeFileSync(resolve(directory,'sources.json.gz'),sources);
  const manifest={schema:'workflow-study-v1',complete:false,date:new Date().toISOString(),source_bundle_sha256:sha(sources),binary_sha256:sha(readFileSync(binary)),commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),node:process.version,platform:process.platform,rows:[],failure:null};
  try{
    const lean=spawnSync(process.env.WORKFLOW_LEAN??'lean',['crates/engine/proofs/WorkflowContracts.lean'],{encoding:'utf8',timeout:30000});
    assert.equal(lean.status,0,lean.error?.message??lean.stdout);assert.ok(!/sorryAx|warning|error/i.test(lean.stdout+lean.stderr));
    const axioms=[...lean.stdout.matchAll(/depends on axioms: \[([^\]]*)\]/g)];assert.equal(axioms.length,7);for(const a of axioms)assert.ok(a[1].split(', ').every(x=>['propext','Quot.sound','Classical.choice'].includes(x)));
    const proof={output:lean.stdout,version:spawnSync(process.env.WORKFLOW_LEAN??'lean',['--version'],{encoding:'utf8'}).stdout};const proofBytes=JSON.stringify(proof,null,2);writeFileSync(resolve(directory,'proof.json'),proofBytes);manifest.proof_sha256=sha(proofBytes);
    for(const family of ['routing','branching'])for(const count of [0,1,4,12])for(const burst of [0,1,2])for(const policy of ['finite','persistent']){
      const example=call(['workflow-example',family,String(count),policy]);assert.equal(example.code,0,example.stderr);
      const request=JSON.parse(example.stdout);request.meta_burst=burst;request.max_steps=1000;
      const check=call(['workflow-check','-'],JSON.stringify(request)),run=call(['workflow-run','-'],JSON.stringify(request));
      const result=run.code===0?JSON.parse(run.stdout):null;
      const replay=result?call(['trace-check','-'],JSON.stringify(result.trace)):null;
      const cell={family,count,burst,policy,request,check,run,replay};
      const filename=`${family}-${count}-${burst}-${policy}.json.gz`,bytes=gzipSync(JSON.stringify(cell));writeFileSync(resolve(directory,filename),bytes);
      const row=verifyCell(cell);manifest.rows.push({...row,filename,sha256:sha(bytes)});console.log(JSON.stringify(row));
    }
    manifest.complete=true;
  }catch(e){manifest.failure=String(e.stack);process.exitCode=1;}
  writeFileSync(resolve(directory,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
}else{
  const m=JSON.parse(readFileSync(resolve(directory,'manifest.json')));assert.equal(m.complete,true);assert.equal(m.rows.length,48);
  assert.equal(sha(readFileSync(resolve(directory,'sources.json.gz'))),m.source_bundle_sha256);assert.equal(sha(readFileSync(resolve(directory,'proof.json'))),m.proof_sha256);
  if(process.argv.includes('--native'))assert.equal(sha(readFileSync(binary)),m.binary_sha256,'native verification requires the recorded binary');
  for(const row of m.rows){
    const bytes=readFileSync(resolve(directory,row.filename));assert.equal(sha(bytes),row.sha256);
    const cell=JSON.parse(gunzipSync(bytes)),live=flags.includes('--live');
    const actual=verifyCell(cell,flags.includes('--native')||live);const{filename,sha256,...expected}=row;assert.deepEqual(actual,expected);
    if(live){
      const run=call(['workflow-run','-'],JSON.stringify(cell.request));assert.equal(run.code,0,run.stderr);
      const result=JSON.parse(run.stdout),replay=call(['trace-check','-'],JSON.stringify(result.trace));
      assert.equal(replay.code,0,replay.stderr);verifyCell({...cell,run,replay});
      // Both unmodified traces have passed replay before comparing. Only producer
      // implementation identity differs; retain both rather than forge old bytes.
      const producers=compareWorkflowResults(result,JSON.parse(cell.run.stdout));
      console.log(JSON.stringify({cell:row.filename,producers}));
    }
  }
  console.log(`Verified ${m.rows.length} cells, ${m.rows.reduce((s,r)=>s+r.commits,0)} committed transitions; ${flags.includes('--live')?'current-build compatibility checked (only producer implementation identity excluded)':flags.includes('--native')?'recorded-binary replay repeated':'native replay not repeated'}.`);
}
}
