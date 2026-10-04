import {test} from 'node:test';
import assert from 'node:assert/strict';
import {tourSteps,tourSelection,visitTourStep} from '../src/walkthrough-state.ts';
import type {EngineState,Command,Reply} from '../src/types.ts';

function snapshot(position=0,endpoint=0,status='Ready'):EngineState{
  return {graph:{next_identity:'10',atoms:{'1':{kind:{Constructor:'Order 1042'},fields:[]},'2':{kind:{Constructor:'Warehouse'},fields:[]},'3':{kind:{Record:{role:'ObjectLink',label:['pending','packed','dispatched'][position]}},fields:['1','2']}}},names:{'4':'pack'},rules:[{head:'4',rank:'0',revision:'5',meta:false,active:true}],quoted:{},events:[],dependencies:[],position:String(position),endpoint:String(endpoint),status,can_advance:position===endpoint&&status!=='NormalForm',relation:'',limitation:'',report:null};
}
test('all eight annotations target real data, relationships or rules',()=>{
  assert.equal(tourSteps.length,8);
  assert.equal(tourSelection(snapshot(),0),'1');
  assert.equal(tourSelection(snapshot(),1),'3');
  assert.equal(tourSelection(snapshot(),2),'4');
});
test('every stage connects its observed result to a scoped research explanation',()=>{
  for(const step of tourSteps){assert.match(step.research,/RQ[1-4]/);assert.ok(step.implication.length>150);assert.doesNotMatch(step.text,/not a|rather than|not the/);}
  assert.match(tourSteps[2].implication,/fixed object-mode rules/);
  assert.match(tourSteps[5].implication,/service integration/);
  assert.match(tourSteps[6].implication,/pending is 2, packed is 1 and dispatched is 0/);
  assert.match(tourSteps[7].implication,/Additional enabling conditions/);
});
test('forward executes two rewrites and one terminal check; revisiting never duplicates events',async()=>{
  let state=snapshot(),advances=0;
  const request=async(c:Command):Promise<Reply>=>{
    if(c.command==='jump')state=snapshot(Number(c.state),Number(state.endpoint),state.status);
    else if(c.command==='advance'){
      advances++;const next=Math.min(2,Number(state.endpoint)+1);
      state=snapshot(next,next,advances===3?'NormalForm':'Yielded');
    }else throw Error('Unexpected command');
    return {ok:true,state,error:null,trace:null};
  };
  for(let i=0;i<8;i++)state=await visitTourStep(request,state,i);
  assert.equal(advances,3);assert.equal(state.status,'NormalForm');
  for(const i of [0,4,2,7,1,6])state=await visitTourStep(request,state,i);
  assert.equal(advances,3);assert.equal(state.endpoint,'2');
});
test('unsupported steps, engine failures and unexpected outcomes cannot advance the lesson',async()=>{
  const reject=async():Promise<Reply>=>({ok:false,state:null,trace:null,error:{kind:'ResourceLimit',message:'Budget reached',line:null,column:null}});
  await assert.rejects(visitTourStep(reject,snapshot(),-1),/Unknown/);
  await assert.rejects(visitTourStep(reject,snapshot(),4),/Budget reached/);
  const noProgress=async():Promise<Reply>=>({ok:true,state:snapshot(),error:null,trace:null});
  await assert.rejects(visitTourStep(noProgress,snapshot(),4),/Expected one rewrite/);
  const wrong=snapshot();wrong.graph.atoms['3'].kind={Record:{role:'ObjectLink',label:'wrong'}};
  await assert.rejects(visitTourStep(noProgress,wrong,0),/Expected the pending/);
});
