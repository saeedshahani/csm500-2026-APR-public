import {useEffect,useId,useMemo,useRef,useState} from 'react';
import type {EngineState} from './types';
import {project,displayName,kindName,atomLabel} from './presentation';
import {arrangeGraph,boundsOf,fitCamera,zoomCamera,routeLinks,neighbourhood,NODE_LIMIT,LINK_LIMIT,type Point,type Camera,type GraphLink} from './graph-layout';
import './graph-canvas.css';

export function GraphCanvas({state,selected,onSelect,practical=false,guided=false}:{state:EngineState;selected:string|null;onSelect:(id:string)=>void;practical?:boolean;guided?:boolean}){
  const positions=useRef(new Map<string,Point>());
  const svgRef=useRef<SVGSVGElement>(null);
  const marker=useId();
  const [size,setSize]=useState({width:900,height:550});
  const [manualCamera,setManualCamera]=useState<{key:string;camera:Camera}|null>(null);
  const [pinned,setPinned]=useState(false),[labels,setLabels]=useState('auto');
  const [filter,setFilter]=useState('all'),[owner,setOwner]=useState('all'),[focus,setFocus]=useState<string|null>(null),[depth,setDepth]=useState(1);
  const [relation,setRelation]=useState('all'),[hovered,setHovered]=useState<string|null>(null);
  const drag=useRef<{x:number;y:number;camera:Camera}|null>(null);

  const projection=useMemo(()=>project(state),[state]);
  const total=Object.keys(state.graph.atoms).length;
  const projected=practical?projection.nodes:Object.keys(state.graph.atoms);
  const allLinks:GraphLink[]=useMemo(()=>practical
    ?projection.links.filter(l=>relation==='all'||l.label===relation).map(l=>({...l,record:l.id,label:`${l.label} · #${l.id}`}))
    :Object.entries(state.graph.atoms).flatMap(([id,atom])=>atom.fields.map((target,slot)=>({id:`${id}:${slot}`,record:id,label:String(slot),source:id,target}))),[state,practical,projection,relation]);
  const ids=projected.filter(id=>(filter==='all'||kindName(state.graph.atoms[id])===filter)&&(owner==='all'||practical||state.rules.find(r=>r.head===owner)?.representation?.includes(id)));
  const candidates=focus?neighbourhood(ids,allLinks,focus,depth):ids;
  const visible=candidates.slice(0,NODE_LIMIT),present=new Set(visible);
  const eligibleLinks=allLinks.filter(l=>present.has(l.source)&&present.has(l.target));
  const links=eligibleLinks.slice(0,LINK_LIMIT);
  // Only displayed topology changes the layout. Inspecting an atom cannot jiggle it.
  const layoutKey=JSON.stringify([practical,visible,links]);
  const layout=useMemo(()=>{
    const [isPractical,nodeIds,edges]=JSON.parse(layoutKey) as [boolean,string[],GraphLink[]];
    return arrangeGraph(nodeIds,edges,isPractical,positions.current,pinned);
  },[layoutKey,pinned]);
  useEffect(()=>{positions.current=new Map(layout.nodes.map(n=>[n.id,{x:n.x,y:n.y}]));},[layout]);
  const routed=useMemo(()=>routeLinks(layout.nodes,JSON.parse(layoutKey)[2] as GraphLink[],practical),[layout,layoutKey,practical]);
  const bounds=useMemo(()=>boundsOf([...layout.nodes,...routed.flatMap(l=>l.extent)]),[layout,routed]);
  // Returning from a filtered view can settle the same topology differently.
  // A camera is valid for exact geometry, not just the same list of identities.
  const cameraKey=JSON.stringify([layout.nodes,bounds,practical,pinned,size]);
  const fitted=fitCamera(bounds,size.width,size.height);
  const camera=manualCamera?.key===cameraKey?manualCamera.camera:fitted;
  const cameraRef=useRef(camera);cameraRef.current=camera;
  const cameraKeyRef=useRef(cameraKey);cameraKeyRef.current=cameraKey;
  const setCamera=(next:Camera)=>setManualCamera({key:cameraKey,camera:next});
  const fit=()=>setCamera(fitted);

  useEffect(()=>{
    const svg=svgRef.current;if(!svg)return;
    const observer=new ResizeObserver(entries=>{
      const {width,height}=entries[0].contentRect;
      if(width>0&&height>0)setSize({width,height});
    });
    observer.observe(svg);
    const wheel=(event:WheelEvent)=>{
      // Ordinary scrolling scrolls the page; trackpad pinch / Ctrl+wheel zooms here.
      if(!event.ctrlKey&&!event.metaKey)return;
      event.preventDefault();
      const rect=svg.getBoundingClientRect();
      const next=zoomCamera(cameraRef.current,Math.exp(-Math.max(-100,Math.min(100,event.deltaY))*.01),{x:event.clientX-rect.left-rect.width/2,y:event.clientY-rect.top-rect.height/2});
      setManualCamera({key:cameraKeyRef.current,camera:next});
    };
    svg.addEventListener('wheel',wheel,{passive:false});
    return()=>{observer.disconnect();svg.removeEventListener('wheel',wheel);};
  },[]);

  const changes=state.events[Number(state.position)-1]?.footprint;
  const change=(id:string)=>changes?.produced.includes(id)?'produced':changes?.preserved.includes(id)?'preserved':'';
  const active=guided?selected:hovered??selected;
  const activeLinks=new Set(routed.filter(l=>l.source===active||l.target===active||l.record===active).map(l=>l.id));
  const activeNodes=new Set(routed.filter(l=>activeLinks.has(l.id)).flatMap(l=>[l.source,l.target]));
  if(active&&present.has(active))activeNodes.add(active);
  const hasHighlight=activeNodes.size>0;
  const detail=labels==='all'||(labels==='auto'&&camera.zoom>=.8);
  const fieldDetail=labels==='all'||(labels==='auto'&&camera.zoom>=1.15);
  const canFocus=selected!==null&&projected.includes(selected);
  const focusSelected=()=>{
    setFilter('all');setOwner('all');setRelation('all');setFocus(selected);setDepth(1);
  };

  return <div className="graph-canvas">
    <div className="graph-tools">
      <button onClick={fit}>Fit</button>
      <button aria-label="Zoom in" onClick={()=>setCamera(zoomCamera(camera,1.3))}>+</button>
      <button aria-label="Zoom out" onClick={()=>setCamera(zoomCamera(camera,1/1.3))}>−</button>
      <output className="graph-zoom" aria-label="Graph zoom">{Math.round(camera.zoom*100)}%</output>
      {!guided&&<>
      <button disabled={!canFocus} onClick={focusSelected}>Focus selected</button>
      {focus&&<><button disabled={depth>=10} onClick={()=>setDepth(d=>Math.min(10,d+1))}>Expand neighbourhood</button><button onClick={()=>setFocus(null)}>Show all</button></>}
      <label><input type="checkbox" checked={pinned} onChange={e=>setPinned(e.target.checked)}/> Pin layout</label>
      <label>Labels<select aria-label="Graph labels" value={labels} onChange={e=>setLabels(e.target.value)}><option value="auto">Auto</option><option value="all">All</option><option value="selected">Selected</option></select></label>
      <label>Kind / role<select aria-label="Graph kind filter" value={filter} onChange={e=>setFilter(e.target.value)}><option value="all">All</option>{[...new Set(projected.map(id=>kindName(state.graph.atoms[id])))].sort().map(k=><option key={k}>{k}</option>)}</select></label>
      {!practical&&<label>Rule group<select aria-label="Rule ownership filter" value={owner} onChange={e=>{setOwner(e.target.value);setFocus(null);setFilter('all');}}><option value="all">All</option>{state.rules.map(r=><option key={r.head} value={r.head}>{displayName(state,r.head)}</option>)}</select></label>}
      {practical&&<label>Example relationship group<select aria-label="Relationship label filter" value={relation} onChange={e=>setRelation(e.target.value)}><option value="all">All</option>{[...new Set(projection.links.map(l=>l.label))].sort().map(label=><option key={label}>{label}</option>)}</select></label>}
      </>}
    </div>
    {!guided&&<p className="view-context">Showing {visible.length} of {total} atoms · {routed.length} connections{practical?` · ${projection.hidden} rule/quoted atoms hidden`:''}{candidates.length>NODE_LIMIT?` · ${candidates.length-NODE_LIMIT} matching atoms beyond the ${NODE_LIMIT}-atom display limit`:''}{eligibleLinks.length>LINK_LIMIT?` · ${eligibleLinks.length-LINK_LIMIT} connections omitted (${LINK_LIMIT} limit)`:''}</p>}
    <svg ref={svgRef} viewBox={`0 0 ${size.width} ${size.height}`} role="group" tabIndex={0} aria-label={practical?'Practical graph':'Carrier graph: atoms and directed fields'}
      onPointerDown={e=>{
        if(e.button!==0||(e.target as Element).closest('[data-graph-item]'))return;
        drag.current={x:e.clientX,y:e.clientY,camera};e.currentTarget.setPointerCapture(e.pointerId);
      }}
      onPointerMove={e=>{if(drag.current){setCamera({...drag.current.camera,x:drag.current.camera.x-(e.clientX-drag.current.x)/drag.current.camera.zoom,y:drag.current.camera.y-(e.clientY-drag.current.y)/drag.current.camera.zoom});}}}
      onPointerUp={()=>{drag.current=null;}} onPointerCancel={()=>{drag.current=null;}} onLostPointerCapture={()=>{drag.current=null;}}
      onKeyDown={e=>{
        if(e.target!==e.currentTarget)return;
        if(e.key==='+'||e.key==='='){e.preventDefault();setCamera(zoomCamera(camera,1.3));}
        else if(e.key==='-'){e.preventDefault();setCamera(zoomCamera(camera,1/1.3));}
        else if(e.key==='0'){e.preventDefault();fit();}
        else if(['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)){
          e.preventDefault();setCamera({...camera,x:camera.x+(e.key==='ArrowRight'?60:e.key==='ArrowLeft'?-60:0)/camera.zoom,y:camera.y+(e.key==='ArrowDown'?60:e.key==='ArrowUp'?-60:0)/camera.zoom});
        }
      }}>
      <defs><marker id={marker} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" fill="currentColor"/></marker></defs>
      <g transform={`translate(${size.width/2},${size.height/2}) scale(${camera.zoom}) translate(${-camera.x},${-camera.y})`}>
        {!pinned&&!guided&&layout.components.filter(c=>c.ids.length>1).map(component=>{
          const rules=state.rules.filter(r=>component.ids.includes(r.head));
          const name=rules.length===1?`Rule ${displayName(state,rules[0].head)}`:rules.length>1?`${rules.length} connected rules`:'Connected atoms';
          const caption=size.width<600?`${rules.length===1?`#${rules[0].head} · `:''}${component.ids.length} atoms`:`${name} · ${component.ids.length} shown`;
          const inspectRule=()=>{if(!guided&&rules.length===1){setOwner(rules[0].head);setFocus(null);setFilter('all');}};
          return <text key={component.ids[0]} className={`component-label ${rules.length===1?'component-action':''}`} style={{fontSize:Math.max(17,12/camera.zoom)}} x={component.bounds.minX} y={component.bounds.minY-5}
            data-graph-item="component" role={rules.length===1?'button':undefined} tabIndex={rules.length===1?0:undefined} aria-label={rules.length===1?`Inspect rule group ${displayName(state,rules[0].head)}`:undefined}
            onClick={inspectRule} onKeyDown={e=>{if(rules.length===1&&(e.key==='Enter'||e.key===' ')){e.preventDefault();inspectRule();}}}>{caption}<title>{name} · {component.ids.length} shown</title></text>;
        })}
        {routed.map(link=>{
          const highlighted=activeLinks.has(link.id),showLabel=fieldDetail||highlighted;
          return <g key={link.id} data-graph-item="field" data-record-id={link.record} className={`field ${change(link.record)} ${selected===link.record?'selected-link':''} ${highlighted?'highlighted':''} ${hasHighlight&&!highlighted?'graph-dimmed':''}`} role="button" tabIndex={0} aria-label={`${practical?'Relationship':'Field'} ${link.record}: ${link.label}`}
            onClick={()=>onSelect(link.record)} onMouseEnter={()=>setHovered(link.record)} onMouseLeave={()=>setHovered(null)} onFocus={()=>setHovered(link.record)} onBlur={()=>setHovered(null)}
            onKeyDown={e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();onSelect(link.record);}}}>
            <path className="field-line" d={link.path} vectorEffect="non-scaling-stroke" markerEnd={`url(#${marker})`}/><path className="field-hit-area" d={link.path}/>
            {showLabel&&<text x={link.labelPoint.x} y={link.labelPoint.y}>{link.label}{practical&&change(link.record)==='produced'?' [new]':''}</text>}
            <title>{link.label} · {link.source} → {link.target}</title>
          </g>;
        })}
        {layout.nodes.map(n=>{
          const atom=state.graph.atoms[n.id],rule=state.rules.find(r=>r.head===n.id);if(!atom)return null;
          const highlighted=activeNodes.has(n.id),showDetail=detail||highlighted;
          return <g key={n.id} data-graph-item="atom" data-atom-id={n.id} role="button" tabIndex={0} aria-label={`Atom ${n.id} ${kindName(atom)} ${atomLabel(atom)}`} aria-pressed={selected===n.id}
            onClick={()=>onSelect(n.id)} onMouseEnter={()=>setHovered(n.id)} onMouseLeave={()=>setHovered(null)} onFocus={()=>setHovered(n.id)} onBlur={()=>setHovered(null)}
            onKeyDown={e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();onSelect(n.id);}}}
            transform={`translate(${n.x},${n.y})`} className={`atom ${selected===n.id?'selected':''} ${state.quoted[n.id]!==undefined?'quoted':''} ${rule?'head':''} ${change(n.id)} ${hasHighlight&&!highlighted?'graph-dimmed':''}`}>
            <circle r={practical?27:rule?22:18} vectorEffect="non-scaling-stroke"/>
            {(camera.zoom>=.45||showDetail||rule)&&<text className="atom-id" y="4" textAnchor="middle">{practical&&!guided?atomLabel(atom).slice(0,8):n.id}</text>}
            {showDetail&&<text className="atom-name" y="37" textAnchor="middle">{(guided&&practical?`${atomLabel(atom)} · #${n.id}`:practical?displayName(state,n.id):(state.names?.[n.id]??atomLabel(atom))).slice(0,30)}{change(n.id)==='produced'?' [+]':''}</text>}
            {rule&&showDetail&&<text y="51" textAnchor="middle">{rule.active?'Active':'Inactive'} rule</text>}
            <title>{displayName(state,n.id)} · {kindName(atom)}{state.quoted[n.id]!==undefined?` · quoted by ${state.quoted[n.id]}`:''}</title>
          </g>;
        })}
      </g>
    </svg>
    {!total&&<p className="empty-graph">The carrier is empty.</p>}
    {!!total&&!visible.length&&<p className="graph-warning">No atoms match this view. Clear the filters or choose another atom.</p>}
    {!guided&&<div className="legend">○ Atom · ◎ Rule head · dashed: quoted · [+] produced · outlined: preserved</div>}
  </div>;
}
