import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
import {compileMonths,monthResults,monthCsv,MONTH_HEADER} from '../months.mjs';
import {start} from '../server.mjs';
import {Store,hash} from '../store.mjs';
import {settings} from '../worker.mjs';
const csv=MONTH_HEADER+'\n0,0,0,0,1,0,0.2,0.2,0,0,June,1,1,1,1,Returning_Visitor,FALSE,FALSE\n';
const binary=path.resolve('target/release/cli'),token='month_tests_local_token_012345678901234';
test('large allowances require operator selection and storage remains bounded',async t=>{
 assert.throws(()=>settings({max_steps:20000,execution_profile:'joined-v4'}),/max_steps/);
 assert.equal(settings({max_steps:20000,execution_profile:'joined-v4'},true).max_steps,20000);
 const directory=fs.mkdtempSync(path.join(os.tmpdir(),'calendar-limits-'));
 const store=new Store(directory,{large:true});
 t.after(async()=>{await store.close();fs.rmSync(directory,{recursive:true,force:true});});
 const digest=hash('quota-test'),file=path.join(directory,'blobs',digest);
 fs.writeFileSync(file,'');fs.truncateSync(file,129*1024*1024);
 assert.throws(()=>store.read(digest),/Artefact too large/);
 fs.truncateSync(file,1024*1024*1024);
 assert.throws(()=>store.put('x'),e=>e.status===507&&e.code==='storage_limit');
});
function run(source,extra=[]){const r=spawnSync(binary,['export','-','--max-steps','20','--execution-profile','joined-v4',...extra],{input:source,encoding:'utf8',maxBuffer:16000000});assert.ok([0,3].includes(r.status),r.stderr);return JSON.parse(r.stdout);}
test('calendar join preserves payload, changes endpoints, and distinguishes graph normal form from useful output',()=>{
 for(const policy of ['fixed','guarded','disabled']){
  const trace=run(compileMonths(csv,policy).source),result=monthResults(trace);
  assert.equal(result.complete,policy!=='disabled');assert.equal(trace.events.length,policy==='guarded'?2:policy==='fixed'?1:0);
  if(result.complete){assert.equal(monthCsv(result),MONTH_HEADER+',MonthNumber\n'+csv.trim().split('\n')[1]+',06\n');assert.equal(trace.events.at(-1).fresh[trace.events.at(-1).footprint.produced[0]].kind.Record.label,'calendar-month');}
 }
 const bad=compileMonths(csv).source.replace('MonthNumber:06','MonthNumber:07');
 const trace=run(bad);assert.equal(trace.report.outcome,'NormalForm');assert.equal(monthResults(trace).complete,false);assert.throws(()=>monthCsv(monthResults(trace)),/oracle/);
 const missing=compileMonths(csv).source.replace(/atom map_5 = [^\n]+\n/,'');assert.equal(monthResults(run(missing)).complete,false);
 const twice=csv+csv.trim().split('\n')[1]+'\n',duplicated=run(compileMonths(twice).source);
 const payloads=Object.values(duplicated.initial.atoms).filter(a=>a.kind.Constructor?.startsWith('Session:'));
 payloads[1].kind.Constructor=payloads[0].kind.Constructor;
 assert.equal(monthResults(duplicated,twice).complete,false,'duplicate row identities cannot substitute for full coverage');
 for(const input of ['broken',csv.replace('June','Unknown'),csv.replace(',FALSE,FALSE',',FALSE'),csv.replace(',0.2,0.2,',',1.2,0.2,')])assert.throws(()=>compileMonths(input));
});
test('authenticated larger route imports, pages, exports, rejects partial results and survives restart',async t=>{
 const directory=fs.mkdtempSync(path.join(os.tmpdir(),'calendar-api-'));let app=await start({directory,binary,token,large:true});
 t.after(async()=>{await app.close();fs.rmSync(directory,{recursive:true,force:true});});
 const request=async(route,data,raw=false,auth=true)=>{const r=await fetch(app.url+route,{method:data===undefined?'GET':'POST',headers:{...(auth?{Authorization:`Bearer ${token}`} :{}),...(data===undefined?{}:{'Content-Type':'application/json'})},...(data===undefined?{}:{body:JSON.stringify(data)})});return {status:r.status,body:raw?await r.text():await r.json()};};
 const p=(await request('/v1/projects',{name:'months'})).body;
 assert.equal((await request(`/v1/projects/${p.id}/month-imports`,{name:'bad',csv:'bad',policy:'fixed'})).status,422);
 const v=(await request(`/v1/projects/${p.id}/month-imports`,{name:'calendar',csv,policy:'guarded'})).body;assert.equal(v.application.rows,1);
 const submitted=await request(`/v1/projects/${p.id}/jobs`,{version_id:v.id,idempotency_key:'calendar',settings:{execution_profile:'joined-v4',max_steps:20000,max_candidates:20000,max_work:10000000,deadline_ms:120000,detect_cycles:true}});assert.equal(submitted.status,202);
 const route='/v1/jobs/'+submitted.body.id;let j;
 for(let i=0;i<200;i++){j=(await request(route)).body;if(j.status==='completed')break;await delay(30);}assert.equal(j.status,'completed',JSON.stringify(j));
 const r=await request(route+'/month-results?limit=1');assert.equal(r.body.complete,true);assert.equal(r.body.total,1);assert.equal(r.body.next_offset,null);assert.equal(r.body.items[0].month_number,'06');
 assert.equal((await request(route+'/month-results?limit=101')).status,400);assert.equal((await request(route+'/month-results',undefined,false,false)).status,401);
 assert.match((await request(route+'/month-csv',undefined,true)).body,/,06\n$/);
 await app.close();app=await start({directory,binary,token,large:true});assert.equal((await request(route+'/month-results')).body.complete,true);
 const no=(await request(`/v1/projects/${p.id}/month-imports`,{name:'disabled',csv,policy:'disabled'})).body;
 const n=(await request(`/v1/projects/${p.id}/jobs`,{version_id:no.id,idempotency_key:'disabled',settings:{execution_profile:'joined-v4'}})).body;
 for(let i=0;i<100;i++){if((await request('/v1/jobs/'+n.id)).body.status==='completed')break;await delay(30);}
 assert.equal((await request('/v1/jobs/'+n.id+'/month-csv')).status,409);
 await assert.rejects(start({directory,binary,token,large:true,authMode:'firebase-production'}),/local token/);
});
