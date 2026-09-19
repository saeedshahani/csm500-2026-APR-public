import {useEffect,useState} from 'react';
import {type Members,type PlatformClient} from './platform-client';

export function ProjectMembers({api,project}:{api:PlatformClient;project:string}){
  const [data,setData]=useState<Members|null>(null),[uid,setUid]=useState(''),[role,setRole]=useState<'viewer'|'editor'>('viewer');
  const [busy,setBusy]=useState(false),[error,setError]=useState('');
  useEffect(()=>{let active=true;void api.members(project).then(value=>{if(active)setData(value);}).catch(e=>{if(active)setError(String(e));});return()=>{active=false;};},[api,project]);
  async function update(uid:string,role:'viewer'|'editor'|null){
    setBusy(true);setError('');
    try{setData(await api.setMember(project,uid,role));setUid('');}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}
  }
  return <details className="platform-section project-members"><summary>Project access</summary><p>Owners manage access. Editors save versions and run or cancel jobs. Viewers inspect and download. Share the test account UID shown after sign-in; this is not an email invitation.</p>{error&&<p role="alert">{error}</p>}{data&&<><p>Owner: <code>{data.owner_uid}</code></p><ul>{data.members.map(member=><li key={member.uid}><code>{member.uid}</code><span>{member.role}</span><button disabled={busy} onClick={()=>void update(member.uid,null)}>Remove {member.uid}</button></li>)}</ul></>}<form onSubmit={e=>{e.preventDefault();void update(uid,role);}}><fieldset disabled={busy}><label>Member UID<input required maxLength={128} value={uid} onChange={e=>setUid(e.target.value)}/></label><label>Member role<select value={role} onChange={e=>setRole(e.target.value as 'viewer'|'editor')}><option value="viewer">Viewer</option><option value="editor">Editor</option></select></label><button disabled={busy||!uid.trim()}>Set member role</button></fieldset></form></details>;
}
