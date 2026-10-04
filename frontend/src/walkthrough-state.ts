import type {Command,EngineState,Reply} from './types.ts';
import {atomLabel,project} from './presentation.ts';

export const tourSteps=[
  {title:'An order and a warehouse',text:'Each circle is a data item. Its identity stays the same while the order moves through the process. The picture is drawn from the running graph, not a separate illustration.',state:0,view:'practical',target:'order'},
  {title:'The line carries the status',text:'This pending relationship connects Order 1042 to Warehouse. Internally, it is a stored record with two fields pointing to those items. The arrow is a relationship, not the direction of execution.',state:0,view:'practical',target:'relationship'},
  {title:'Rules live here too',text:'Internal view reveals the same graph, including the two rules. The highlighted head belongs to pack. Its surrounding atoms describe what to match, what to keep and what to create. The rules in this sample do not modify themselves.',state:0,view:'internal',target:'rule'},
  {title:'What the pack rule changes',text:'Pack matches a pending relationship. It keeps the order and warehouse, removes that relationship and creates a fresh packed relationship. Dispatch cannot match until packed exists.',state:0,view:'rule',target:'rule'},
  {title:'First rewrite: packed',text:'The engine has applied pack once. The relationship now says packed and has a new identity. The two data items and both rule definitions are unchanged. This is one recorded event, not a screen animation.',state:1,view:'practical',target:'relationship'},
  {title:'Second rewrite: dispatched',text:'Dispatch consumed the packed relationship and created dispatched. Order 1042 and Warehouse still have their original identities. This models a status change; it has not sent a parcel or called an external service.',state:2,view:'practical',target:'relationship'},
  {title:'Why this run finishes',text:'The engine checked for another match and found none: NormalForm. Pack requires pending; dispatch requires packed. Only dispatched remains. That final check adds no event. Other programs can cycle or keep growing; this result belongs to this sample.',state:2,view:'practical',target:'relationship'},
  {title:'Two events, three states',text:'State 0 was pending, state 1 was packed and state 2 is dispatched. The second event used the record produced by the first, so the trace contains a resource dependency from event 0 to event 1. Use Back to revisit the recorded states without running either rule again.',state:2,view:'history',target:'relationship'},
] as const;

export const tourConfig={max_steps:'8',max_work:'1000000',max_candidates:'1000',seed:null,detect_cycles:false,observer:'observer-v1\nall\n',execution_profile:'reference-v1'};

/** Navigate the real recorded prefix. Back/forward never duplicates an event. */
export async function visitTourStep(request:(command:Command)=>Promise<Reply>,current:EngineState,index:number):Promise<EngineState>{
  if(!Number.isInteger(index)||index<0||index>=tourSteps.length)throw Error('Unknown walkthrough step');
  let state=current;
  const send=async(command:Command)=>{
    const reply=await request(command);
    if(!reply.ok||!reply.state)throw Error(reply.error?.message??'The walkthrough engine returned no state');
    state=reply.state;
  };
  const desired=tourSteps[index].state;
  while(Number(state.endpoint)<desired){
    if(state.position!==state.endpoint)await send({command:'jump',state:state.endpoint});
    const before=state.endpoint;
    await send({command:'advance',batch:'1'});
    if(Number(state.endpoint)!==Number(before)+1)throw Error(`Expected one rewrite; engine returned ${state.status}`);
  }
  if(index>=6&&state.status!=='NormalForm'){
    if(state.position!==state.endpoint)await send({command:'jump',state:state.endpoint});
    await send({command:'advance',batch:'1'});
    if(state.status!=='NormalForm'||state.endpoint!=='2')throw Error('The sample did not finish after its two expected rewrites');
  }
  if(Number(state.position)!==desired)await send({command:'jump',state:String(desired)});
  const expected=['pending','packed','dispatched'][desired];
  const links=project(state).links;
  if(links.length!==1||links[0].label!==expected)throw Error(`Expected the ${expected} relationship at state ${desired}`);
  return state;
}

export function tourSelection(state:EngineState,index:number):string|null{
  const step=tourSteps[index];
  if(step.target==='rule')return state.rules.find(r=>state.names?.[r.head]==='pack')?.head??state.rules[0]?.head??null;
  if(step.target==='order')return Object.entries(state.graph.atoms).find(([,atom])=>atomLabel(atom)==='Order 1042')?.[0]??null;
  return project(state).links[0]?.id??null;
}
