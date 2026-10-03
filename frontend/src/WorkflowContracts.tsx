import {useState} from 'react';
import type {PlatformClient,WorkflowRequest,WorkflowResult} from './platform-client';
import routing from '../../examples/practical/workflow-routing.request.json?raw';
import branching from '../../examples/practical/workflow-branching.request.json?raw';

export function WorkflowContracts({busy,inspectorBusy,perform,onTrace}:{busy:boolean;inspectorBusy:boolean;perform:(op:(api:PlatformClient,valid:()=>boolean)=>Promise<void>)=>Promise<void>;onTrace:(text:string)=>Promise<void>}){
  const [request,setRequest]=useState<WorkflowRequest>(()=>JSON.parse(routing) as WorkflowRequest);
  const [name,setName]=useState('Adaptive routing'),[result,setResult]=useState<WorkflowResult|null>(null);
  const report=result?.report??(result?.records?result:null);
  function load(text:string,label:string){
    const value:unknown=JSON.parse(text);
    if(!value||typeof value!=='object'||!('source' in value)||typeof value.source!=='string'||!('contract' in value))throw Error('Expected a workflow request with source and contract');
    setRequest(value as WorkflowRequest);setName(label);setResult(null);
  }
  function run(action:'check'|'run'){void perform(async(api,valid)=>{const response=await api.workflow(action,request);if(valid())setResult(response);});}
  return <details className="platform-section"><summary>Stability contracts and adaptive tasks</summary>
    <p>Check a supported workflow without exploring every future graph. The opt-in runner serves unfinished task identities across rule changes. Completion does not mean that all rule changes must stop.</p>
    <fieldset disabled={busy}><legend>Workflow request</legend>
      <div className="platform-actions"><button onClick={()=>load(routing,'Adaptive routing')}>Adaptive routing example</button><button onClick={()=>load(branching,'Branching task example')}>Branching task example</button></div>
      <label>Import workflow request JSON<input type="file" accept=".json,application/json" onChange={e=>{const file=e.target.files?.[0];e.target.value='';if(!file)return;void perform(async(_api,valid)=>{if(file.size>1024*1024)throw Error('Workflow request exceeds 1 MiB');const text=await file.text();if(valid())load(text,file.name);});}}/></label>
      <p>Selected: {name}</p>
      <label>Maximum steps<input type="number" min="0" max="1000" value={request.max_steps??0} onChange={e=>{setRequest({...request,max_steps:Number(e.target.value)});setResult(null);}}/></label>
      <label>Maximum rule edits between task steps<input type="number" min="0" max="32" value={request.meta_burst??0} onChange={e=>{setRequest({...request,meta_burst:Number(e.target.value)});setResult(null);}}/></label>
      <div className="platform-actions"><button onClick={()=>run('check')}>Check stability contract</button><button onClick={()=>run('run')}>Run checked workflow</button></div>
    </fieldset>
    <details><summary>Source and contract</summary><pre className="event-page">{request.source}</pre><pre className="event-page">{JSON.stringify(request.contract,null,2)}</pre></details>
    {result&&<div role="status"><h3>{result.outcome??result.verdict}</h3>{result.reason&&<p>{result.reason}</p>}{report&&<><p>{report.unfinished} unfinished tasks · remaining work weight {report.work_potential}</p><p>{report.guarantee==='completion-under-all-maximal-orders'?'Tasks finish under every maximal processing order, with sufficient resources.':report.guarantee==='completion-under-persistent-work-scheduler'?'Tasks finish under the bounded task-service runner, with sufficient resources.':report.guarantee}</p>{result.completion_step_bound!==undefined&&<p>Calculated completion bound from the starting state: {result.completion_step_bound} committed steps.</p>}<p>The abstract ranking proof is checked in Lean. Its connection to native graphs is a written argument; the engine is not formally verified.</p><table><caption>Task identities and current statuses</caption><thead><tr><th>Identity</th><th>Payload</th><th>Status</th></tr></thead><tbody>{report.records?.slice(0,100).map(r=><tr key={r.ticket}><td>{r.ticket}</td><td>{r.payload}</td><td>{r.stage}</td></tr>)}</tbody></table>{(report.records?.length??0)>100&&<p>First 100 tasks shown; download the report for the complete result.</p>}</>}
      {result.trace!==undefined&&<button disabled={busy||inspectorBusy} onClick={()=>void perform(async(_api,valid)=>{if(valid())await onTrace(JSON.stringify(result.trace));})}>Inspect workflow trace</button>}
      <button onClick={()=>{const url=URL.createObjectURL(new Blob([JSON.stringify(result,null,2)],{type:'application/json'}));const a=document.createElement('a');a.href=url;a.download='workflow-result.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}}>Download workflow result</button>
    </div>}
    <p>Unsupported means outside this checker’s scope, not divergent. StepLimit and inconclusive do not establish completion. Runs here are not saved as platform jobs; downloaded traces can be imported into the workbench. The examples route tasks, not document contents.</p>
  </details>;
}
