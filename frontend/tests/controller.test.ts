import {test} from 'node:test';
import assert from 'node:assert/strict';
import {EngineClient,type WorkerPort} from '../src/controller.ts';
import type {Request,Response,Reply,EngineState} from '../src/types.ts';
class Fake implements WorkerPort {
  onmessage:((event:{data:Response})=>void)|null=null;onerror:((event:{message:string})=>void)|null=null;
  messages:Request[]=[];terminated=false;
  postMessage(message:Request){this.messages.push(message);}terminate(){this.terminated=true;}
  answer(reply:Reply,index=this.messages.length-1){const request=this.messages[index];this.onmessage?.({data:{generation:request.generation,id:request.id,reply}});}
}
const ok=(trace:string|null=null):Reply=>({ok:true,error:null,state:null,trace});
test('failed debugger recovery retains the original acknowledged settings for retry',async()=>{
 const workers:Fake[]=[];const client=new EngineClient(()=>{const w=new Fake();workers.push(w);return w;});
 const state:EngineState={graph:{next_identity:'8',atoms:{}},rules:[],quoted:{},events:[],dependencies:[],relation:'positive_resource_order',limitation:'',position:'0',endpoint:'0',status:'Ready',can_advance:true,report:null,breakpoints:['7']};
 const initial=client.request({command:'state'});workers[0].answer({...ok('prefix'),state});await initial;
 const attempt=client.interrupt(),failure=assert.rejects(attempt,/Breakpoint recovery failed/);
 workers[1].answer({...ok('prefix'),state:{...state,breakpoints:[]}});await Promise.resolve();
 workers[1].answer({ok:false,error:{kind:'ResourceExhaustion',message:'test limit',line:null,column:null},state:null,trace:null});await failure;
 await assert.rejects(client.request({command:'state'}),/Recover engine/);
 const retry=client.interrupt();workers[2].answer({...ok('prefix'),state:{...state,breakpoints:[]}});await Promise.resolve();
 assert.deepEqual(workers[2].messages.at(-1)?.payload,{command:'breakpoints',heads:['7']});workers[2].answer({...ok('prefix'),state});assert.deepEqual((await retry)?.state?.breakpoints,['7']);client.dispose();
});
test('recovery restores breakpoints and rediscovers the same paused occurrence even after its checkbox is cleared',async()=>{
 for(const heads of [['7'],[]]){
  const workers:Fake[]=[];const client=new EngineClient(()=>{const w=new Fake();workers.push(w);return w;});
  const breakpoint={state:'0',source:'7',revision:'9',binding:[]};
  const state:EngineState={graph:{next_identity:'10',atoms:{}},rules:[],quoted:{},events:[],dependencies:[],relation:'positive_resource_order',limitation:'',position:'0',endpoint:'0',status:'Breakpoint',can_advance:true,report:null,breakpoints:heads,breakpoint};
  const initial=client.request({command:'state'});workers[0].answer({...ok('prefix'),state});await initial;
  const recovery=client.interrupt();workers[1].answer({...ok('prefix'),state:{...state,status:'Paused',breakpoint:null,breakpoints:[]}});await Promise.resolve();
  assert.deepEqual(workers[1].messages.at(-1)?.payload,{command:'breakpoints',heads:['7']});workers[1].answer({...ok('prefix'),state:{...state,breakpoint:null,breakpoints:['7']}});await Promise.resolve();
  assert.deepEqual(workers[1].messages.at(-1)?.payload,{command:'advance',batch:'1'});workers[1].answer({...ok('prefix'),state:{...state,breakpoints:['7']}});await Promise.resolve();
  if(!heads.length){assert.deepEqual(workers[1].messages.at(-1)?.payload,{command:'breakpoints',heads:[]});workers[1].answer({...ok('prefix'),state});}
  assert.deepEqual((await recovery)?.state?.breakpoint,breakpoint);client.dispose();
 }
});
test('synchronous worker send failure releases the request and permits recovery',async()=>{let first=true;const client=new EngineClient(()=>{const w=new Fake();if(first){w.postMessage=()=>{throw Error('send failed');};first=false;}return w;});await assert.rejects(client.request({command:'state'}),/send failed/);await assert.rejects(client.request({command:'state'}),/Recover engine/);assert.equal(await client.interrupt(),null);client.dispose();});
test('recovery restores a historical inspection cursor, not just the endpoint',async()=>{
  const workers:Fake[]=[];const client=new EngineClient(()=>{const w=new Fake();workers.push(w);return w;});
  const state:EngineState={graph:{next_identity:'0',atoms:{}},rules:[],quoted:{},events:[],dependencies:[],relation:'positive_resource_order',limitation:'',position:'0',endpoint:'2',status:'Paused',can_advance:false,report:null};
  const initial=client.request({command:'jump',state:'0'});workers[0].answer({...ok('history'),state});await initial;
  const recovery=client.interrupt();workers[1].answer({...ok('history'),state:{...state,position:'2'}});await Promise.resolve();
  assert.deepEqual(workers[1].messages[1].payload,{command:'jump',state:'0'});workers[1].answer({...ok('history'),state});assert.equal((await recovery)?.state?.position,'0');client.dispose();
});
test('worker failure rejects later requests until recovery',async()=>{
  const workers:Fake[]=[];const client=new EngineClient(()=>{const w=new Fake();workers.push(w);return w;});
  const pending=client.request({command:'validate'});const failure=assert.rejects(pending,/crash/);workers[0].onerror?.({message:'crash'});await failure;
  await assert.rejects(client.request({command:'state'}),/Recover engine/);await client.interrupt();const next=client.request({command:'validate'});workers[1].answer(ok());await next;client.dispose();
});
test('late replies, repeated replacement and disposal cannot publish old sessions',async()=>{
  const workers:Fake[]=[];const client=new EngineClient(()=>{const w=new Fake();workers.push(w);return w;});
  const initial=client.request({command:'load'});workers[0].answer(ok('verified-prefix'));await initial;
  const pending=client.request({command:'advance',batch:'1'});const rejected=assert.rejects(pending,/Interrupted/);
  const stopped=client.interrupt();assert.equal(workers[0].terminated,true);workers[0].answer(ok('unacknowledged-future'));workers[1].answer(ok('verified-prefix'));assert.equal((await stopped)?.trace,'verified-prefix');await rejected;
  assert.equal(workers[1].messages[0].payload.trace,'verified-prefix');
  const next=client.request({command:'reset'});workers[0].answer(ok('stale-reset'));workers[1].answer(ok('reset-prefix'));assert.equal((await next).trace,'reset-prefix');
  const request=client.request({command:'state'});const disposed=assert.rejects(request,/disposed/);client.dispose();workers[1].answer(ok('stale'));await disposed;assert.equal(workers[1].terminated,true);await assert.rejects(client.request({command:'state'}),/disposed/);client.dispose();
});
test('out-of-order identities and concurrent commands are rejected',async()=>{
  const worker=new Fake();const client=new EngineClient(()=>worker);const p=client.request({command:'load'});await assert.rejects(client.request({command:'reset'}),/in flight/);
  worker.onmessage?.({data:{generation:0,id:999,reply:ok('wrong')}});worker.answer(ok('right'));assert.equal((await p).trace,'right');client.dispose();
});
test('a failed load does not replace the recovery prefix',async()=>{
  const workers:Fake[]=[];const client=new EngineClient(()=>{const w=new Fake();workers.push(w);return w;});let p=client.request({command:'load'});workers[0].answer(ok('old'));await p;
  p=client.request({command:'load'});workers[0].answer({ok:false,error:{kind:'MalformedInput',message:'bad',line:null,column:null},state:null,trace:null});await p;
  const recovery=client.interrupt();assert.equal(workers[1].messages[0].payload.trace,'old');workers[1].answer(ok('old'));await recovery;client.dispose();
});
