import {useEffect,useRef,useState} from 'react';
import {EngineClient,type WorkerPort} from './controller';
import type {Command,EngineState,Reply} from './types';
import {GraphView,atomLabel,kindName} from './GraphView';
import {Editor} from './Editor';
const examples=import.meta.glob('../../examples/{programs,behaviour,independent}/*.graph',{query:'?raw',import:'default',eager:true}) as Record<string,string>;
const choices=Object.keys(examples).filter(k=>!k.includes('.expected.')).sort();
const initial=choices.find(k=>k.endsWith('/independent/shared-read.graph'))!;
const observers=import.meta.glob('../../examples/observers/*.observer',{query:'?raw',import:'default',eager:true}) as Record<string,string>;
const all=observers[Object.keys(observers).find(k=>k.endsWith('/all.observer'))!];
const valueObserver=observers[Object.keys(observers).find(k=>k.endsWith('/value.observer'))!];
function download(text:string,name:string){const url=URL.createObjectURL(new Blob([text],{type:'application/json'}));const a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
export default function App(){
  const client=useRef<EngineClient|null>(null),loop=useRef(false),epoch=useRef(0);
  const [source,setSource]=useState(examples[initial]),[example,setExample]=useState(initial),[state,setState]=useState<EngineState|null>(null),[trace,setTrace]=useState<string|null>(null);
  const [busy,setBusy]=useState(false),[initialising,setInitialising]=useState(true),[diagnostic,setDiagnostic]=useState(''),[selected,setSelected]=useState<string|null>(null),[selectedEvent,setSelectedEvent]=useState<string|null>(null);
  const [maxSteps,setMaxSteps]=useState('20'),[seed,setSeed]=useState('42'),[cycles,setCycles]=useState(false),[observer,setObserver]=useState('all'),[batch,setBatch]=useState('1'),[dataset,setDataset]=useState(0),[sessionSource,setSessionSource]=useState('');
  const [atomSearch,setAtomSearch]=useState('');
  function publish(reply:Reply){if(!reply.ok){setDiagnostic(`${reply.error?.kind}: ${reply.error?.message}${reply.error?.line?` (line ${reply.error.line}, column ${reply.error.column})`:''}`);return false;}setDiagnostic('');if(reply.state)setState(reply.state);if(reply.trace)setTrace(reply.trace);return true;}
  useEffect(()=>{
    const engine=new EngineClient(()=>new Worker(new URL('./engine.worker.ts',import.meta.url),{type:'module'}) as unknown as WorkerPort);client.current=engine;let alive=true;
    engine.request({command:'validate',source:'program {}'}).then(reply=>{if(alive){publish(reply);setInitialising(false);}}).catch(e=>{if(alive){setDiagnostic(`Initialisation failed: ${String(e)}. Reload the page to retry.`);setInitialising(false);}});
    return ()=>{alive=false;loop.current=false;epoch.current++;engine.dispose();client.current=null;};
  },[]);
  async function command(payload:Command){if(!client.current)return;const token=epoch.current;setBusy(true);try{const reply=await client.current.request(payload);if(token===epoch.current)publish(reply);return reply;}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current)setBusy(false);}}
  async function load(){const reply=await command({command:'load',source,config:{max_steps:maxSteps,seed:seed.trim()?seed:null,detect_cycles:cycles,observer:observer==='all'?all:valueObserver}});if(reply?.ok){setDataset(n=>n+1);setSelected(null);setSelectedEvent(null);setSessionSource(source);}}
  async function run(){if(!client.current)return;const token=epoch.current;loop.current=true;setBusy(true);try{while(loop.current&&token===epoch.current){const reply=await client.current.request({command:'advance',batch});if(token!==epoch.current)break;publish(reply);if(!reply.ok||!reply.state?.can_advance)break;await new Promise(resolve=>setTimeout(resolve,0));}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current){loop.current=false;setBusy(false);}}}
  async function stop(){loop.current=false;const token=++epoch.current;setBusy(true);try{const reply=await client.current!.interrupt();if(token===epoch.current){if(reply)publish(reply);setDiagnostic('Stopped at the last acknowledged state. Any unfinished batch was discarded; continue preserves this prefix.');}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current)setBusy(false);}}
  async function reset(){const reply=await command({command:'reset'});if(reply?.ok){setSelected(null);setSelectedEvent(null);setDataset(n=>n+1);}}
  async function importFile(file:File|undefined){if(!file)return;if(file.size>16*1024*1024){setDiagnostic('ResourceExhaustion: trace exceeds 16 MiB');return;}const token=epoch.current;setBusy(true);try{const text=await file.text();if(token!==epoch.current)return;const reply=await command({command:'import',trace:text});if(token===epoch.current&&reply?.ok){setSelected(null);setSelectedEvent(null);setDataset(n=>n+1);setSessionSource('');}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current)setBusy(false);}}
  async function recover(){const token=++epoch.current;setBusy(true);try{const reply=await client.current!.interrupt();if(token===epoch.current){if(reply)publish(reply);else publish(await client.current!.request({command:'validate',source:'program {}'}));}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current)setBusy(false);}}
  const locked=busy||initialising;
  const atom=selected&&state?state.graph.atoms[selected]:null;
  const event=state?.events.find(e=>e.id===selectedEvent);
  const rule=state?.rules.find(r=>r.head===selected);
  const labels=state?Object.entries(state.graph.atoms).filter(([id,a])=>`${id} ${kindName(a)} ${atomLabel(a)}`.toLowerCase().includes(atomSearch.toLowerCase())).slice(0,100):[];
  return <main>
    <header className="app-header"><div className="wordmark"><span className="brand-symbol" aria-hidden="true">⌘</span><div>Reflective Graph<span className="subtitle">Research workbench</span></div></div><div className="header-meta">Graph-resident rules <span> / </span> Verified histories</div></header>
    <section className="toolbar" aria-label="Execution controls">
      <button className="primary" onClick={load} disabled={locked}>Load program</button>
      <button onClick={()=>command({command:'advance',batch:'1'})} disabled={locked||!state?.can_advance}>Step</button>
      <button onClick={run} disabled={locked||!state?.can_advance}>Run</button>
      <button onClick={stop} disabled={!busy||initialising}>Stop</button>
      <button onClick={reset} disabled={locked||!state}>Reset</button>
      <div className="toolbar-gap"/>
      <button disabled={locked||!trace} onClick={()=>trace&&download(trace,'reflective-graph.trace.json')}>Export trace</button>
      <label className={`file-button ${locked?'disabled':''}`}>Import trace<input aria-label="Import trace" type="file" accept=".json,application/json" disabled={locked} onChange={e=>{void importFile(e.target.files?.[0]);e.target.value='';}}/></label>
    </section>
    <div className="status-strip" aria-live="polite"><span className={`status-dot ${busy?'working':''}`}/><strong data-testid="status">{initialising?'Loading engine':busy?'Working':state?.status??'No program loaded'}</strong><span>{state?`State ${state.position} / ${state.endpoint} · ${Object.keys(state.graph.atoms).length} atoms · ${state.rules.length} live rules`:'Choose an example, inspect its source, then load it.'}</span>{state&&sessionSource!==source&&<span className="draft-note">Editor differs from loaded source</span>}</div>
    {diagnostic&&<div className="diagnostic" role="alert">{diagnostic} <button disabled={locked} onClick={recover}>Recover engine</button></div>}
    <div className="workspace">
      <section className="source-pane" aria-label="Program editor"><div className="section-heading"><h1>Program</h1><span>01 / SOURCE</span></div>
        <label className="example-label">Example<select aria-label="Example" value={example} disabled={locked} onChange={e=>{setExample(e.target.value);setSource(examples[e.target.value]);}}>{choices.map(k=><option key={k} value={k}>{k.replace('../../examples/','').replace('.graph','')}</option>)}</select></label>
        <Editor value={source} onChange={setSource}/>
        <details className="settings"><summary>Run settings <span>Applied on load</span></summary><div className="settings-grid">
          <label>Absolute step bound<input aria-label="Absolute step bound" inputMode="numeric" value={maxSteps} onChange={e=>setMaxSteps(e.target.value)} disabled={locked}/></label>
          <label>Seed · blank is deterministic<input aria-label="Seed" value={seed} onChange={e=>setSeed(e.target.value)} disabled={locked}/></label>
          <label>Observer<select aria-label="Observer" value={observer} onChange={e=>setObserver(e.target.value)} disabled={locked}><option value="all">Complete carrier</option><option value="value">Value links</option></select></label>
          <label>Commits per batch<select aria-label="Commits per batch" value={batch} onChange={e=>setBatch(e.target.value)} disabled={locked}><option>1</option><option>2</option><option>5</option><option>10</option></select></label>
          <label className="checkbox"><input type="checkbox" checked={cycles} onChange={e=>setCycles(e.target.checked)} disabled={locked}/> Detect recurrence</label>
        </div></details>
      </section>
      <section className="graph-pane"><div className="section-heading"><h2>Carrier</h2><span>02 / STATE {state?.position??'—'}</span></div>
        {state?<GraphView key={dataset} state={state} selected={selected} onSelect={setSelected}/>:<div className="welcome"><span className="welcome-mark" aria-hidden="true">○ → ○</span><h2>The graph is the program.</h2><p>Load an example to inspect its atoms, rules and recorded rewrites.</p></div>}
        <div className="inspector"><div className="section-heading"><h2>Atom inspector</h2><span>{selected?`ID ${selected}`:'SELECT AN ATOM'}</span></div>
          <input aria-label="Find atom" className="atom-search" placeholder="Find by identity, kind or label" value={atomSearch} onChange={e=>setAtomSearch(e.target.value)}/>
          <select aria-label="Select atom" value={selected??''} onChange={e=>setSelected(e.target.value||null)}><option value="">Select an atom ({labels.length} shown)</option>{labels.map(([id,a])=><option key={id} value={id}>{id} · {kindName(a)} · {atomLabel(a)}</option>)}</select>
          {atom?<dl><dt>Kind / label</dt><dd>{kindName(atom)} · {atomLabel(atom)}</dd><dt>Owned fields</dt><dd>{atom.fields.length?atom.fields.map((id,i)=><button className="text-button" key={i} onClick={()=>setSelected(id)}>slot {i} → {id}</button>):'None'}</dd><dt>Quotation</dt><dd>{state?.quoted[selected!]!==undefined?`Quoted by head ${state.quoted[selected!]}`:'Not quoted'}</dd><dt>Membership records</dt><dd>{Object.entries(state!.graph.atoms).filter(([,a])=>['Lhs','Rhs'].includes(kindName(a))&&a.fields.includes(selected!)).map(([id])=>id).join(', ')||'None'}</dd>{rule&&<><dt>Rule</dt><dd>{rule.active?'Active':'Inactive'} · {rule.meta?'Meta':'Object'} · rank {rule.rank} · revision {rule.revision}</dd></>}</dl>:<p className="muted">Select a node or use the keyboard-accessible atom list.</p>}
        </div>
      </section>
    </div>
    <section className="history" aria-label="Recorded history"><div className="section-heading"><h2>Recorded rewrites <span className="count">{state?.events.length??0}</span></h2><span>03 / HISTORY</span></div>
      <div className="history-controls"><label>Inspect state<select aria-label="Inspect state" disabled={locked||!state} value={state?.position??'0'} onChange={e=>command({command:'jump',state:e.target.value})}>{Array.from({length:(state?.events.length??0)+1},(_,i)=><option key={i} value={String(i)}>{i===0?'0 · Initial':`${i} · After event ${i-1}`}</option>)}</select></label><span>Historical inspection does not append. Return to the endpoint to continue a paused run.</span></div>
      <div className="history-grid"><div className="event-list">{state?.events.length?state.events.map(e=><button key={e.id} className={selectedEvent===e.id?'event selected-event':'event'} onClick={()=>{setSelectedEvent(e.id);setSelected(null);}}><span className="event-number">{e.id.padStart(2,'0')}</span><span>Rule {e.redex.source}<small>revision {e.redex.revision} · state {e.redex.state} → {e.post_state}</small></span><span>−{e.footprint.consumed.length} +{e.footprint.produced.length}</span></button>):<p className="muted">No rewrites recorded.</p>}</div><div className="event-detail">{event?<><h3>Event {event.id}</h3><p>Applied rule {event.redex.source}, exact revision {event.redex.revision}.</p><dl>{Object.entries(event.footprint).map(([name,ids])=><div key={name}><dt>{name}</dt><dd>{ids.join(', ')||'None'}</dd></div>)}</dl></>:<p className="muted">Select a recorded rewrite to inspect what it read and changed.</p>}</div><div className="dependency-summary"><h3>Positive-resource order</h3><p>Shared reads alone do not impose an order. Missing edges do not prove independence.</p>{state?.dependencies.length?<ul>{state.dependencies.map(e=><li key={`${e.before}:${e.after}`}>{e.before} → {e.after}<small>{e.witnesses.map(w=>`${w.kind}: ${w.atom}`).join('; ')}</small></li>)}</ul>:<p className="muted">No direct resource edges in this trace.</p>}{state?.report&&<p>Outcome: {state.report.outcome}. Payload: {state.report.payload??'unmeasured'}{state.report.recurrence?` · recurrence ${state.report.recurrence.first} → ${state.report.recurrence.repeated}`:''}</p>}</div></div>
    </section>
    <footer>Local execution · No server or account required<span>Limits and recurrence are observations, not proofs of divergence.</span></footer>
  </main>;
}
