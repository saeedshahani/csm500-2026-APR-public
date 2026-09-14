import type {Atom,EngineState} from './types.ts';
export function kindName(atom:Atom):string {if(typeof atom.kind==='string')return atom.kind;if(atom.kind.Record)return atom.kind.Record.role;if(atom.kind.Constructor!==undefined)return 'Constructor';return 'PatternVariable';}
export function atomLabel(atom:Atom):string {if(typeof atom.kind==='string')return atom.kind;return atom.kind.Constructor??atom.kind.Record?.label??kindName(atom);}
export function displayName(state:EngineState,id:string){const atom=state.graph.atoms[id]??state.previous?.atoms[id];return `${state.names?.[id]??(atom?atomLabel(atom):'atom')} · #${id}`;}
export type Projection={nodes:string[];links:{id:string;source:string;target:string;label:string}[];hidden:number;fallback:string[]};
/** Only unquoted non-rule records whose endpoints are unquoted constructors
 * become domain edges. No recursive projection or identity merging is inferred. */
export function project(state:EngineState):Projection{
  const atoms=state.graph.atoms;
  const ruleAtoms=new Set(state.rules.flatMap(r=>r.representation??[r.head]));
  const data=Object.keys(atoms).filter(id=>state.quoted[id]===undefined&&!ruleAtoms.has(id));
  const constructors=new Set(data.filter(id=>kindName(atoms[id])==='Constructor'));
  const links=data.filter(id=>kindName(atoms[id])==='ObjectLink'&&atoms[id].fields.length===2&&atoms[id].fields.every(x=>constructors.has(x))).map(id=>({id,source:atoms[id].fields[0],target:atoms[id].fields[1],label:atomLabel(atoms[id])}));
  const linked=new Set(links.map(l=>l.id));const fallback=data.filter(id=>!constructors.has(id)&&!linked.has(id));
  return {nodes:[...constructors,...fallback],links,hidden:Object.keys(atoms).length-data.length,fallback};
}
export function stopReason(state:EngineState){
  const messages:Record<string,string>={NormalForm:'No admitted rewrite remains at the run endpoint.',StepLimit:'The absolute step bound was reached. Reload with a larger bound to start a longer run.',CycleDetected:'The selected observation repeated. This is not necessarily identity equality or equality of hidden data. Disable recurrence detection and reload for a longer bounded run.',ResourceExhaustion:'A resource allowance prevented a complete answer. This is not a normal form.',Yielded:'The batch finished. Step or Run continues the same logical run.',Paused:'Execution is paused at an acknowledged boundary.',Ready:'The loaded program is ready. Step applies one admitted rewrite.',RecordedPrefix:'This recorded prefix is inspectable, but is not a resumable scheduled run.'};
  return messages[state.status]??`Run endpoint outcome: ${state.status}.`;
}
