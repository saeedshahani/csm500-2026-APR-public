export type Theme='light'|'dark'|'system';
export type Preferences={theme:Theme;view:string};
export type PendingConfig={max_steps:string;max_work:string;max_candidates:string;seed:string|null;detect_cycles:boolean;observer:string};
export type Project={schema:'reflective-project-v1';source:string|null;draft:string;config:PendingConfig;trace:string|null;execution:unknown;position:string;presentation:Preferences};
const object=(v:unknown):v is Record<string,unknown>=>!!v&&typeof v==='object'&&!Array.isArray(v);
function keys(v:Record<string,unknown>,allowed:string[]){if(Object.keys(v).some(k=>!allowed.includes(k))||allowed.some(k=>!(k in v)))throw Error('Unexpected or missing project field');}
const natural=(v:unknown,max:bigint)=>typeof v==='string'&&/^(0|[1-9][0-9]*)$/.test(v)&&v.length<=20&&BigInt(v)<=max;
export function parseProject(text:string):Project {
 if(new TextEncoder().encode(text).length>20*1024*1024)throw Error('Project exceeds 20 MiB');
 const p:unknown=JSON.parse(text);if(!object(p))throw Error('Invalid project');
 keys(p,['schema','source','draft','config','trace','execution','position','presentation']);
 if(p.schema!=='reflective-project-v1'||typeof p.draft!=='string'||!(p.source===null||typeof p.source==='string')||!(p.trace===null||typeof p.trace==='string')||!natural(p.position,10000n))throw Error('Invalid project schema, text or position');
 for(const v of [p.draft,p.source])if(typeof v==='string'&&new TextEncoder().encode(v).length>4*1024*1024)throw Error('Source exceeds 4 MiB');
 if(typeof p.trace==='string'&&new TextEncoder().encode(p.trace).length>16*1024*1024)throw Error('Trace exceeds 16 MiB');
 if(!object(p.config)||!object(p.presentation))throw Error('Missing configuration');
 keys(p.config,['max_steps','max_work','max_candidates','seed','detect_cycles','observer']);
 const c=p.config;if(!natural(c.max_steps,10000n)||!natural(c.max_work,1000000n)||!natural(c.max_candidates,1000n)||!(c.seed===null||natural(c.seed,18446744073709551615n))||typeof c.detect_cycles!=='boolean'||typeof c.observer!=='string'||c.observer.length>4*1024*1024)throw Error('Invalid pending run configuration');
 keys(p.presentation,['theme','view']);if(!['light','dark','system'].includes(String(p.presentation.theme))||!['Practical','Internal','Rules','History'].includes(String(p.presentation.view)))throw Error('Unsupported presentation preference');
 if(p.trace===null&&(p.source===null||p.execution!==null||p.position!=='0'))throw Error('Source-only project requires source and state zero');
 if(p.trace!==null&&!object(p.execution))throw Error('Trace execution settings required');
 return p as Project;
}
export function saveLocal(key:string,value:string):string|null {try{localStorage.setItem(key,value);return null;}catch{return 'Browser storage is unavailable or full. Export source or project to keep your work.';}}
export async function readUtf8(file:File):Promise<string>{return new TextDecoder('utf-8',{fatal:true}).decode(await file.arrayBuffer());}
export function readLocal(key:string):{value:string|null;error:string|null}{try{return {value:localStorage.getItem(key),error:null};}catch{return {value:null,error:'Browser storage is unavailable. Use file export to save your work.'};}}
