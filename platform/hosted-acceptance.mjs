// Explicit, small production acceptance. Creates disposable test identities only;
// passwords/tokens stay in memory. Immutable experiment records are retained.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {randomBytes,randomUUID} from 'node:crypto';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
import {initializeApp,deleteApp} from 'firebase-admin/app';
import {getAuth} from 'firebase-admin/auth';
import {target} from './deployment-target.mjs';
import {hash} from './store.mjs';

const args=process.argv.slice(2);
if(args.length!==2||args[0]!=='--project')throw Error('Explicit --project reflective-graph-f1cd4 required');
const config=target(args[1]),P=config.project_id;
if(!config.deployment_approved)throw Error('Deployment approval required');
const require=createRequire(import.meta.url),login=require('firebase-tools/lib/auth');
const account=login.getProjectDefaultAccount(process.cwd());if(!account)throw Error('Firebase CLI login required');
login.setActiveAccount({project:P},account);
const app=initializeApp({projectId:P,credential:{getAccessToken:async()=>({access_token:await require('firebase-tools/lib/apiv2').getAccessToken(),expires_in:300})}},'hosted-acceptance');
const auth=getAuth(app),users=[],results=[],run=randomUUID();
const base=`https://${P}.web.app/api`,direct=`https://graph-api-${config.project_number}.europe-west2.run.app`;
const key='AIzaSyDFDb9JDYw22vpi2V204Li3b17QKPwGENE'; // Public Firebase web-app identifier.
const pass=name=>{results.push(name);console.log(`PASS ${name}`);};
let queuePaused=false;
async function queue(paused){
  const r=await fetch(`https://cloudtasks.googleapis.com/v2/projects/${P}/locations/europe-west2/queues/graph-jobs:${paused?'pause':'resume'}`,{method:'POST',headers:{Authorization:`Bearer ${await require('firebase-tools/lib/apiv2').getAccessToken()}`,'x-goog-user-project':P,'Content-Type':'application/json'},body:'{}'});
  assert.equal(r.status,200,'Queue control failed');queuePaused=paused;
  const state=await r.json();assert.equal(state.state,paused?'PAUSED':'RUNNING');
  // Provider control-plane changes need not stop already propagating dispatches.
  if(paused)await delay(30000);
}
async function user(verified){
  const password=randomBytes(32).toString('base64url'),email=`acceptance-${randomUUID()}@example.invalid`;
  const created=await auth.createUser({email,password,emailVerified:verified});users.push(created.uid);
  const r=await fetch(`https://identitytoolkit.googleapis.com/v1/accounts:signInWithPassword?key=${key}`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email,password,returnSecureToken:true})});
  assert.equal(r.status,200,'Synthetic password sign-in failed');const signed=await r.json();
  return {uid:created.uid,token:signed.idToken};
}
async function call(user,path,data,{raw=false,retry=false}={}){
  for(let i=0;i<30;i++){
    const response=await fetch(base+path,{method:data===undefined?'GET':'POST',headers:{...(user?{Authorization:`Bearer ${user.token}`}:{ }),...(data===undefined?{}:{'Content-Type':'application/json'})},...(data===undefined?{}:{body:JSON.stringify(data)}),redirect:'error',signal:AbortSignal.timeout(60000)});
    const text=await response.text();
    if(response.status===503&&(data===undefined||retry)&&i<29){await delay(2000);continue;}
    return {status:response.status,headers:response.headers,body:raw?text:JSON.parse(text)};
  }
}
async function wait(user,id){
  for(let i=0;i<60;i++){const r=await call(user,`/v1/jobs/${id}`);assert.equal(r.status,200);if(!['queued','running'].includes(r.body.status))return r.body;await delay(3000);}
  throw Error('Hosted job did not reach a bounded endpoint');
}
try{
  assert.equal((await call(null,'/v1/health')).status,401);
  assert.equal((await fetch(direct+'/_work',{method:'POST',body:'{}'})).status,401);
  pass('unauthenticated API and internal dispatch denied');
  const owner=await user(true),stranger=await user(true),unverified=await user(false);
  assert.equal((await call(unverified,'/v1/health')).status,403);
  const health=await call(owner,'/v1/health');assert.equal(health.status,200,JSON.stringify(health.body));assert.equal(health.body.uid,owner.uid);
  assert.equal(health.body.mode,'firebase-production');assert.match(health.headers.get('cache-control'),/no-store/);
  pass('real password identity, verified-email gate and private response caching');
  for(const family of ['routing','branching']){
    const request=JSON.parse(readFileSync(`examples/practical/workflow-${family}.request.json`,'utf8'));
    assert.equal((await call(null,'/v1/workflows/check',request)).status,401);
    assert.equal((await call(unverified,'/v1/workflows/check',request)).status,403);
    const certified=await call(owner,'/v1/workflows/check',request,{retry:true});
    assert.equal(certified.status,200,JSON.stringify(certified.body));assert.equal(certified.body.verdict,'certified');
    const prefix=await call(owner,'/v1/workflows/run',{...request,max_steps:0},{retry:true});
    assert.equal(prefix.status,200,JSON.stringify(prefix.body));assert.equal(prefix.body.outcome,'StepLimit');
    assert.ok(prefix.body.report.unfinished>0);
    const completed=await call(owner,'/v1/workflows/run',request,{retry:true});
    assert.equal(completed.status,200,JSON.stringify(completed.body));assert.equal(completed.body.outcome,'Completed');
    assert.equal(completed.body.engine_sha256,health.body.engine_sha256);
    assert.equal(completed.body.report.unfinished,0);
    assert.equal(completed.body.report.records.length,family==='routing'?4:6);
    assert.ok(BigInt(completed.body.trace.events.length)<=BigInt(completed.body.completion_step_bound));
    const replay=spawnSync('target/release/cli',['trace-check','-'],{input:JSON.stringify(completed.body.trace),encoding:'utf8',timeout:30000});
    assert.equal(replay.status,0,replay.stderr);
    const invalid=structuredClone(request);invalid.contract.weights.received='0';
    const rejected=await call(owner,'/v1/workflows/check',invalid,{retry:true});
    assert.equal(rejected.status,200,JSON.stringify(rejected.body));assert.equal(rejected.body.verdict,'unsupported');
    assert.equal(rejected.body.refuted,false);
    pass(`hosted ${family}: contract, bounded prefix, completion, rejection and native replay`);
  }
  const created=await call(owner,'/v1/projects',{name:`Hosted acceptance ${run}`});assert.equal(created.status,201,JSON.stringify(created.body));
  const project=created.body,route=`/v1/projects/${project.id}`;
  assert.equal((await call(stranger,route+'/versions')).status,404);
  assert.equal((await call(owner,route+'/versions',{name:'invalid',source:'not a graph'})).status,422);
  const document=JSON.stringify({schema:'work-items-v1',policy:'guarded_enable',items:[{id:'one',status:'pending'},{id:'two',status:'processed'}]});
  const data=await call(owner,route+'/datasets',{name:'work items',document});assert.equal(data.status,201,JSON.stringify(data.body));
  const version=await call(owner,route+'/versions',{name:'compiled dataset',dataset_id:data.body.id});assert.equal(version.status,201,JSON.stringify(version.body));
  pass('project isolation, rejected invalid source and immutable dataset compilation');
  const submission={version_id:version.body.id,idempotency_key:run,settings:{deadline_ms:30000}};
  const queued=await call(owner,route+'/jobs',submission,{retry:true});assert.equal(queued.status,202,JSON.stringify(queued.body));
  assert.equal((await call(owner,route+'/jobs',submission,{retry:true})).body.id,queued.body.id);
  const done=await wait(owner,queued.body.id);assert.equal(done.status,'completed',JSON.stringify(done));assert.equal(done.outcome,'NormalForm');assert.equal(done.commits,2);
  const trace=await call(owner,`/v1/jobs/${done.id}/trace`,undefined,{raw:true});assert.equal(trace.status,200);assert.equal(hash(trace.body),done.trace_sha256);
  assert.equal(trace.headers.get('x-content-sha256'),done.trace_sha256);
  const checked=spawnSync('target/release/cli',['trace-check','-'],{input:trace.body,encoding:'utf8',timeout:30000});assert.equal(checked.status,0,checked.stderr);
  const report=await call(owner,`/v1/jobs/${done.id}/report`);assert.equal(report.body.commits,2);
  assert.equal((await call(owner,`/v1/jobs/${done.id}/events?limit=1`)).body.next_offset,1);
  assert.equal((await call(stranger,`/v1/jobs/${done.id}/trace`)).status,404);
  pass('Cloud Tasks execution, duplicate submission, paginated report and independent native replay');
  const shared=await call(owner,route+'/members',{uid:stranger.uid,role:'viewer'});assert.equal(shared.status,200);
  assert.equal((await call(stranger,route+'/jobs',submission)).status,403);
  assert.equal((await call(stranger,`/v1/jobs/${done.id}/trace`)).status,200);
  await call(owner,route+'/members',{uid:stranger.uid,role:null});
  assert.equal((await call(stranger,`/v1/jobs/${done.id}/trace`)).status,404);
  pass('viewer read-only access and immediate membership revocation');
  const oscillator=await call(owner,route+'/versions',{name:'bounded oscillator',source:readFileSync('examples/behaviour/oscillating.graph','utf8')});assert.equal(oscillator.status,201);
  // Hold delivery explicitly: a fast finished job correctly refuses a late pause.
  // Run only during the authorised pilot acceptance window, not concurrent use.
  await queue(true);
  const second=await call(owner,route+'/jobs',{version_id:oscillator.body.id,idempotency_key:run+'-osc',settings:{max_steps:30,deadline_ms:30000}},{retry:true});assert.equal(second.status,202);
  const paused=await call(owner,`/v1/jobs/${second.body.id}/pause`,{},{retry:true});assert.equal(paused.status,202,JSON.stringify(paused.body));
  const endpoint=await wait(owner,second.body.id);assert.equal(endpoint.status,'paused',JSON.stringify(endpoint));
  assert.equal((await call(owner,`/v1/jobs/${second.body.id}/resume`,{},{retry:true})).status,202);
  await queue(false);
  const continued=await wait(owner,second.body.id);assert.equal(continued.status,'completed',JSON.stringify(continued));assert.equal(continued.outcome,'StepLimit');assert.equal(continued.commits,30);
  assert.ok(continued.attempts.length>=3);
  const history=await call(owner,`/v1/jobs/${continued.id}/trace`,undefined,{raw:true});
  assert.equal(spawnSync('target/release/cli',['trace-check','-'],{input:history.body,timeout:30000}).status,0);
  pass('queued pause/resume, multi-request continuation and one verified logical history');
  await queue(true);
  const third=await call(owner,route+'/jobs',{version_id:oscillator.body.id,idempotency_key:run+'-cancel',settings:{max_steps:1000,deadline_ms:30000}},{retry:true});assert.equal(third.status,202);
  assert.equal((await call(owner,`/v1/jobs/${third.body.id}/cancel`,{},{retry:true})).status,202);
  assert.equal((await wait(owner,third.body.id)).status,'cancelled');
  await queue(false);
  pass('hosted cancellation remains distinct from convergence');
  await auth.updateUser(owner.uid,{disabled:true});assert.equal((await call(owner,'/v1/health')).status,401);
  pass('disabled identity rejected by production verifier');
  mkdirSync('output/hosted',{recursive:true});
  writeFileSync('output/hosted/acceptance.json',JSON.stringify({schema:'reflective-hosted-acceptance-v1',project:P,checked_at:new Date().toISOString(),run,project_id:project.id,job_ids:[done.id,continued.id,third.body.id],trace_sha256:done.trace_sha256,engine_sha256:done.engine_sha256,results,limitations:['Disposable accounts were administrator-verified; inbox delivery is not tested.','Immutable test experiment records are retained; disposable identities are removed.']},null,2));
}finally{
  if(queuePaused)await queue(false);
  for(const uid of users)await auth.deleteUser(uid);
  await deleteApp(app);
}
