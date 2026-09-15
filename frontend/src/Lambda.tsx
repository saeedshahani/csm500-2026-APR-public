import {useState} from 'react';
import type {Command,EngineState,Reply} from './types';
const examples=import.meta.glob('../../examples/lambda/*.lambda',{query:'?raw',import:'default',eager:true}) as Record<string,string>;

export function LambdaInput({request,prepare,disabled}:{request:(c:Command)=>Promise<Reply|undefined|null>;prepare:(s:string)=>void;disabled:boolean}){
  const [source,setSource]=useState('((\\x. x) (\\y. y))');
  return <details className="lambda-input"><summary>Lambda program</summary>
    <p>Closed terms, evaluated call-by-name to weak head form. Use <code>(\x. x)</code> for a function and <code>(f a)</code> for application. Every variable needs a binder.</p>
    <label>Lambda example<select aria-label="Lambda example" disabled={disabled} value="" onChange={e=>setSource(examples[e.target.value])}><option value="">Choose a term</option>{Object.keys(examples).sort().map(path=><option key={path} value={path}>{path.split('/').pop()?.replace('.lambda','')}</option>)}</select></label>
    <label>Source term<textarea aria-label="Lambda source" value={source} disabled={disabled} onChange={e=>setSource(e.target.value)}/></label>
    <button disabled={disabled} onClick={async()=>{const reply=await request({command:'lambda_compile',source});if(reply?.ok&&reply.compiled)prepare(reply.compiled);}}>Prepare lambda graph</button>
    <p>Replaces the graph draft; press Load program next. Steps include bookkeeping; readback changes only at beta reductions. The source is retained in the generated graph’s opening comment. Resource limits still apply.</p>
  </details>;
}
export function LambdaResult({state}:{state:EngineState}){
  if(!state.lambda)return null;
  return <section className="lambda-result" aria-label="Lambda interpretation"><h3>Lambda term · state {state.position}</h3>
    {state.lambda.error?<p role="status">Readback unavailable: {state.lambda.error}</p>:<pre data-testid="lambda-term">{state.lambda.term}</pre>}
    <p>Binding indices count outward: 0 is the nearest function parameter. This reads the inspected graph, including administrative states; it does not evaluate separately or certify an edited graph. Only NormalForm marks the engine’s completed run.</p>
  </section>;
}
