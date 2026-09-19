import {useEffect,useRef,useState} from 'react';
import {type DatasetRecord,type PlatformClient,type VersionRecord} from './platform-client';

export function ProjectDatasets({api,project,canEdit,onVersion,onDirty}:{api:PlatformClient;project:string;canEdit:boolean;onVersion:(version:VersionRecord)=>void;onDirty:(value:boolean)=>void}){
  const live=useRef(true);
  useEffect(()=>{live.current=true;return()=>{live.current=false;};},[]);
  const [datasets,setDatasets]=useState<DatasetRecord[]>([]),[selected,setSelected]=useState('');
  const [name,setName]=useState(''),[document,setDocument]=useState(''),[parent,setParent]=useState<string|null>(null);
  const [busy,setBusy]=useState(false),[message,setMessage]=useState('');
  useEffect(()=>{let live=true;void api.datasets(project).then(d=>{if(live)setDatasets(d);}).catch(e=>{if(live)setMessage(String(e));});return()=>{live=false;};},[api,project]);
  const data=datasets.find(d=>d.id===selected);
  useEffect(()=>{onDirty(Boolean(name||document));},[name,document,onDirty]);
  async function act(operation:()=>Promise<void>){setBusy(true);setMessage('');try{await operation();}catch(e){if(live.current)setMessage(e instanceof Error?e.message:String(e));}finally{if(live.current)setBusy(false);}}
  return <section className="platform-section" aria-label="Named datasets"><h2>Datasets</h2><p>Save work-items-v1 JSON independently, then compile a program version with its data and selected policy in one graph. Imports are limited to 1,000 items and 256 KiB.</p>
    {message&&<p role="status">{message}</p>}
    <label>Saved dataset<select value={selected} disabled={busy} onChange={e=>setSelected(e.target.value)}><option value="">Choose a dataset</option>{datasets.map(d=><option key={d.id} value={d.id}>{d.name} · {d.items} items</option>)}</select></label>
    {data&&<><p>Document SHA-256: <code>{data.document_sha256}</code></p><div className="platform-actions"><button disabled={busy||!canEdit} onClick={()=>void act(async()=>{if((document||name)&&!window.confirm('Replace the unsaved dataset form?'))return;setDocument(await api.dataset(data));setName(`${data.name} fork`.slice(0,100));setParent(data.id);})}>Fork dataset</button><button disabled={busy||!canEdit} onClick={()=>void act(async()=>{const version=await api.datasetVersion(project,data.name,data.id);if(!live.current)return;onVersion(version);setMessage('Compiled program version saved. Select its run settings below.');})}>Create program from dataset</button></div></>}
    <details><summary>Save dataset version</summary><form onSubmit={e=>{e.preventDefault();void act(async()=>{const saved=await api.saveDataset(project,name,document,parent);if(!live.current)return;setDatasets(d=>[...d,saved]);setSelected(saved.id);setDocument('');setName('');setParent(null);setMessage('Dataset saved; earlier versions are unchanged.');});}}><fieldset disabled={busy||!canEdit}><label>Dataset name<input required maxLength={100} value={name} onChange={e=>setName(e.target.value)}/></label><label>Dataset JSON<textarea required spellCheck={false} value={document} onChange={e=>setDocument(e.target.value)}/></label><p>Example: <code>{'{"schema":"work-items-v1","policy":"fixed","items":[{"id":"task-1","status":"pending"}]}'}</code></p>{parent&&<p>Parent dataset: {parent}</p>}<button disabled={busy||!name.trim()||!document}>Save dataset</button></fieldset></form></details>
  </section>;
}
