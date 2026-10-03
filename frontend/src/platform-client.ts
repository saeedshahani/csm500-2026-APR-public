import {hostedOrigins} from './hosted-config.ts';
export type ProjectRole = 'owner'|'editor'|'viewer';
export type ProjectRecord = {id:string;name:string;created_at:string;owner_uid?:string;role?:ProjectRole};
export type Members = {owner_uid:string;members:{uid:string;role:'editor'|'viewer'}[]};
export type DatasetRecord = {id:string;project_id:string;name:string;schema:string;items:number;document_sha256:string;source_sha256:string;validated_engine_sha256:string;parent_dataset_id:string|null;created_at:string};
export type RunSummary = {schema:string;trace_sha256:string;outcome:string|null;commits:number;initial:{atoms:number;fields:number};final:{atoms:number;fields:number};consumed_total:number;produced_total:number;altered_rule_identities:string[];limitation:string};
export type TokenProvider = ()=>Promise<string>;
export type WorkflowRequest = {source:string;contract:unknown;max_steps?:number;meta_burst?:number};
export type WorkflowReport = {verdict:string;guarantee:string;proof_scope:string;unfinished:number;work_potential:string;records:{ticket:string;payload:string;stage:string}[]};
export type WorkflowResult = {verdict?:string;reason?:string;outcome?:string;completion_step_bound?:string;report?:WorkflowReport;trace?:unknown;guarantee?:string;proof_scope?:string;unfinished?:number;work_potential?:string;records?:WorkflowReport['records'];engine_sha256:string};
export type VersionRecord = ProjectRecord & {project_id:string;source_sha256:string;validated_engine_sha256:string;parent_version_id?:string|null;application?:{schema:string;rows:number;policy:string;input_sha256:string}};
export type JobSettings = {max_steps:number;max_work:number;max_candidates:number;seed:string|null;detect_cycles:boolean;deadline_ms:number;execution_profile?:'reference-v1'|'indexed-v2'|'framed-v3'|'joined-v4'};
export type MonthPage = Page<{id:number;fields:string[];source_month:string;month_number:string|null;status:string}> & {complete:boolean;normalised:number;invalid_total:number;trace_sha256:string};
export type JobRecord = {id:string;project_id:string;version_id:string;source_sha256:string;engine_sha256:string;settings:JobSettings;status:string;outcome:string|null;commits:number|null;trace_sha256:string|null;failure:string|null;created_at:string;started_at:string|null;finished_at:string|null;parent_job_id?:string|null;idempotency_key:string;checkpoint_sha256?:string|null;elapsed_ms?:number;pause_requested_at?:string|null;attempts?:{number:number;status:string;started_at:string;finished_at:string|null}[]};
export type Page<T> = {items:T[];total:number;next_offset:number|null};
export const defaultJobSettings:JobSettings = {max_steps:100,max_work:1000000,max_candidates:1000,seed:null,detect_cycles:false,deadline_ms:10000};
export const finished = (job:JobRecord) => ['completed','cancelled','timed_out','failed','interrupted'].includes(job.status);
const id = (value:string) => {
  if (!/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(value)) throw Error('Invalid platform identity');
  return value;
};

export async function readBounded(response:Response, maximum:number):Promise<Uint8Array<ArrayBuffer>> {
  if (Number(response.headers.get('Content-Length')) > maximum) { await response.body?.cancel(); throw Error('Response exceeds the byte limit'); }
  if (!response.body) throw Error('Empty API response');
  const reader=response.body.getReader(), chunks:Uint8Array[]=[]; let size=0;
  try {
    for (;;) {
      const {done,value}=await reader.read(); if(done)break;
      size+=value.length; if(size>maximum)throw Error('Response exceeds the byte limit'); chunks.push(value);
    }
  } catch(error) { await reader.cancel(); throw error; }
  finally { reader.releaseLock(); }
  const bytes=new Uint8Array(size); let offset=0;
  for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
  return bytes;
}

