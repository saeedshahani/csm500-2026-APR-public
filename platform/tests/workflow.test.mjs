import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,mkdtempSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {start} from '../server.mjs';

test('authenticated workflows certify, complete, return replayable traces and keep limits distinct',async t=>{
  const directory=mkdtempSync(join(tmpdir(),'workflow-api-'));
  const binary=resolve('target/release/cli'),token='workflow_test_token_012345678901234567890';
  const app=await start({directory,binary,token,background:false});
  t.after(async()=>{await app.close();rmSync(directory,{recursive:true,force:true});});
  const request=JSON.parse(readFileSync('examples/practical/workflow-branching.request.json'));
  const send=(action,body,auth=token)=>fetch(`${app.url}/v1/workflows/${action}`,{method:'POST',headers:{Authorization:`Bearer ${auth}`,'Content-Type':'application/json'},body:JSON.stringify(body)});
  assert.equal((await send('check',request,'bad')).status,401);
  assert.equal((await send('run',{...request,max_steps:1001})).status,400);
  const checked=await send('check',request);assert.equal(checked.status,200);assert.equal((await checked.json()).verdict,'certified');
  const zero=await (await send('run',{...request,max_steps:0})).json();assert.equal(zero.outcome,'StepLimit');assert.equal(zero.report.unfinished,2);
  const result=await (await send('run',request)).json();assert.equal(result.outcome,'Completed');assert.equal(result.report.records.length,6);assert.equal(result.report.unfinished,0);
  const replay=spawnSync(binary,['trace-check','-'],{input:JSON.stringify(result.trace),encoding:'utf8'});assert.equal(replay.status,0,replay.stderr);
  const invalid=structuredClone(request);invalid.contract.weights.split='4';
  const rejected=await (await send('check',invalid)).json();assert.equal(rejected.verdict,'unsupported');assert.equal(rejected.refuted,false);
  assert.equal((await send('check',{...request,forged:true})).status,400);
});
