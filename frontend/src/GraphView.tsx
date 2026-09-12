import { useEffect, useRef, useState } from 'react';
import { forceSimulation, forceLink, forceManyBody, forceCenter, forceCollide, type SimulationNodeDatum, type SimulationLinkDatum } from 'd3-force';
import type { Atom, EngineState } from './types';
export function kindName(atom:Atom):string {if(typeof atom.kind==='string')return atom.kind;if(atom.kind.Record)return atom.kind.Record.role;if(atom.kind.Constructor!==undefined)return 'Constructor';return 'PatternVariable';}
export function atomLabel(atom:Atom):string {if(typeof atom.kind==='string')return atom.kind;return atom.kind.Constructor??atom.kind.Record?.label??kindName(atom);}
type Node=SimulationNodeDatum&{id:string;atom:Atom};
type Link=SimulationLinkDatum<Node>&{slot:number;id:string};
export function GraphView({state,selected,onSelect}:{state:EngineState;selected:string|null;onSelect:(id:string)=>void}) {
  const positions=useRef(new Map<string,{x:number;y:number}>());
  const [layout,setLayout]=useState<{nodes:Node[];links:Link[]}>({nodes:[],links:[]});
  useEffect(()=>{
    const nodes:Node[]=Object.entries(state.graph.atoms).slice(0,250).map(([id,atom],i)=>({id,atom,...(positions.current.get(id)??{x:450+220*Math.cos(i*2.399),y:275+180*Math.sin(i*2.399)})}));
    const ids=new Set(nodes.map(n=>n.id));
    const links:Link[]=nodes.flatMap(n=>n.atom.fields.flatMap((target,slot)=>ids.has(target)?[{id:`${n.id}:${slot}`,source:n.id,target,slot}]:[]));
    const simulation=forceSimulation(nodes).force('link',forceLink<Node,Link>(links).id(n=>n.id).distance(65).strength(.5)).force('charge',forceManyBody().strength(-190)).force('center',forceCenter(450,275)).force('collision',forceCollide(25));
    const tick=()=>{for(const n of nodes){n.x=Math.max(35,Math.min(865,n.x??450));n.y=Math.max(35,Math.min(515,n.y??275));}positions.current=new Map(nodes.map(n=>[n.id,{x:n.x!,y:n.y!}]));setLayout({nodes:[...nodes],links});};
    if(window.matchMedia('(prefers-reduced-motion: reduce)').matches){simulation.stop();simulation.tick(80);}else simulation.on('tick',tick);tick();
    return ()=>{simulation.on('tick',null);simulation.stop();};
  },[state.graph]);
  const heads=new Map(state.rules.map(r=>[r.head,r]));
  return <div className="graph-canvas">
    <svg viewBox="0 0 900 550" role="group" aria-label="Carrier graph: atoms and directed fields">
      <defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="currentColor"/></marker></defs>
      {layout.links.map(link=>{const a=link.source as Node,b=link.target as Node;const ax=a.x??0,ay=a.y??0,bx=b.x??0,by=b.y??0;const dx=bx-ax,dy=by-ay,len=Math.hypot(dx,dy)||1;const endX=bx-dx/len*19,endY=by-dy/len*19;const bend=link.slot===0?10:-10;const cx=(ax+endX)/2-dy/len*bend,cy=(ay+endY)/2+dx/len*bend;const path=a.id===b.id?`M ${ax} ${ay-18} C ${ax+70} ${ay-80}, ${ax+70} ${ay+60}, ${ax+15} ${ay+10}`:`M ${ax} ${ay} Q ${cx} ${cy} ${endX} ${endY}`;return <g key={link.id} className="field"><path d={path} markerEnd="url(#arrow)"/><text x={cx} y={cy-4}>{link.slot}</text></g>;})}
      {layout.nodes.map(n=><g key={n.id} role="button" tabIndex={0} aria-label={`Atom ${n.id} ${kindName(n.atom)} ${atomLabel(n.atom)}`} aria-pressed={selected===n.id} onClick={()=>onSelect(n.id)} onKeyDown={e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();onSelect(n.id);}}} transform={`translate(${n.x},${n.y})`} className={`atom ${selected===n.id?'selected':''} ${state.quoted[n.id]!==undefined?'quoted':''} ${heads.has(n.id)?'head':''}`}>
        <circle r={heads.has(n.id)?20:16}/><text className="atom-id" y="4" textAnchor="middle">{n.id}</text><text className="atom-name" y="33" textAnchor="middle">{(heads.has(n.id)?`${heads.get(n.id)!.active?'Active':'Inactive'} rule`:atomLabel(n.atom)).slice(0,22)}</text><title>{kindName(n.atom)} · {atomLabel(n.atom)}{state.quoted[n.id]!==undefined?` · quoted by ${state.quoted[n.id]}`:''}</title>
      </g>)}
    </svg>
    {Object.keys(state.graph.atoms).length===0&&<p className="empty-graph">The carrier is empty.</p>}
    {Object.keys(state.graph.atoms).length>250&&<p className="graph-warning">Showing the first 250 atoms. Export retains the complete carrier.</p>}
    <div className="legend"><span>○ Atom identity</span><span>◎ Live rule</span><span>◌ Quoted syntax</span><span>→ Owned field · slot 0 or 1</span></div>
  </div>;
}
