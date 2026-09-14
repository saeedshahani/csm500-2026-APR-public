import {useEffect,useRef,useState} from 'react';
import {forceSimulation,forceLink,forceManyBody,forceCenter,forceCollide,type SimulationNodeDatum,type SimulationLinkDatum} from 'd3-force';
import type {EngineState} from './types';
import {project,displayName,kindName,atomLabel} from './presentation';
type Node=SimulationNodeDatum&{id:string};
type Link=SimulationLinkDatum<Node>&{id:string;label:string;record:string};
export function GraphCanvas({state,selected,onSelect,practical=false}:{state:EngineState;selected:string|null;onSelect:(id:string)=>void;practical?:boolean}){
  const positions=useRef(new Map<string,{x:number;y:number}>());
  const [layout,setLayout]=useState<{nodes:Node[];links:Link[]}>({nodes:[],links:[]});
  const [zoom,setZoom]=useState(1),[pan,setPan]=useState({x:0,y:0}),[pinned,setPinned]=useState(false);
  const [filter,setFilter]=useState('all'),[owner,setOwner]=useState('all'),[focus,setFocus]=useState<string|null>(null),[depth,setDepth]=useState(1);
  const [relation,setRelation]=useState('all');
  const drag=useRef<{x:number;y:number;px:number;py:number}|null>(null);
  const projection=project(state),total=Object.keys(state.graph.atoms).length;
  const projected=practical?projection.nodes:Object.keys(state.graph.atoms);
  const ids=projected.filter(id=>(filter==='all'||kindName(state.graph.atoms[id])===filter)&&(owner==='all'||state.rules.find(r=>r.head===owner)?.representation?.includes(id)));
  const neighbours=new Set<string>();
  if(focus){neighbours.add(focus);for(let i=0;i<depth;i++){const next=new Set(neighbours);for(const[id,a]of Object.entries(state.graph.atoms))if(neighbours.has(id)||a.fields.some(f=>neighbours.has(f))){next.add(id);a.fields.forEach(f=>next.add(f));}for(const id of next)neighbours.add(id);}}
  const visible=ids.filter(id=>!focus||neighbours.has(id)).slice(0,250);
  const signature=JSON.stringify(visible);
  useEffect(()=>{
    const selectedIds:string[]=JSON.parse(signature);
    const nodes:Node[]=selectedIds.map((id,i)=>({id,...(positions.current.get(id)??{x:450+240*Math.cos(i*2.399),y:275+170*Math.sin(i*2.399)})}));
    const present=new Set(selectedIds);
    const links:Link[]=practical?project(state).links.filter(l=>present.has(l.source)&&present.has(l.target)).map(l=>({...l,record:l.id,label:`${l.label} · #${l.id}`})):nodes.flatMap(n=>state.graph.atoms[n.id].fields.flatMap((target,slot)=>present.has(target)?[{id:`${n.id}:${slot}`,record:n.id,label:String(slot),source:n.id,target}]:[]));
    if(practical&&relation!=='all'){for(let i=links.length-1;i>=0;i--)if(atomLabel(state.graph.atoms[links[i].record])!==relation)links.splice(i,1);}
    links.splice(500);
    const simulation=forceSimulation(nodes).force('link',forceLink<Node,Link>(links).id(n=>n.id).distance(practical?180:90).strength(.5)).force('charge',forceManyBody().strength(practical?-500:-250)).force('center',forceCenter(450,275)).force('collision',forceCollide(practical?55:35));
    const tick=()=>{for(const n of nodes){n.x=Math.max(45,Math.min(855,n.x??450));n.y=Math.max(50,Math.min(490,n.y??275));}positions.current=new Map(nodes.map(n=>[n.id,{x:n.x!,y:n.y!}]));setLayout({nodes:[...nodes],links});};
    if(pinned)simulation.stop();else if(window.matchMedia('(prefers-reduced-motion: reduce)').matches){simulation.stop();simulation.tick(100);}else simulation.on('tick',tick);tick();
    return()=>{simulation.on('tick',null);simulation.stop();};
  },[state,signature,practical,pinned,relation]);
  const changes=state.events[Number(state.position)-1]?.footprint;
  const change=(id:string)=>changes?.produced.includes(id)?'produced':changes?.preserved.includes(id)?'preserved':'';
  return <div className="graph-canvas">
    <div className="graph-tools">
      <button onClick={()=>{setZoom(1);setPan({x:0,y:0});}}>Fit</button><button aria-label="Zoom in" onClick={()=>setZoom(z=>Math.min(4,z*1.25))}>+</button><button aria-label="Zoom out" onClick={()=>setZoom(z=>Math.max(.5,z/1.25))}>−</button>
      <button disabled={!selected} onClick={()=>{setFocus(selected);setDepth(1);}}>Focus selected</button>{focus&&<><button onClick={()=>setDepth(d=>Math.min(10,d+1))}>Expand neighbourhood</button><button onClick={()=>setFocus(null)}>Show all</button></>}
      <label><input type="checkbox" checked={pinned} onChange={e=>setPinned(e.target.checked)}/> Pin layout</label>
      <label>Kind / role<select aria-label="Graph kind filter" value={filter} onChange={e=>setFilter(e.target.value)}><option value="all">All</option>{[...new Set(projected.map(id=>kindName(state.graph.atoms[id])))].sort().map(k=><option key={k}>{k}</option>)}</select></label>
      {practical&&<label>Example relationship group<select aria-label="Relationship label filter" value={relation} onChange={e=>setRelation(e.target.value)}><option value="all">All</option>{[...new Set(projection.links.map(l=>l.label))].sort().map(label=><option key={label}>{label}</option>)}</select></label>}
      {!practical&&<label>Rule group<select aria-label="Rule ownership filter" value={owner} onChange={e=>setOwner(e.target.value)}><option value="all">All</option>{state.rules.map(r=><option key={r.head} value={r.head}>{displayName(state,r.head)}</option>)}</select></label>}
    </div>
    <p className="view-context">{practical?'Practical projection: relationships map to record identities. Rule syntax and metadata are hidden.':'Internal carrier: circles are atoms; arrows are owned fields, not execution order.'} Showing {visible.length} of {total} atoms{practical?`; ${projection.links.length} eligible relationships, ${projection.hidden} rule/quoted atoms hidden, ${projection.fallback.length} unprojectable data atoms retained`:''}. Filters do not affect execution.</p>
    <svg viewBox="0 0 900 550" role="group" aria-label={practical?'Practical graph':'Carrier graph: atoms and directed fields'} onPointerDown={e=>{if(e.target===e.currentTarget){drag.current={x:e.clientX,y:e.clientY,px:pan.x,py:pan.y};e.currentTarget.setPointerCapture(e.pointerId);}}} onPointerMove={e=>{if(drag.current){const scale=900/e.currentTarget.getBoundingClientRect().width;setPan({x:drag.current.px+(e.clientX-drag.current.x)*scale,y:drag.current.py+(e.clientY-drag.current.y)*scale});}}} onPointerUp={()=>{drag.current=null;}}>
      <defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" fill="currentColor"/></marker></defs>
      <g transform={`translate(${450+pan.x},${275+pan.y}) scale(${zoom}) translate(-450,-275)`}>
      {layout.links.map(link=>{const a=link.source as Node,b=link.target as Node;const ax=a.x??0,ay=a.y??0,bx=b.x??0,by=b.y??0;const dx=bx-ax,dy=by-ay,len=Math.hypot(dx,dy)||1;const ex=bx-dx/len*22,ey=by-dy/len*22;const peers=layout.links.filter(l=>{const s=l.source as Node,t=l.target as Node;return (s.id===a.id&&t.id===b.id)||(s.id===b.id&&t.id===a.id);});const offset=peers.indexOf(link)-(peers.length-1)/2,bend=offset*42*(a.id<b.id?1:-1);const cx=(ax+ex)/2-dy/len*bend,cy=(ay+ey)/2+dx/len*bend,radius=50+(offset+(peers.length-1)/2)*25;const path=a.id===b.id?`M ${ax} ${ay-18} C ${ax+radius} ${ay-radius-50}, ${ax+radius} ${ay+radius}, ${ax+18} ${ay}`:`M ${ax} ${ay} Q ${cx} ${cy} ${ex} ${ey}`;return <g key={link.id} className={`field ${change(link.record)} ${selected===link.record?'selected-link':''}`} role="button" tabIndex={0} aria-label={`${practical?'Relationship':'Field'} ${link.record}: ${link.label}`} onClick={()=>onSelect(link.record)} onKeyDown={e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();onSelect(link.record);}}}><path d={path} markerEnd="url(#arrow)"/><text x={a.id===b.id?ax+radius:cx} y={a.id===b.id?ay-radius/2:cy-5}>{link.label}{practical&&change(link.record)==='produced'?' [new]':''}</text><title>{link.label} · {a.id} → {b.id}</title></g>;})}
      {layout.nodes.map(n=>{const atom=state.graph.atoms[n.id],rule=state.rules.find(r=>r.head===n.id);if(!atom)return null;return <g key={n.id} role="button" tabIndex={0} aria-label={`Atom ${n.id} ${kindName(atom)} ${atomLabel(atom)}`} aria-pressed={selected===n.id} onClick={()=>onSelect(n.id)} onKeyDown={e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();onSelect(n.id);}}} transform={`translate(${n.x},${n.y})`} className={`atom ${selected===n.id?'selected':''} ${state.quoted[n.id]!==undefined?'quoted':''} ${rule?'head':''} ${change(n.id)}`}><circle r={practical?27:rule?22:18}/><text className="atom-id" y="4" textAnchor="middle">{practical?atomLabel(atom).slice(0,8):n.id}</text><text className="atom-name" y="37" textAnchor="middle">{(practical?displayName(state,n.id):(state.names?.[n.id]??atomLabel(atom))).slice(0,30)}{change(n.id)==='produced'?' [+]':''}</text>{rule&&<text y="51" textAnchor="middle">{rule.active?'Active':'Inactive'} rule</text>}<title>{displayName(state,n.id)} · {kindName(atom)}{state.quoted[n.id]!==undefined?` · quoted by ${state.quoted[n.id]}`:''}</title></g>;})}
      </g>
    </svg>
    {!total&&<p className="empty-graph">The carrier is empty.</p>}
    <p className="view-context">Displaying {layout.links.length} connections after filters. Canvas limit: 250 atoms and 500 connections. The inspector searches the complete carrier; use Internal for records not represented by practical edges. Relationship groups use example labels, not an inferred business schema.</p>
    <div className="legend">○ Atom · ◎ Rule head · dashed: quoted syntax · [+] produced · outlined: preserved. Select a relationship to inspect its record.</div>
  </div>;
}
