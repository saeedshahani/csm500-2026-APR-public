import {test} from 'node:test';
import assert from 'node:assert/strict';
import {LatestQuery} from '../src/navigation.ts';
import {project,stopReason,displayName} from '../src/presentation.ts';
import type {EngineState} from '../src/types.ts';
const state:EngineState={graph:{next_identity:'9',atoms:{'0':{kind:{Constructor:'A'},fields:[]},'1':{kind:{Constructor:'B'},fields:[]},'2':{kind:{Record:{role:'ObjectLink',label:'edge'}},fields:['0','1']},'3':{kind:{Record:{role:'ObjectLink',label:'edge'}},fields:['0','1']},'4':{kind:{Record:{role:'ObjectLink',label:'loop'}},fields:['0','0']},'5':{kind:{Record:{role:'ObjectLink',label:'quoted'}},fields:['0','1']},'6':{kind:{Record:{role:'ObjectLink',label:'higher'}},fields:['2','1']},'7':{kind:'RuleHead',fields:[]}}},rules:[{head:'7',revision:'8',rank:'0',active:true,meta:false,representation:['5','7']}],quoted:{'5':'7'},events:[],dependencies:[],position:'0',endpoint:'0',status:'Ready',relation:'positive_resource_order',limitation:'',can_advance:true,report:null};
test('projection preserves loops and parallel identities, hides quotation and retains unprojectable data',()=>{
  const copy=JSON.stringify(state),p=project(state);assert.deepEqual(p.links.map(l=>l.id),['2','3','4']);assert.deepEqual(p.fallback,['6']);assert.equal(p.hidden,2);assert.equal(p.links[2].source,p.links[2].target);assert.equal(JSON.stringify(state),copy);
  for(const link of p.links)assert.deepEqual({kind:{Record:{role:'ObjectLink',label:link.label}},fields:[link.source,link.target]},state.graph.atoms[link.id]);
});
test('observer does not change projection and deleted identity uses prior payload',()=>{
  assert.deepEqual(project({...state,observer:'value'}),project(state));assert.match(displayName({...state,previous:{next_identity:'10',atoms:{'9':{kind:{Constructor:'deleted'},fields:[]}}}},'9'),/deleted/);
  assert.match(stopReason({...state,status:'ResourceExhaustion'}),/not a normal form/);assert.match(stopReason({...state,status:'CycleDetected'}),/hidden data/);
});
test('rapid scrubbing runs only active and latest pending query and suppresses stale publication',async()=>{
  const calls:number[]=[];const completions:((n:number)=>void)[]=[];const q=new LatestQuery<number>(n=>{calls.push(n);return new Promise(resolve=>completions.push(resolve));});
  const a=q.request(1),b=q.request(2),c=q.request(10000);assert.equal(await b,null);assert.deepEqual(calls,[1]);completions[0](1);assert.equal(await a,null);assert.deepEqual(calls,[1,10000]);completions[1](10000);assert.equal(await c,10000);
});
test('replacement invalidates in-flight and queued inspections including late errors',async()=>{
  let reject!:(e:Error)=>void;const q=new LatestQuery<number>(()=>new Promise((_,r)=>{reject=r;}));const a=q.request(1),b=q.request(2);q.invalidate();reject(new Error('old worker'));assert.equal(await a,null);assert.equal(await b,null);
});