// Transport only. No graph semantics, credentials in persistent storage, or automatic POST retries.
export class PlatformClient {
  readonly url:string;
  #token:string|TokenProvider;
  #closed=false;
  #pending=new Set<AbortController>();
  #submissions=new Map<string,string>();
  #serial:Promise<void>=Promise.resolve();
  #hosted:boolean;
  #large=false;
  constructor(url:string,token:string|TokenProvider) {
    const parsed=new URL(url);
    const hosted=hostedOrigins.includes(parsed.origin)&&parsed.pathname==='/api';
    this.#hosted=hosted;
    if((!hosted&&(parsed.protocol!=='http:'||parsed.hostname!=='127.0.0.1'||parsed.pathname!=='/'))||parsed.username||parsed.password||parsed.search||parsed.hash)
      throw Error('Use the local API address: http://127.0.0.1:8787');
    if(typeof token!=='function'&&!/^[A-Za-z0-9_-]{32,128}$/.test(token))throw Error('The API token must contain 32–128 letters, digits, underscores or hyphens');
    if(hosted&&typeof token!=='function')throw Error('Hosted API requires Firebase sign-in');
    this.url=parsed.origin+(hosted?'/api':'');this.#token=token;
  }
  close(){this.#closed=true;this.#token='';for(const request of this.#pending)request.abort();this.#pending.clear();this.#submissions.clear();}
  #request(route:string,data?:unknown,maximum=1024*1024):Promise<{bytes:Uint8Array<ArrayBuffer>;headers:Headers}> {
    if(!this.#hosted)return this.#send(route,data,maximum);
    // The hosted pilot has one fenced metadata owner. Do not make its own
    // parallel UI panels compete for that lease. Mutations are never retried.
    const result=this.#serial.then(()=>this.#send(route,data,maximum));
    this.#serial=result.then(()=>{},()=>{});return result;
  }
  async #send(route:string,data?:unknown,maximum=1024*1024):Promise<{bytes:Uint8Array<ArrayBuffer>;headers:Headers}> {
    if(this.#closed)throw Error('API disconnected');
    const control=new AbortController();this.#pending.add(control);
    const timeout=setTimeout(()=>control.abort(),35000);
    try {
      const token=typeof this.#token==='function'?await this.#token():this.#token;
      if(this.#closed||control.signal.aborted)throw Error('API disconnected or token refresh timed out');
      if(!/^[A-Za-z0-9_.-]{1,16384}$/.test(token))throw Error('Invalid authentication token');
      const response=await fetch(this.url+route,{method:data===undefined?'GET':'POST',credentials:'omit',redirect:'error',cache:'no-store',signal:control.signal,
        headers:{Authorization:`Bearer ${token}`,...(data===undefined?{}:{'Content-Type':'application/json'})},...(data===undefined?{}:{body:JSON.stringify(data)})});
      const bytes=await readBounded(response,response.ok?maximum:65536);
      if(this.#closed)throw Error('API disconnected');
      if(!response.ok){let message=`API request failed (${response.status})`;try{const error=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)).error;if(typeof error?.message==='string')message=`${error.code}: ${error.message}`;}catch{/* Keep status if the error body is malformed. */}throw Error(message);}
      return {bytes,headers:response.headers};
    } catch(error) {
      if(this.#closed)throw Error('API disconnected');
      if(error instanceof TypeError)throw Error('Cannot reach the API. Check your connection and server availability.');
      if(control.signal.aborted)throw Error('API request timed out. A submitted job may still exist; retry the same submission to reuse its key.');
      throw error;
    } finally {clearTimeout(timeout);this.#pending.delete(control);}
  }
  async #json<T>(route:string,data?:unknown):Promise<T>{const {bytes}=await this.#request(route,data);return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)) as T;}
  async health(){const health=await this.#json<{api:string;mode:string;engine_sha256:string;uid?:string;project_id?:string;execution_envelope?:string}>('/v1/health');this.#large=!this.#hosted&&health.execution_envelope==='large-local-v1';return health;}
  importMonths(project:string,name:string,csv:string,policy:string){return this.#json<VersionRecord>(`/v1/projects/${id(project)}/month-imports`,{name,csv,policy});}
  monthResults(job:JobRecord,offset=0){return this.#json<MonthPage>(`/v1/jobs/${id(job.id)}/month-results?limit=25&offset=${offset}`);}
  async monthCsv(job:JobRecord){const {bytes}=await this.#request(`/v1/jobs/${id(job.id)}/month-csv`,undefined,4*1024*1024);return new TextDecoder('utf-8',{fatal:true}).decode(bytes);}
  projects(){return this.#json<Page<ProjectRecord>>('/v1/projects?limit=100');}
  createProject(name:string){return this.#json<ProjectRecord>('/v1/projects',{name});}
  members(project:string){return this.#json<Members>(`/v1/projects/${id(project)}/members`);}
  setMember(project:string,uid:string,role:'viewer'|'editor'|null){return this.#json<Members>(`/v1/projects/${id(project)}/members`,{uid,role});}
  async versions(project:string):Promise<VersionRecord[]>{
    const first=await this.#json<Page<VersionRecord>>(`/v1/projects/${id(project)}/versions?limit=100`);
    if(first.next_offset===null)return first.items;
    const second=await this.#json<Page<VersionRecord>>(`/v1/projects/${id(project)}/versions?limit=100&offset=${first.next_offset}`);
    if(second.next_offset!==null)throw Error('Version collection exceeds this client’s limit');
    return [...first.items,...second.items];
  }
  validate(source:string){return this.#json<{valid:true;source_sha256:string;engine_sha256:string}>('/v1/validate',{source});}
  async workflow(action:'check'|'run',request:WorkflowRequest):Promise<WorkflowResult>{
    const {bytes}=await this.#request(`/v1/workflows/${action}`,request,16*1024*1024);
    return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)) as WorkflowResult;
  }
  saveVersion(project:string,name:string,source:string,parent:string|null){return this.#json<VersionRecord>(`/v1/projects/${id(project)}/versions`,{name,source,parent_version_id:parent});}
  async datasets(project:string):Promise<DatasetRecord[]>{
    const first=await this.#json<Page<DatasetRecord>>(`/v1/projects/${id(project)}/datasets?limit=100`);
    if(first.next_offset===null)return first.items;
    const second=await this.#json<Page<DatasetRecord>>(`/v1/projects/${id(project)}/datasets?limit=100&offset=${first.next_offset}`);
    if(second.next_offset!==null)throw Error('Dataset collection exceeds this client’s limit');
    return [...first.items,...second.items];
  }
  saveDataset(project:string,name:string,document:string,parent:string|null){return this.#json<DatasetRecord>(`/v1/projects/${id(project)}/datasets`,{name,document,parent_dataset_id:parent});}
  datasetVersion(project:string,name:string,dataset:string){return this.#json<VersionRecord>(`/v1/projects/${id(project)}/versions`,{name,dataset_id:id(dataset)});}
  report(job:JobRecord){return this.#json<RunSummary>(`/v1/jobs/${id(job.id)}/report`);}
  events(job:JobRecord,offset=0){return this.#json<Page<unknown>&{trace_sha256:string}>(`/v1/jobs/${id(job.id)}/events?limit=10&offset=${offset}`);}
  jobs(project:string,offset=0){return this.#json<Page<JobRecord>>(`/v1/projects/${id(project)}/jobs?limit=25&offset=${offset}`);}
  job(job:string){return this.#json<JobRecord>(`/v1/jobs/${id(job)}`);}
  cancel(job:string){return this.#json<JobRecord>(`/v1/jobs/${id(job)}/cancel`,{});}
  pause(job:string){return this.#json<JobRecord>(`/v1/jobs/${id(job)}/pause`,{});}
  resume(job:string){return this.#json<JobRecord>(`/v1/jobs/${id(job)}/resume`,{});}
  checkpoint(job:JobRecord){if(!job.checkpoint_sha256)throw Error('No verified checkpoint is available');return this.#artifact(`/v1/jobs/${id(job.id)}/checkpoint`,job.checkpoint_sha256,(this.#large?128:16)*1024*1024);}
  async submit(project:string,version:string,settings:JobSettings,parent:string|null=null){
    const signature=JSON.stringify({project,version,settings,parent});
    const key=this.#submissions.get(signature)??crypto.randomUUID();this.#submissions.set(signature,key);
    const result=await this.#json<JobRecord>(`/v1/projects/${id(project)}/jobs`,{version_id:id(version),settings,idempotency_key:key,parent_job_id:parent});
    this.#submissions.delete(signature);return result;
  }
  async #artifact(route:string,expected:string,maximum:number){
    const {bytes,headers}=await this.#request(route,undefined,maximum);
    const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
    if(this.#closed)throw Error('API disconnected');
    if(digest!==expected||headers.get('X-Content-SHA256')!==expected)throw Error('Artefact checksum mismatch; nothing was imported');
    return new TextDecoder('utf-8',{fatal:true}).decode(bytes);
  }
  source(version:VersionRecord){return this.#artifact(`/v1/projects/${id(version.project_id)}/versions/${id(version.id)}/source`,version.source_sha256,(this.#large?16384:256)*1024);}
  dataset(data:DatasetRecord){return this.#artifact(`/v1/projects/${id(data.project_id)}/datasets/${id(data.id)}/document`,data.document_sha256,256*1024);}
  trace(job:JobRecord){if(!job.trace_sha256)throw Error('No verified trace is available');return this.#artifact(`/v1/jobs/${id(job.id)}/trace`,job.trace_sha256,(this.#large?128:16)*1024*1024);}
}
