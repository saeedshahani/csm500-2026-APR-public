import type {Command,EngineState,Reply} from './types.ts';
import {atomLabel,project} from './presentation.ts';

export const tourSteps=[
  {title:'Entities and persistent identity',text:'The circles represent Order 1042 and Warehouse. Each has an identity that remains available throughout processing. Their labels supply the application vocabulary.',research:'Representation and identity · RQ1',implication:'A graph can retain an entity while its relationships change. This supports applications in which several records refer to the same order, document or task. The practical view reads those relationships from the carrier. The interpretation of “order” and “warehouse” comes from the application design; the engine operates on their structural representation.',state:0,view:'practical',target:'order'},
  {title:'A relationship as stored structure',text:'The pending relationship connects the order to the warehouse. Its underlying record has its own identity and two ordered fields, one for each endpoint.',research:'Addressable relationships · RQ1',implication:'Giving a relationship an identity allows a rule to select, preserve or replace that particular relationship. Parallel records can connect the same entities and remain distinguishable. The practical arrow summarises one such record; its direction follows the ordered endpoints. Execution order will be established separately from the recorded changes.',state:0,view:'practical',target:'relationship'},
  {title:'Executable rules in the carrier',text:'Internal view exposes the pack and dispatch rule descriptions alongside their data. The highlighted head identifies pack; surrounding records specify its pattern, result, activation, mode, rank and revision.',research:'Operational reflection · RQ1',implication:'The engine reads these stored descriptions to determine the available changes. A valid modification to an operative description can therefore affect subsequent execution. In the wider system, meta-rules perform such modifications subject to validity, revision and rank checks. This sample uses two fixed object-mode rules. It establishes the shared representation of program and data; separate examples demonstrate self-modification. Matching and admission remain defined by the engine.',state:0,view:'internal',target:'rule'},
  {title:'Matching, preservation and replacement',text:'Pack requires a pending relationship between two data items. It preserves both endpoints and replaces the status record with a fresh packed record. The resulting record supplies the pattern required by dispatch.',research:'A structural account of change · RQ1',implication:'The rule specifies what may change and what must retain its identity. Before commitment, the engine checks the match, surviving references and the proposed state, together with the applicable revision and rank conditions. This gives each accepted change an explicit structural basis. The dissertation relates grounded, injective replacements to double-pushout graph rewriting under stated assumptions. Application-specific requirements still have to be encoded or checked separately.',state:0,view:'rule',target:'rule'},
  {title:'The first committed rewrite',text:'Pack has produced a new packed relationship. Order 1042, Warehouse and both rule definitions retain their identities. The history now contains one event and its before-and-after states.',research:'Inspectable computation · RQ3–RQ4',implication:'An event identifies the applied rule revision and the atoms read, preserved, removed and created. Those records support inspection and checked replay, which can help a developer investigate an unexpected result or compare executions. The practical view emphasises application status; the internal view also exposes the supporting program structure. Stating which view supplies an observation is essential when assessing whether a system has changed or stabilised.',state:1,view:'practical',target:'relationship'},
  {title:'The second committed rewrite',text:'Dispatch has consumed packed and produced dispatched. The order and warehouse retain their original identities. The expected application status is now present.',research:'Application meaning and correctness · RQ3',implication:'Here, dispatch denotes a modelled status transition. Physical fulfilment would require a separately implemented service integration. The label is useful because the application assigns it an interpretation and an expected outcome. This distinction matters when evaluating “meaning”: reaching a stable graph and satisfying a practical requirement are separate obligations. The sample satisfies its stated requirement by retaining the original entities with a dispatched relationship.',state:2,view:'practical',target:'relationship'},
  {title:'Establishing the stopping point',text:'A complete applicability check finds no further admitted rewrite, yielding NormalForm. The final check leaves the event count at two.',research:'Termination and useful stability · RQ2–RQ3',implication:'Assign the status a remaining-work score: pending is 2, packed is 1 and dispatched is 0. Each available rewrite decreases this score by one while preserving the order and warehouse. Exactly one rule is enabled at each unfinished stage, so this initial program reaches the expected final state after two rewrites. This argument applies to the stated rules and initial structure. Broader recovery results require their own assumptions about permitted modifications, progress and preserved data.',state:2,view:'practical',target:'relationship'},
  {title:'Dependency across the history',text:'Two events connect three states. Event 1 consumes the packed record created by event 0, establishing the displayed resource dependency. Back revisits each recorded state.',research:'Dependency-derived temporal order · RQ4',implication:'The ordering has a structural witness: dispatch requires a resource supplied by pack. In larger histories, resource dependencies identify chains of work and shared prerequisites that help explain results. The project proves that the extracted relation is a strict partial order over a recorded execution. Additional enabling conditions can remain outside that relation; local conflict checks and bounded replay interventions provide complementary evidence.',state:2,view:'history',target:'relationship'},
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
