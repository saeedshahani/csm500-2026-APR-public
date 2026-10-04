import {useEffect,useRef,useState} from 'react';
import source from '../../examples/walkthrough/order.graph?raw';
import {EngineClient,type WorkerPort} from './controller';
import type {EngineState} from './types';
import {GraphCanvas} from './GraphCanvas';
import {tourConfig,tourSteps,tourSelection,visitTourStep} from './walkthrough-state';
import {project} from './presentation';
import './walkthrough.css';

export function Walkthrough(){
  const engine=useRef<EngineClient|null>(null),inFlight=useRef(false),generation=useRef(0);
  const [state,setState]=useState<EngineState|null>(null),[step,setStep]=useState(0),[busy,setBusy]=useState(true),[error,setError]=useState('');
  const [restart,setRestart]=useState(0);
  const scene=useRef<HTMLDivElement>(null),heading=useRef<HTMLHeadingElement>(null),bubble=useRef<HTMLElement>(null);
  const [anchor,setAnchor]=useState<{x:number;y:number;width:number;height:number;bubbleHeight:number;targetBottom:number}|null>(null);
  const current=tourSteps[step],selected=state?tourSelection(state,step):null;

  useEffect(()=>{
    const token=++generation.current,client=new EngineClient(()=>new Worker(new URL('./engine.worker.ts',import.meta.url),{type:'module'}) as unknown as WorkerPort);
    engine.current=client;inFlight.current=true;
    client.request({command:'load',source,config:tourConfig}).then(reply=>{
      if(token!==generation.current)return;
      if(!reply.ok||!reply.state)throw Error(reply.error?.message??'Could not load the sample');
      setState(reply.state);setStep(0);setError('');
    }).catch(e=>{if(token===generation.current)setError(String(e));}).finally(()=>{if(token===generation.current){setBusy(false);inFlight.current=false;}});
    return()=>{generation.current++;client.dispose();engine.current=null;};
  },[restart]);

  async function navigate(index:number){
    if(inFlight.current||!engine.current||!state)return;
    inFlight.current=true;setBusy(true);setError('');const token=generation.current;
    try{
      const next=await visitTourStep(command=>engine.current!.request(command),state,index);
      if(token===generation.current){setState(next);setStep(index);}
    }catch(e){if(token===generation.current)setError(`${String(e)}. Restart the walkthrough to retry.`);}
    finally{if(token===generation.current){inFlight.current=false;setBusy(false);}}
  }
  useEffect(()=>{heading.current?.focus({preventScroll:true});},[step,state]);
  useEffect(()=>{
    const root=scene.current;if(!root)return;
    const update=()=>{
      const target=current.view==='rule'?root.querySelector('.tour-rule-pattern'):current.view==='history'?root.querySelector('.tour-history ol'):root.querySelector(`[data-atom-id="${selected}"] circle, [data-record-id="${selected}"] .field-line`);
      if(!target){setAnchor(null);return;}
      const rect=target.getBoundingClientRect(),area=root.getBoundingClientRect();
      const panel=current.view==='rule'||current.view==='history';
      setAnchor({x:(panel?rect.right+8:rect.left+rect.width/2)-area.left,y:rect.top+rect.height/2-area.top,width:area.width,height:area.height,bubbleHeight:bubble.current?.offsetHeight??280,targetBottom:rect.bottom-area.top});
    };
    const resize=new ResizeObserver(update);resize.observe(root);
    if(bubble.current)resize.observe(bubble.current);
    const mutation=new MutationObserver(update);mutation.observe(root,{subtree:true,attributes:true,attributeFilter:['transform','viewBox']});
    const frame=requestAnimationFrame(update);window.addEventListener('resize',update);
    return()=>{cancelAnimationFrame(frame);resize.disconnect();mutation.disconnect();window.removeEventListener('resize',update);};
  },[selected,current.view,state,step]);

  const packed=state?project(state).links[0]:null;
  const panel=current.view==='rule'||current.view==='history';
  const bubbleX=anchor?panel&&anchor.width>=900?anchor.width-312:Math.max(12,Math.min(anchor.width-292,anchor.x+40)):24;
  const below=anchor?anchor.targetBottom+50:60;
  const graphBubbleY=anchor&&below+anchor.bubbleHeight>anchor.height-16?2*anchor.y-anchor.targetBottom-anchor.bubbleHeight-50:below;
  const bubbleY=anchor?Math.max(24,Math.min(anchor.height-anchor.bubbleHeight-16,panel?(anchor.width>=900?80:anchor.targetBottom+130):graphBubbleY)):60;
  const action=step===3?(Number(state?.endpoint??0)>0?'Inspect packing step':'Apply pack rule'):step===4?(Number(state?.endpoint??0)>1?'Inspect dispatch step':'Apply dispatch rule'):step===5?'Check for more work':'Next';
  return <article className="walkthrough" aria-label="Order walkthrough">
    <header className="tour-header"><div><p className="tour-eyebrow">Guided sample</p><h1>From order to dispatch</h1></div><a href="#workspace">Exit walkthrough</a></header>
    <div className="tour-controls"><span>Step {step+1} of {tourSteps.length}</span><button disabled={busy||step===0||!!error} onClick={()=>void navigate(step-1)}>Back</button>{step<tourSteps.length-1?<button className="primary" disabled={busy||!state||!!error} onClick={()=>void navigate(step+1)}>{busy?'Loading…':action}</button>:<a className="tour-finish" href="#workspace">Finish</a>}<button disabled={busy} onClick={()=>{setBusy(true);setError('');setState(null);setStep(0);setRestart(n=>n+1);}}>Restart walkthrough</button></div>
    {error&&<p className="diagnostic" role="alert">{error}</p>}
    {!state&&!error&&<p role="status">Loading sample…</p>}
    {state&&<>
      <div className="tour-state"><strong>{current.view==='internal'?'Internal':current.view==='rule'?'Rule definition':current.view==='history'?'Recorded history':'Practical'}</strong><span>State {state.position} · {packed?.label} · {state.events.length} recorded events{step>=6?` · ${state.status}`:''}</span></div>
      <div className="tour-scene" ref={scene}>
        {(current.view==='practical'||current.view==='internal')&&<GraphCanvas key={current.view} state={state} selected={selected} onSelect={()=>{}} practical={current.view==='practical'} guided/>}
        {current.view==='rule'&&<div className="tour-rule" aria-label="Pack rule"><h2>pack</h2><div className="tour-rule-pattern"><span>Order 1042</span><strong>pending → packed</strong><span>Warehouse</span></div><dl><dt>Match</dt><dd>A pending relationship between two data items.</dd><dt>Preserve</dt><dd>The two data items and their identities.</dd><dt>Replace</dt><dd>The status record, with a fresh packed record.</dd></dl></div>}
        {current.view==='history'&&<div className="tour-history" aria-label="Sample history"><ol>{['pending','packed','dispatched'].map((label,i)=><li key={label}><span>State {i}</span><strong>{label}</strong>{i<2&&<small>Event {i}: {i===0?'pack':'dispatch'} →</small>}</li>)}</ol><p>Recorded resource dependency: {state.dependencies.some(d=>d.before==='0'&&d.after==='1')?'event 0 → event 1':'not present'}</p><p>{state.events.length} rewrites · endpoint {state.endpoint} · {state.status}</p></div>}
        {anchor&&<svg className="tour-pointer" aria-hidden="true"><line x1={anchor.x} y1={anchor.y} x2={bubbleX+16} y2={bubbleY+16}/><circle cx={anchor.x} cy={anchor.y} r="8"/></svg>}
        <aside ref={bubble} className="tour-bubble" style={{left:bubbleX,top:bubbleY}} aria-label={`Annotation ${step+1}`}><span className="tour-number">{step+1}</span><h2 ref={heading} tabIndex={-1}>{current.title}</h2><p>{current.text}</p></aside>
      </div>
      <section className="tour-discussion" aria-label="Research significance" aria-live="polite">
        <h2>{current.research}</h2><p>{current.implication}</p>
        {step===tourSteps.length-1&&<><h3>Implications for the project</h3><p>The example brings representation, execution and evidence into one inspectable process. A developer can follow an entity across changes, inspect the rules responsible and examine the dependencies that connect the results. These capabilities support the investigation of adaptive workflows, transformation tools and auditable processing.</p><p>The wider research examines guarded self-modification, conditional recovery and alternative graph representations. RQ5 studies computational expressiveness through a separate lambda-calculus encoding: eleven resident rules evaluate closed and explicit-context open terms to weak head form. That correspondence and the recovery results have explicit assumptions. The order example illustrates the underlying mechanisms on a small, finite computation.</p></>}
      </section>
      <details className="tour-source"><summary>Annotated sample source</summary><pre>{source}</pre></details>
    </>}
  </article>;
}
