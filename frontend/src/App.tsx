import {useEffect,useRef,useState} from 'react';
import {EngineClient,type WorkerPort} from './controller';
import type {Analysis,Command,EngineState,Reply} from './types';
import {GraphCanvas} from './GraphCanvas';
import {atomLabel,kindName,displayName,stopReason} from './presentation';
import {LatestQuery} from './navigation';
import {Timeline} from './Timeline';
import {Rules,HistoryAnalysis} from './Inspection';
import {Editor,EditorDiagnostics} from './Editor';
import {EventList} from './EventList';
import {DatasetInput,WorkItems} from './Datasets';
import {LambdaInput,LambdaResult} from './Lambda';
import {Guide} from './Guide';
import {parseProject,readLocal,readUtf8,saveLocal,type Theme,type Project} from './projects';
import manifest from '../../examples/manifest.json';
import './workspace.css';
import './application.css';
const examples=import.meta.glob('../../examples/{programs,behaviour,independent,practical,lambda}/*.graph',{query:'?raw',import:'default',eager:true}) as Record<string,string>;
const choices=manifest.map(m=>{const key='../../examples/'+m.file;if(!(key in examples))throw new Error(`Missing gallery source: ${m.file}`);return key;}).sort();
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
  const [atomPage,setAtomPage]=useState(0),[inspected,setInspected]=useState<Reply['inspection']>(null);
  const [maxWork,setMaxWork]=useState('1000000'),[maxCandidates,setMaxCandidates]=useState('1000');
  const [theme,setTheme]=useState<Theme>('system'),[guide,setGuide]=useState(location.hash==='#guide'),[storageError,setStorageError]=useState(''),[savedDraft,setSavedDraft]=useState<string|null>(null);
  const [sourceError,setSourceError]=useState<Reply['error']>(null),[jumpError,setJumpError]=useState(0);
  const [diagnosticTarget,setDiagnosticTarget]=useState<string|null>(null);
  const draft=useRef(source),sourceVersion=useRef(0);
  function edit(text:string){draft.current=text;sourceVersion.current++;setSource(text);setExample(choices.find(k=>examples[k]===text)??'');setSourceError(null);}
  useEffect(()=>{const t=readLocal('reflective-theme'),d=readLocal('reflective-draft');if(['light','dark','system'].includes(t.value??''))setTheme(t.value as Theme);setSavedDraft(d.value);setStorageError(t.error??d.error??'');const navigate=()=>setGuide(location.hash==='#guide');window.addEventListener('hashchange',navigate);return()=>window.removeEventListener('hashchange',navigate);},[]);
  useEffect(()=>{const media=matchMedia('(prefers-color-scheme: dark)');const apply=()=>{document.documentElement.dataset.theme=theme==='system'?(media.matches?'dark':'light'):theme;};apply();media.addEventListener('change',apply);return()=>media.removeEventListener('change',apply);},[theme]);
  function chooseTheme(t:Theme){setTheme(t);const error=saveLocal('reflective-theme',t);if(error)setStorageError(error);}
  const pendingConfig=()=>({max_steps:maxSteps,max_work:maxWork,max_candidates:maxCandidates,seed:seed.trim()?seed:null,detect_cycles:cycles,observer:observer==='all'?all:observer==='value'?valueObserver:observer});
  const [view,setView]=useState('Practical'),[target,setTarget]=useState(0),[seeking,setSeeking]=useState(false),[playing,setPlaying]=useState(false),[speed,setSpeed]=useState('500'),[pace,setPace]=useState('0');
  const [analysis,setAnalysis]=useState<Analysis|null>(null),[analysisError,setAnalysisError]=useState('');
  const navigator=useRef<LatestQuery<Reply>|null>(null),seekVersion=useRef(0),playback=useRef(false),playVersion=useRef(0);
  function publish(reply:Reply){if(!reply.ok){setDiagnostic(`${reply.error?.kind}: ${reply.error?.message}${reply.error?.line?` (line ${reply.error.line}, column ${reply.error.column})`:''}`);return false;}setDiagnostic('');if(reply.state){setInspected(null);const s=reply.state;setState(s);setTarget(Number(s.position));setSelected(id=>id&&(s.graph.atoms[id]||s.previous?.atoms[id])?id:null);setSelectedEvent(Number(s.position)>0?String(Number(s.position)-1):null);setAnalysis(null);setAnalysisError('');}if(reply.trace)setTrace(reply.trace);return true;}
  useEffect(()=>{
    const engine=new EngineClient(()=>new Worker(new URL('./engine.worker.ts',import.meta.url),{type:'module'}) as unknown as WorkerPort);client.current=engine;navigator.current=new LatestQuery(value=>engine.request({command:'jump',state:String(value)}));let alive=true;
    engine.request({command:'validate',source:'program {}'}).then(reply=>{if(alive){publish(reply);setInitialising(false);}}).catch(e=>{if(alive){setDiagnostic(`Initialisation failed: ${String(e)}. Reload the page to retry.`);setInitialising(false);}});
    return ()=>{alive=false;loop.current=false;playback.current=false;navigator.current?.invalidate();epoch.current++;engine.dispose();client.current=null;};
  },[]);
  async function command(payload:Command){if(!client.current)return;const token=epoch.current;setBusy(true);try{const reply=await client.current.request(payload);if(token===epoch.current){publish(reply);return reply;}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current)setBusy(false);}}
  async function load(text=source,replaceDraft=false){const version=sourceVersion.current;const reply=await command({command:'load',source:text,config:pendingConfig()});if(reply?.ok){setDataset(n=>n+1);setSelected(null);setSelectedEvent(null);setSessionSource(text);setInspected(null);if(replaceDraft&&version===sourceVersion.current)edit(text);}if(version===sourceVersion.current)setSourceError(text===draft.current?(reply?.error??null):null);}
  async function validateDraft(){const version=sourceVersion.current;const reply=await command({command:'validate',source});if(version===sourceVersion.current)setSourceError(reply?.error??null);else setDiagnostic('Validation result belongs to an earlier draft; validate the current text.');}
  async function importLocal(file:File|undefined,project:boolean){if(!file)return;if(file.size>(project?20:4)*1024*1024){setDiagnostic('File exceeds the import byte limit');return;}const token=epoch.current,version=sourceVersion.current;setBusy(true);try{const text=await readUtf8(file);if(token!==epoch.current)return;if(!project){if(version!==sourceVersion.current)throw Error('Draft changed during file reading; import again to replace it.');edit(text);return;}const p=parseProject(text);const reply=await command({command:'project',source:p.source,trace:p.trace,config:p.config,position:p.position,execution:p.execution});if(reply?.ok&&token===epoch.current){if(version===sourceVersion.current)edit(p.draft);setSessionSource(p.source??'');setMaxSteps(p.config.max_steps);setMaxWork(p.config.max_work);setMaxCandidates(p.config.max_candidates);setSeed(p.config.seed??'');setCycles(p.config.detect_cycles);setObserver(p.config.observer===all?'all':p.config.observer===valueObserver?'value':p.config.observer);chooseTheme(p.presentation.theme);setView(p.presentation.view);setDataset(n=>n+1);setInspected(null);}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current)setBusy(false);}}
  function exportProject(){try{const p:Project={schema:'reflective-project-v1',source:state?(sessionSource||null):source,draft:source,config:pendingConfig(),trace:state?trace:null,execution:state&&trace?JSON.parse(trace).settings:null,position:state?.position??'0',presentation:{theme,view}};const text=JSON.stringify(p,null,2);parseProject(text);download(text,'reflective-graph.project.json');}catch(e){setDiagnostic(String(e));}}
  async function run(){if(!client.current)return;const token=epoch.current;loop.current=true;setBusy(true);try{while(loop.current&&token===epoch.current){const reply=await client.current.request({command:'advance',batch});if(token!==epoch.current)break;publish(reply);if(!reply.ok||!reply.state?.can_advance)break;await new Promise(resolve=>setTimeout(resolve,Number(pace)));}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current){loop.current=false;setBusy(false);}}}
  async function stop(){pausePlayback();navigator.current?.invalidate();seekVersion.current++;setSeeking(false);loop.current=false;const token=++epoch.current;setBusy(true);try{const reply=await client.current!.interrupt();if(token===epoch.current){if(reply)publish(reply);setDiagnostic('Stopped at the last acknowledged state. Any unfinished batch was discarded; continue preserves this prefix.');}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current)setBusy(false);}}
  async function reset(){const reply=await command({command:'reset'});if(reply?.ok){setSelected(null);setSelectedEvent(null);setDataset(n=>n+1);}}
  async function importFile(file:File|undefined){if(!file)return;if(file.size>16*1024*1024){setDiagnostic('ResourceExhaustion: trace exceeds 16 MiB');return;}const token=epoch.current;setBusy(true);try{const text=await readUtf8(file);if(token!==epoch.current)return;const reply=await command({command:'import',trace:text});if(token===epoch.current&&reply?.ok){setSelected(null);setSelectedEvent(null);setDataset(n=>n+1);setSessionSource('');}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current)setBusy(false);}}
  async function recover(){const token=++epoch.current;setBusy(true);try{const reply=await client.current!.interrupt();if(token===epoch.current){if(reply)publish(reply);else publish(await client.current!.request({command:'validate',source:'program {}'}));}}catch(e){if(token===epoch.current)setDiagnostic(String(e));}finally{if(token===epoch.current)setBusy(false);}}
  async function seek(value:number){
    if(!Number.isSafeInteger(value)||!navigator.current)return;
    const token=epoch.current,version=++seekVersion.current;setTarget(value);setSeeking(true);
    try{const reply=await navigator.current.request(value);if(reply&&token===epoch.current){publish(reply);return reply;}}
    catch(e){if(token===epoch.current&&version===seekVersion.current)setDiagnostic(String(e));}
    finally{if(token===epoch.current&&version===seekVersion.current)setSeeking(false);}
  }
  function pausePlayback(){playVersion.current++;playback.current=false;setPlaying(false);}
  async function playHistory(){
    if(!state||seeking)return;const version=++playVersion.current;playback.current=true;setPlaying(true);const token=epoch.current,end=Number(state.endpoint);let position=Number(state.position);
    try{if(position===end){const reply=await seek(0);if(!reply?.ok)return;position=0;}while(version===playVersion.current&&playback.current&&token===epoch.current&&position<end){await new Promise(resolve=>setTimeout(resolve,Number(speed)));if(version!==playVersion.current||!playback.current||token!==epoch.current)break;const reply=await seek(++position);if(!reply?.ok)break;}}
    finally{if(version===playVersion.current&&token===epoch.current){playback.current=false;setPlaying(false);}}
  }
  async function analyse(event?:string){
    if(!client.current)return;const token=epoch.current;setBusy(true);setAnalysis(null);setAnalysisError('');
    try{const reply=await client.current.request({command:'analyse',event:event??null});if(token===epoch.current){if(reply.ok)setAnalysis(reply.analysis??null);else setAnalysisError(`${reply.error?.kind}: ${reply.error?.message}`);}}
    catch(e){if(token===epoch.current)setAnalysisError(String(e));}finally{if(token===epoch.current)setBusy(false);}
  }
  const locked=busy||initialising||seeking||playing;
  const atom=selected&&state?(state.graph.atoms[selected]??state.previous?.atoms[selected]):null;
  const event=state?.events.find(e=>e.id===selectedEvent);
  const rule=state?.rules.find(r=>r.head===selected);
  const matches=state?Object.entries(state.graph.atoms).filter(([id,a])=>`${displayName(state,id)} ${kindName(a)} ${atomLabel(a)}`.toLowerCase().includes(atomSearch.toLowerCase())):[];
  const lastPage=Math.max(0,Math.ceil(matches.length/100)-1),currentPage=Math.min(atomPage,lastPage),labels=matches.slice(currentPage*100,currentPage*100+100);
  const exampleInfo=manifest.find(m=>example.endsWith(m.file));
  return <main>
    <header className="app-header"><div className="wordmark">Reflective Graph</div><nav aria-label="Application"><a href="#workspace">Workspace</a> <a href="#guide">Guide</a> <label>Theme <select aria-label="Theme" value={theme} onChange={e=>chooseTheme(e.target.value as Theme)}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></label></nav></header>
    {guide&&<Guide/>}<div hidden={guide} className="workbench-content">
    <section className="toolbar" aria-label="Execution controls">
      <button className="primary" onClick={()=>void load()} disabled={locked}>Load program</button>
      <button onClick={()=>command({command:'advance',batch:'1'})} disabled={locked||!state?.can_advance}>Step</button>
      <button onClick={run} disabled={locked||!state?.can_advance}>Run</button>
      <button onClick={stop} disabled={!(busy||seeking||playing)||initialising}>Stop</button>
      <button onClick={reset} disabled={locked||!state}>Reset</button>
      <div className="toolbar-gap"/>
      <button disabled={locked||!trace} onClick={()=>trace&&download(trace,'reflective-graph.trace.json')}>Export trace</button>
      <label className={`file-button ${locked?'disabled':''}`}>Import trace<input aria-label="Import trace" type="file" accept=".json,application/json" disabled={locked} onChange={e=>{void importFile(e.target.files?.[0]);e.target.value='';}}/></label>
    </section>
    <details className="local-files"><summary>Source and project files</summary><p>Import source replaces the draft only. Import project replaces the loaded session after verification. Trace files retain identities and history; source files do not.</p><button onClick={()=>download(source,'reflective-graph.graph')}>Export source</button><label className="file-button">Import source<input aria-label="Import source" type="file" accept=".graph,.txt" disabled={locked} onChange={e=>{void importLocal(e.target.files?.[0],false);e.target.value='';}}/></label><button disabled={locked} onClick={exportProject}>Export project</button><label className="file-button">Import project<input aria-label="Import project" type="file" accept=".json" disabled={locked} onChange={e=>{void importLocal(e.target.files?.[0],true);e.target.value='';}}/></label><button onClick={()=>{const error=saveLocal('reflective-draft',source);setStorageError(error??'Draft saved in this browser; file export is the backup.');if(!error)setSavedDraft(source);}}>Save draft locally</button><button disabled={savedDraft===null||locked} onClick={()=>savedDraft!==null&&edit(savedDraft)}>Restore saved draft</button></details>{storageError&&<p className="storage-notice" role="status">{storageError}</p>}
    <div className="status-strip" aria-live="polite"><span className={`status-dot ${busy?'working':''}`}/><strong data-testid="status">{initialising?'Loading engine':busy?'Working':state?.status??'No program loaded'}</strong><span>{state?`State ${state.position} / ${state.endpoint} · ${Object.keys(state.graph.atoms).length} atoms · ${state.rules.length} live rules`:''}</span>{state&&sessionSource!==source&&<span className="draft-note">{sessionSource?'Editor differs from loaded source':'Imported history · no source attached'}</span>}</div>
    {diagnostic&&<div className="diagnostic" role="alert">{diagnostic} <button disabled={locked} onClick={recover}>Recover engine</button></div>}
    {analysisError&&view!=='History'&&<p className="diagnostic" role="alert">Analysis incomplete: {analysisError}. No applicability or independence conclusion is available.</p>}
    {state&&<><div className="outcome-context"><p>{stopReason(state)}</p><span>Loaded observer: {state.observer?.trim().replaceAll('\n',' · ')??'See trace metadata'}. Graph display filters are separate.</span></div><Timeline state={state} target={target} disabled={busy||initialising} playing={playing} onSeek={n=>{pausePlayback();void seek(n);}} onPlay={()=>void playHistory()} onPause={pausePlayback} speed={speed} setSpeed={setSpeed}/></>}
    <div className="workspace">
      <section className="source-pane" aria-label="Program editor"><div className="section-heading"><h1>Program</h1></div>
        <label className="example-label">Example<select aria-label="Example" value={example} disabled={locked} onChange={e=>{if(e.target.value)edit(examples[e.target.value]);}}><option value="">Custom source / dataset</option>{choices.map(k=><option key={k} value={k}>{k.replace('../../examples/','').replace('.graph','')}</option>)}</select></label>
        {exampleInfo&&<div className="example-context"><p>{exampleInfo.purpose} {exampleInfo.expected} Reference profile: {exampleInfo.observer}; {exampleInfo.limit}. Different source or settings may change the result.</p><button disabled={locked} onClick={()=>{const p=exampleInfo.profile;setMaxSteps(p.max_steps);setSeed(p.seed);setCycles(p.detect_cycles);setMaxWork(p.max_work);setMaxCandidates(p.max_candidates);setObserver(p.observer==='observers/value.observer'?'value':'all');}}>Apply example settings</button> Settings take effect on load.</div>}
        <DatasetInput request={command} load={s=>load(s,true)} disabled={locked} maxSteps={maxSteps}/>
        <LambdaInput request={command} prepare={edit} disabled={locked}/>
        <div className="editor-controls"><button disabled={locked} onClick={()=>void validateDraft()}>Validate draft</button><EditorDiagnostics source={source} error={sourceError} onJump={key=>{setDiagnosticTarget(key);setJumpError(n=>n+1);}}/></div>
        <Editor value={source} onChange={edit} diagnostic={sourceError} jump={jumpError} target={diagnosticTarget}/>
        <details className="settings"><summary>Run settings <span>Applied on load</span></summary><div className="settings-grid">
          <label>Absolute step bound<input aria-label="Absolute step bound" inputMode="numeric" value={maxSteps} onChange={e=>setMaxSteps(e.target.value)} disabled={locked}/></label>
          <label>Seed · blank is deterministic<input aria-label="Seed" value={seed} onChange={e=>setSeed(e.target.value)} disabled={locked}/></label>
          <label>Search work per step<input aria-label="Search work per step" value={maxWork} onChange={e=>setMaxWork(e.target.value)} disabled={locked}/></label><label>Candidate limit<input aria-label="Candidate limit" value={maxCandidates} onChange={e=>setMaxCandidates(e.target.value)} disabled={locked}/></label>
          <label>Observer<select aria-label="Observer" value={observer} onChange={e=>setObserver(e.target.value)} disabled={locked}><option value="all">Complete carrier</option><option value="value">Value links</option>{!['all','value'].includes(observer)&&<option value={observer}>Imported observer declaration</option>}</select></label>
          <label>Commits per batch<select aria-label="Commits per batch" value={batch} onChange={e=>setBatch(e.target.value)} disabled={locked}><option>1</option><option>2</option><option>5</option><option>10</option></select></label>
          <label>Execution pacing<select aria-label="Execution pacing" value={pace} onChange={e=>setPace(e.target.value)} disabled={locked}><option value="0">Fast</option><option value="250">0.25 seconds per batch</option><option value="1000">1 second per batch</option></select></label>
          <label className="checkbox"><input type="checkbox" checked={cycles} onChange={e=>setCycles(e.target.checked)} disabled={locked}/> Detect recurrence</label>
        </div><p>Blank seed selects deterministic order; a seed selects repeatable pseudo-random scheduling. Limits and observer changes apply only when loading a new run.</p>{state?.settings&&<details><summary>Loaded run settings</summary><p>Scheduler: {state.settings.plan?.seed ? `seeded (${state.settings.plan.seed})` : 'deterministic'}. Step bound: {state.settings.plan?.max_steps??'not recorded'}. Search work per step: {state.settings.search?.max_work}; candidate limit: {state.settings.search?.max_candidates}. Recurrence detection: {state.settings.plan?.detect_cycles?'on':'off'}.</p><pre>{JSON.stringify(state.settings,null,2)}</pre></details>}</details>
      </section>
      <section className="graph-pane"><div className="section-heading"><h2>{view}</h2><span>State {state?.position??'—'}</span></div>
        <nav className="view-tabs" aria-label="Graph views">{['Practical','Internal','Rules','History'].map(name=><button key={name} aria-pressed={view===name} onClick={()=>setView(name)}>{name}</button>)}</nav>
        {state&&view==='Practical'&&<WorkItems state={state} select={setSelected}/>}
        {state&&<LambdaResult state={state}/>}
        {state?<>{(view==='Practical'||view==='Internal')&&<GraphCanvas key={`${dataset}:${view}`} state={state} selected={selected} onSelect={setSelected} practical={view==='Practical'}/>}{view==='Rules'&&<Rules state={state} select={setSelected} analysis={analysis} onAnalyse={()=>void analyse()} disabled={locked}/>} {view==='History'&&<HistoryAnalysis state={state} analysis={analysis} error={analysisError} onAnalyse={e=>void analyse(e)} selectedEvent={selectedEvent} selectEvent={(id,atom)=>{if(!locked)void seek(Number(id)+1).then(reply=>{if(reply?.ok&&atom)setSelected(atom);});}} disabled={locked}/>}</>:<div className="welcome"><p>Load a program to view its graph.</p></div>}
        <div className="inspector"><div className="section-heading"><h2>Atom inspector</h2><span>{selected?`ID ${selected}`:''}</span></div>
          <input aria-label="Find atom" className="atom-search" placeholder="Find by identity, kind or label" value={atomSearch} onChange={e=>{setAtomSearch(e.target.value);setAtomPage(0);setInspected(null);}}/>
          <select aria-label="Select atom" value={selected??''} onChange={e=>setSelected(e.target.value||null)}><option value="">Select an atom ({labels.length} shown)</option>{labels.map(([id,a])=><option key={id} value={id}>{displayName(state!,id)} · {kindName(a)}</option>)}</select>
          <div className="pagination"><button disabled={!currentPage} onClick={()=>setAtomPage(currentPage-1)}>Previous atoms</button> Page {currentPage+1} / {lastPage+1} · {matches.length} matches <button disabled={currentPage===lastPage} onClick={()=>setAtomPage(currentPage+1)}>Next atoms</button><button disabled={locked||!state} onClick={async()=>{const r=await command({command:'inspect',query:atomSearch,offset:String(currentPage*100),limit:'100'});setInspected(r?.inspection);}}>Query engine page</button>{inspected&&<span>Engine query: {inspected.atoms.length} of {inspected.total} matches (kind, name and identity).</span>}</div>
          {atom?<dl><dt>Inspection state</dt><dd>{state?.graph.atoms[selected!] ? state?.position : `Before event ${selectedEvent}: removed from state ${state?.position}`}</dd><dt>Kind / label</dt><dd>{kindName(atom)} · {atomLabel(atom)}</dd><dt>Owned fields</dt><dd>{atom.fields.length?atom.fields.map((id,i)=><button className="text-button" key={i} onClick={()=>setSelected(id)}>slot {i} → {id}</button>):'None'}</dd><dt>Quotation</dt><dd>{!state?.graph.atoms[selected!]?'Removed atom: inspect the preceding state for quotation metadata':state?.quoted[selected!]!==undefined?`Quoted by head ${state.quoted[selected!]}`:'Not quoted'}</dd><dt>Membership records</dt><dd>{Object.entries(state!.graph.atoms[selected!]?state!.graph.atoms:state!.previous?.atoms??{}).filter(([,a])=>['Lhs','Rhs'].includes(kindName(a))&&a.fields.includes(selected!)).map(([id])=>id).join(', ')||'None'}</dd>{rule&&<><dt>Rule</dt><dd>{rule.active?'Active':'Inactive'} · {rule.meta?'Meta':'Object'} · rank {rule.rank} · revision {rule.revision}</dd></>}</dl>:<p className="muted">Select an atom to inspect it.</p>}
        </div>
      </section>
    </div>
    <section className="history" aria-label="Recorded history"><div className="section-heading"><h2>Recorded rewrites <span className="count">{state?.events.length??0}</span></h2></div>
      {event&&state&&<div className="change-summary"><h3>Event {event.id}: state {event.redex.state} → {event.post_state}</h3><p>Rule <button className="text-button" onClick={()=>setSelected(event.redex.source)}>{displayName(state,event.redex.source)}</button> · exact revision <button className="text-button" onClick={()=>setSelected(event.redex.revision)}>#{event.redex.revision}</button>. Binding: {event.redex.binding.map(([a,b])=>`${a} → ${b}`).join(', ')||'Empty'}.</p><button disabled={locked} onClick={()=>void seek(Number(event.redex.state))}>Inspect before</button><button disabled={locked} onClick={()=>void seek(Number(event.post_state))}>Inspect after</button>{['consumed','produced','preserved','entered','exited','altered'].map(kind=><div key={kind}><strong>{kind}: </strong>{event.footprint[kind]?.map(id=><button key={id} className="text-button" onClick={()=>setSelected(id)}>{displayName(state,id)}</button>)}{!event.footprint[kind]?.length&&'None'}</div>)}</div>}
      <div className="history-grid"><EventList state={state} selected={selectedEvent} disabled={locked} seek={n=>void seek(n)}/><div className="event-detail">{event?<><h3>Event {event.id}</h3><p>Applied rule {event.redex.source}, exact revision {event.redex.revision}.</p><dl>{Object.entries(event.footprint).map(([name,ids])=><div key={name}><dt>{name}</dt><dd>{ids.join(', ')||'None'}</dd></div>)}</dl></>:<p className="muted">Select a rewrite to inspect its changes.</p>}</div><div className="dependency-summary"><h3>Positive-resource order</h3>{!!state&&state.dependencies.length>200&&<p>Summary shows 200 of {state.dependencies.length} edges. Use History and select an event for its witnesses.</p>}<p>Shared reads alone do not impose an order. Missing edges do not prove independence.</p>{state?.dependencies.length?<ul>{state.dependencies.slice(0,200).map(e=><li key={`${e.before}:${e.after}`}>{e.before} → {e.after}<small>{e.witnesses.map(w=>`${w.kind}: ${w.atom}`).join('; ')}</small></li>)}</ul>:<p className="muted">No direct resource edges in this trace.</p>}{state?.report&&<p>Outcome: {state.report.outcome}. Payload: {state.report.payload??'unmeasured'}{state.report.recurrence?` · recurrence ${state.report.recurrence.first} → ${state.report.recurrence.repeated}`:''}</p>}</div></div>
    </section>
  </div></main>;
}
