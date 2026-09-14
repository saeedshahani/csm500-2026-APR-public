/// <reference lib="webworker" />
import init, { WorkbenchHandle } from '../pkg/wasm_bridge';
import type { Request, Reply } from './types';
// Attach a rejection handler immediately, even if loading fails before a request.
const ready=init().then(()=>({handle:new WorkbenchHandle(),error:null}),error=>({handle:null,error:String(error)}));
// Serialize messages even during asynchronous WASM initialisation.
let chain:Promise<void>=Promise.resolve();
self.onmessage=(event:MessageEvent<Request>)=>{
  const {generation,id,payload}=event.data;
  chain=chain.then(async()=>{
    let reply:Reply;
    try {const result=await ready;if(!result.handle)throw Error(result.error??'WASM initialisation failed');reply=result.handle.dispatch(JSON.stringify(payload)) as Reply;}
    catch(error){reply={ok:false,error:{kind:'InternalFailure',message:String(error),line:null,column:null},state:null,trace:null};}
    self.postMessage({generation,id,reply});
  });
};
