/** At most one query runs and one latest destination waits. Superseded results
 * resolve to null, so consumers cannot publish intermediate inspection states. */
export class LatestQuery<T> {
  private version=0;
  private active=false;
  private waiting:{value:number;version:number;resolve:(v:T|null)=>void;reject:(e:unknown)=>void}|null=null;
  private query:(value:number)=>Promise<T>;
  constructor(query:(value:number)=>Promise<T>){this.query=query;}
  request(value:number):Promise<T|null>{
    const version=++this.version;
    this.waiting?.resolve(null);
    const result=new Promise<T|null>((resolve,reject)=>{this.waiting={value,version,resolve,reject};});
    void this.drain();return result;
  }
  invalidate(){this.version++;this.waiting?.resolve(null);this.waiting=null;}
  private async drain(){
    if(this.active)return;this.active=true;
    while(this.waiting){const item=this.waiting;this.waiting=null;try{const value=await this.query(item.value);item.resolve(item.version===this.version?value:null);}catch(e){if(item.version===this.version)item.reject(e);else item.resolve(null);}}
    this.active=false;
  }
}
