import {useEffect,useState} from 'react';
import type {EngineState} from './types';
/** Pagination affects only presentation, never the retained event sequence. */
export function EventList({state,selected,disabled,seek}:{state:EngineState|null;selected:string|null;disabled:boolean;seek:(n:number)=>void}){
 const [page,setPage]=useState(0),events=state?.events??[],last=Math.max(0,Math.ceil(events.length/50)-1),current=Math.min(page,last);
 useEffect(()=>{setPage(selected===null?0:Math.floor(Number(selected)/50));},[selected]);
 return <div className="event-list">{events.length?<><div className="pagination"><button disabled={!current} onClick={()=>setPage(current-1)}>Previous events</button><span>Events page {current+1} / {last+1} · {events.length} recorded</span><button disabled={current===last} onClick={()=>setPage(current+1)}>Next events</button></div>{events.slice(current*50,current*50+50).map(e=><button key={e.id} className={selected===e.id?'event selected-event':'event'} onClick={()=>seek(Number(e.post_state))} disabled={disabled}><span className="event-number">{e.id.padStart(2,'0')}</span><span>Rule {state?.names?.[e.redex.source]??e.redex.source}<small>revision {e.redex.revision} · state {e.redex.state} → {e.post_state}</small></span><span>−{e.footprint.consumed.length} +{e.footprint.produced.length}</span></button>)}</>:<p className="muted">No rewrites recorded.</p>}</div>;
}
