/// <reference lib="webworker" />
import init, { WorkbenchHandle } from '../pkg/wasm_bridge';
import type { Request, Reply } from './types';
const ready=init().then(()=>new WorkbenchHandle());
// Serialize messages even during asynchronous WASM initialisation.
let chain:Promise<void>=Promise.resolve();
self.onmessage=(event:MessageEvent<Request>)=>{
  const {generation,id,payload}=event.data;
  chain=chain.then(async()=>{
    let reply:Reply;
    try {const handle=await ready;reply=handle.dispatch(JSON.stringify(payload)) as Reply;}
    catch(error){reply={ok:false,error:{kind:'InternalFailure',message:String(error),line:null,column:null},state:null,trace:null};}
    self.postMessage({generation,id,reply});
  });
};
