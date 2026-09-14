import type {EngineState} from './types';
export function Timeline({state,target,disabled,playing,onSeek,onPlay,onPause,speed,setSpeed}:{state:EngineState;target:number;disabled:boolean;playing:boolean;onSeek:(n:number)=>void;onPlay:()=>void;onPause:()=>void;speed:string;setSpeed:(v:string)=>void}){
  const end=Number(state.endpoint);const recurrence=state.report?.recurrence;
  return <section className="timeline" aria-label="Execution timeline">
    <div className="timeline-heading"><h2>Timeline</h2><span>Inspecting state {state.position} of {end}{target!==Number(state.position)?` · loading ${target}`:''}</span></div>
    <input aria-label="Timeline state" type="range" min="0" max={end} step="1" value={target} disabled={disabled||!end} onChange={e=>onSeek(Number(e.target.value))}/>
    <div className="timeline-controls">
      <button disabled={disabled||target===0} onClick={()=>onSeek(0)}>First</button><button disabled={disabled||target===0} onClick={()=>onSeek(target-1)}>Previous</button>
      <label>Inspect state<input aria-label="Inspect state" type="number" min="0" max={end} value={target} disabled={disabled} onChange={e=>{if(e.target.value!=='')onSeek(Math.max(0,Math.min(end,Number(e.target.value))));}}/></label>
      <button disabled={disabled||target===end} onClick={()=>onSeek(target+1)}>Next</button><button disabled={disabled||target===end} onClick={()=>onSeek(end)}>Latest</button>
      <button disabled={disabled||!end} onClick={playing?onPause:onPlay}>{playing?'Pause playback':'Play history'}</button>
      <label>Playback interval<select aria-label="Playback interval" value={speed} onChange={e=>setSpeed(e.target.value)} disabled={playing}><option value="150">0.15 seconds</option><option value="500">0.5 seconds</option><option value="1000">1 second</option></select></label>
    </div>
    <p>State 0 is the initial graph. Event i leads to state i + 1. Playback inspects recorded states; it creates no rewrites.</p>
    {state.position!==state.endpoint&&<p>Historical state is read-only. Return to Latest to continue a paused run.</p>}
    {recurrence&&<p>Repeated observation: <button disabled={disabled} onClick={()=>onSeek(Number(recurrence.first))}>State {recurrence.first}</button> → <button disabled={disabled} onClick={()=>onSeek(Number(recurrence.repeated))}>State {recurrence.repeated}</button>. {recurrence.complete_state_equal===false?'Hidden change: the selected observation repeats, but the complete carrier differs.':recurrence.complete_state_equal===true?'The complete carriers also agree up to isomorphism; identities may differ.':'Complete-carrier comparison is not supplied.'}</p>}
  </section>;
}
