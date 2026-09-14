import type { Command, Reply, Request, Response } from './types.ts';
export interface WorkerPort {
  onmessage:((event:{data:Response})=>void)|null;
  onerror:((event:{message:string})=>void)|null;
  postMessage(message:Request):void;
  terminate():void;
}
type Pending={id:number;generation:number;resolve:(reply:Reply)=>void;reject:(error:Error)=>void};
/** Transport identities never enter graph semantics. Only acknowledged replies publish. */
export class EngineClient {
  private factory:()=>WorkerPort;
  private worker:WorkerPort;
  private generation=0;
  private nextId=0;
  private pending:Pending|null=null;
  private dead=false;
  private failed=false;
  private recovery:string|null=null;
  private recoveryPosition:string|null=null;
  constructor(factory:()=>WorkerPort){this.factory=factory;this.worker=this.start();}
  private start(){
    const worker=this.factory();
    worker.onmessage=({data})=>{
      const pending=this.pending;
      if(this.dead||!pending||data.generation!==this.generation||data.generation!==pending.generation||data.id!==pending.id)return;
      this.pending=null;
      if(data.reply.ok&&data.reply.trace){this.recovery=data.reply.trace;this.recoveryPosition=data.reply.state?.position??null;}
      pending.resolve(data.reply);
    };
    worker.onerror=({message})=>{if(worker!==this.worker)return;this.failed=true;const p=this.pending;this.pending=null;p?.reject(new Error(message));};
    return worker;
  }
  request(payload:Command):Promise<Reply>{
    if(this.dead)return Promise.reject(new Error('Engine client is disposed'));
    if(this.failed)return Promise.reject(new Error('Engine worker failed; use Recover engine'));
    if(this.pending)return Promise.reject(new Error('An engine command is already in flight'));
    return new Promise((resolve,reject)=>{const id=++this.nextId;this.pending={id,generation:this.generation,resolve,reject};try{this.worker.postMessage({generation:this.generation,id,payload});}catch(error){this.pending=null;this.failed=true;reject(error);}});
  }
  /** Abort unacknowledged work, then rebuild exactly the last acknowledged prefix. */
  async interrupt():Promise<Reply|null>{
    if(this.dead)throw new Error('Engine client is disposed');
    this.generation++;const p=this.pending;this.pending=null;p?.reject(new Error('Interrupted; unacknowledged batch discarded'));
    this.worker.terminate();this.failed=false;this.worker=this.start();
    if(!this.recovery)return null;
    const position=this.recoveryPosition;
    let reply=await this.request({command:'import',trace:this.recovery});
    if(!reply.ok)throw new Error(`Recovery failed: ${reply.error?.message}`);
    if(position!==null&&position!==reply.state?.position){reply=await this.request({command:'jump',state:position});if(!reply.ok)throw new Error(`Recovery inspection failed: ${reply.error?.message}`);}
    return reply;
  }
  dispose(){if(this.dead)return;this.dead=true;this.generation++;this.pending?.reject(new Error('Engine client disposed'));this.pending=null;this.worker.terminate();this.recovery=null;}
}
