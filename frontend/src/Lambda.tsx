import {useState} from 'react';
import type {Command,EngineState,Reply} from './types';
const examples=import.meta.glob('../../examples/lambda/*.lambda',{query:'?raw',import:'default',eager:true}) as Record<string,string>;

export function LambdaInput({request,prepare,disabled}:{request:(c:Command)=>Promise<Reply|undefined|null>;prepare:(s:string)=>void;disabled:boolean}){
  const [source,setSource]=useState('((\\x. x) (\\y. y))');
  const [profile,setProfile]=useState('closed-cbn-v1');
  const [freeNames,setFreeNames]=useState('database,select');
  return <details className="lambda-input"><summary>Lambda program</summary>
    <p>Call-by-name evaluation to weak head form. Use <code>(\x. x)</code> for a function and <code>(f a)</code> for application. Evaluation stops at a function or an unresolved input; it does not simplify everything inside them.</p>
    <label>Lambda profile<select aria-label="Lambda profile" disabled={disabled} value={profile} onChange={e=>setProfile(e.target.value)}><option value="closed-cbn-v1">Closed · closed-cbn-v1</option><option value="open-cbn-v1">Symbolic inputs · open-cbn-v1</option></select></label>
    {profile==='open-cbn-v1'&&<><label>Unresolved inputs<input aria-label="Unresolved inputs" disabled={disabled} value={freeNames} onChange={e=>setFreeNames(e.target.value)}/></label><p>Comma-separated names. These are symbols, not database connections or external calls.</p><button disabled={disabled} onClick={()=>{setFreeNames('database,select');setSource('((\\source. (select source)) database)');}}>Use symbolic query example</button></>}
    <label>Lambda example<select aria-label="Lambda example" disabled={disabled} value="" onChange={e=>setSource(examples[e.target.value])}><option value="">Choose a term</option>{Object.keys(examples).sort().map(path=><option key={path} value={path}>{path.split('/').pop()?.replace('.lambda','')}</option>)}</select></label>
    <label>Source term<textarea aria-label="Lambda source" value={source} disabled={disabled} onChange={e=>setSource(e.target.value)}/></label>
    <button disabled={disabled} onClick={async()=>{const reply=await request({command:'lambda_compile',source,profile,free_names:profile==='open-cbn-v1'&&freeNames.trim()?freeNames.split(',').map(n=>n.trim()):[]});if(reply?.ok&&reply.compiled)prepare(reply.compiled);}}>Prepare lambda graph</button>
    <p>Replaces the graph draft; press Load program next. Steps include bookkeeping; readback changes only at beta reductions. The source is retained in the generated graph’s opening comment. Resource limits still apply.</p>
  </details>;
}
export function LambdaResult({state}:{state:EngineState}){
  if(!state.lambda)return null;
  return <section className="lambda-result" aria-label="Lambda interpretation"><h3>Lambda term · state {state.position}</h3>
    {state.lambda.error?<p role="status">Readback unavailable: {state.lambda.error}</p>:<pre data-testid="lambda-term">{state.lambda.term}</pre>}
    <p>Binding indices count outward: 0 is the nearest function parameter. Unresolved inputs retain their names. This reads the inspected graph, including administrative states; it does not evaluate separately or certify an edited graph. NormalForm means no graph rule is enabled, not necessarily full beta-normal form.</p>
  </section>;
}
