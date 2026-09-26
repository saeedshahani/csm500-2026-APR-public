// Request-scoped orchestration: no work is left running after an HTTP response.
import http from 'node:http';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {GoogleAuth,OAuth2Client} from 'google-auth-library';
import {initializeApp,deleteApp} from 'firebase-admin/app';
import {getFirestore} from 'firebase-admin/firestore';
import {identity,productionConfig} from './identity.mjs';
import {FirebaseStore} from './firebase-store.mjs';
import {start} from './server.mjs';
import {Worker} from './worker.mjs';
import {fail} from './store.mjs';
import {backup} from './backup.mjs';

export const PROJECT='reflective-graph-f1cd4', REGION='europe-west2';
export const DISPATCH_ACCOUNT=`graph-dispatch@${PROJECT}.iam.gserviceaccount.com`;
export const ORIGINS=[`https://${PROJECT}.web.app`,`https://${PROJECT}.firebaseapp.com`];
const UUID=/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/;

export function taskRequest(base,job,minute=Math.floor(Date.now()/60000)) {
  if(!UUID.test(job.id)||!Number.isSafeInteger(job.attempts.length))throw Error('Invalid task identity');
  const queue=`projects/${PROJECT}/locations/${REGION}/queues/graph-jobs`;
  return {parent:queue,task:{name:`${queue}/tasks/${job.id}-${job.attempts.length}-${minute}`,
    dispatchDeadline:'120s',httpRequest:{httpMethod:'POST',url:`${base}/_work`,
      headers:{'Content-Type':'application/json'},body:Buffer.from(JSON.stringify({job_id:job.id})).toString('base64'),
      oidcToken:{serviceAccountEmail:DISPATCH_ACCOUNT,audience:base}}}};
}

export async function readBody(req,maximum=1024*1024) {
  const chunks=[];let length=0;
  for await(const b of req){length+=b.length;if(length>maximum)fail(413,'body_limit','Request exceeds byte limit');chunks.push(b);}
  return Buffer.concat(chunks);
}

// Admission is independent of process restarts. A kill switch preserves read/export access.
export async function admission(db,uid,mutation,now=new Date()) {
  const day=now.toISOString().slice(0,10),minute=Math.floor(now.getTime()/60000);
  const control=db.doc('reflectiveControl/pilot'),global=db.doc('reflectiveUsage/global');
  // Firebase UIDs are untrusted path components; use a fixed-length hash.
  const {hash}=await import('./store.mjs');const user=db.doc(`reflectiveUsage/u-${hash(uid)}`);
  await db.runTransaction(async tx=>{
    const [c,g,u]=await Promise.all([tx.get(control),tx.get(global),tx.get(user)]);
    if(mutation&&c.data()?.disabled===true)fail(503,'execution_disabled','New work is temporarily disabled; existing results remain accessible');
    const values=[g.data(),u.data()].map(x=>({day,minute,daily:x?.day===day?x.daily:0,burst:x?.minute===minute?x.burst:0,mutations:x?.day===day?x.mutations:0}));
    for(const [index,x]of values.entries()){
      if(![x.daily,x.burst,x.mutations].every(v=>Number.isSafeInteger(v)&&v>=0))throw Error('Invalid usage counters');
      if(x.daily>=(index?4000:10000)||x.burst>=(index?120:240)||(mutation&&x.mutations>=(index?100:300)))fail(429,'request_limit','Request allowance reached; retry later');
      x.daily++;x.burst++;if(mutation)x.mutations++;
    }
    tx.set(global,values[0]);tx.set(user,values[1]);
  });
}

export async function startHosted({port=Number(process.env.PORT??8080),base=process.env.GRAPH_SERVICE_URL,
  binary=resolve(process.env.GRAPH_ENGINE??'target/release/cli')}={}) {
  productionConfig(process.env.GRAPH_FIREBASE_PROJECT);
  if(!/^https:\/\/graph-api-[a-z0-9-]+\.europe-west2\.run\.app$/.test(base??'')
    && !/^https:\/\/graph-api-[a-z0-9-]+-nw\.a\.run\.app$/.test(base??''))throw Error('Explicit authorised Cloud Run service URL required');
  const auth=await identity({mode:'firebase-production',projectId:PROJECT});
  const app=initializeApp({projectId:PROJECT},'hosted-control'),db=getFirestore(app);
  const oidc=new OAuth2Client(),google=new GoogleAuth({scopes:['https://www.googleapis.com/auth/cloud-platform']});
  async function dispatch(jobs){
    const client=await google.getClient();
    for(const job of jobs.filter(j=>j.status==='queued')){
      const {parent,task}=taskRequest(base,job);
      try{await client.request({url:`https://cloudtasks.googleapis.com/v2/${parent}/tasks`,method:'POST',data:{task},timeout:15000});}
      catch(e){if(Number(e.response?.status)!==409)throw Error('Dispatch unavailable; durable job will be reconciled');}
    }
  }
  async function internal(header){
    if(!/^Bearer [A-Za-z0-9_.-]{1,16384}$/.test(header??''))fail(401,'unauthorized','Dispatch identity required');
    try{const ticket=await oidc.verifyIdToken({idToken:header.slice(7),audience:base}),p=ticket.getPayload();
      if(p?.email!==DISPATCH_ACCOUNT||p.email_verified!==true)throw Error('Wrong dispatch identity');
    }catch{fail(401,'unauthorized','Invalid dispatch identity');}
  }
  const open=()=>FirebaseStore.open({projectId:PROJECT,namespace:`firebase-production:${PROJECT}`,production:true,
    bucket:`${PROJECT}.firebasestorage.app`,storeId:'pilot',verifyArtefacts:false});
  const server=http.createServer(async(req,res)=>{
    res.setHeader('Cache-Control','private, no-store');res.setHeader('X-Content-Type-Options','nosniff');
    const send=(status,value)=>{res.writeHead(status,{'Content-Type':'application/json'});res.end(JSON.stringify(value));};
    let store,api;
    try{
      const url=new URL(req.url,'http://host');
      if(req.method==='GET'&&url.pathname==='/healthz'){send(200,{service:'reflective-graph',api:'v1'});return;}
      const work=['/_work','/_reconcile'].includes(url.pathname);
      if(!work&&!url.pathname.startsWith('/api/v1/'))fail(404,'not_found','Unknown route');
      const origin=req.headers.origin;
      if(origin&&!ORIGINS.includes(origin))fail(403,'origin_denied','Browser origin not allowed');
      if(origin){res.setHeader('Access-Control-Allow-Origin',origin);res.setHeader('Vary','Origin');res.setHeader('Access-Control-Expose-Headers','X-Content-SHA256');}
      if(req.method==='OPTIONS'&&!work){
        const requested=(req.headers['access-control-request-headers']??'').toLowerCase().split(',').map(x=>x.trim()).filter(Boolean);
        if(!origin||!['GET','POST'].includes(req.headers['access-control-request-method'])||requested.some(x=>!['authorization','content-type'].includes(x)))fail(403,'origin_denied','Invalid preflight');
        res.writeHead(204,{'Access-Control-Allow-Methods':'GET, POST','Access-Control-Allow-Headers':'Authorization, Content-Type'});res.end();return;
      }
      if(!['GET','POST'].includes(req.method))fail(405,'method','Unsupported method');
      let uid;
      if(work){if(req.method!=='POST')fail(405,'method','POST required');await internal(req.headers.authorization);}
      else{uid=await auth.authenticate(req.headers.authorization);await admission(db,uid,req.method==='POST');}
      const bytes=await readBody(req);
      store=await open();
      if(work){
        if((await db.doc('reflectiveControl/pilot').get()).data()?.disabled!==true){
          if(url.pathname==='/_work'){
            let body;try{body=JSON.parse(bytes);}catch{fail(400,'invalid_request','Invalid dispatch body');}
            if(!UUID.test(body?.job_id??'')||Object.keys(body).length!==1)fail(400,'invalid_request','Invalid dispatch identity');
            const job=store.state.jobs.find(j=>j.id===body.job_id);
            if(job?.status==='queued')await new Worker(store,binary).execute(job,new AbortController().signal);
          }
          await dispatch(store.state.jobs);
          if(url.pathname==='/_reconcile')await backup(store);
        }
        await store.close();store=null;send(200,{accepted:true});return;
      }
      // The existing API owns the complete authorization/validation contract. Its
      // private listener is held only during this request and never dispatches work.
      api=await start({binary,suppliedStore:store,background:false,suppliedAuth:{mode:auth.mode,namespace:auth.namespace,
        projectId:PROJECT,authenticate:async()=>uid,close:async()=>{}}});
      const response=await fetch(api.url+url.pathname.slice(4)+url.search,{method:req.method,redirect:'error',
        headers:{'Content-Type':req.headers['content-type']??'application/json'},...(req.method==='POST'?{body:bytes}:{})});
      const output=Buffer.from(await response.arrayBuffer());
      // Submission persists before enqueue. Failure can return an uncertain result;
      // the original idempotency key and scheduled reconciliation recover it.
      if(req.method==='POST'&&response.ok)await dispatch(store.state.jobs);
      const headers={'Content-Type':response.headers.get('content-type')??'application/json'};
      if(response.headers.has('x-content-sha256'))headers['X-Content-SHA256']=response.headers.get('x-content-sha256');
      await api.close();api=null;store=null;
      res.writeHead(response.status,headers);res.end(output);
    }catch(e){
      if(api){try{await api.close();}catch{}api=null;store=null;}
      if(store)try{await store.close();}catch{}
      if(!res.headersSent){if(e.status===503)res.setHeader('Retry-After','2');send(e.status??503,{error:{code:e.code??'unavailable',message:e.status?e.message:'Service unavailable; retry with the same submission key'}});}
      else res.end();
      console.error(JSON.stringify({event:'hosted_request_failed',code:e.code??'unavailable'}));
    }
  });
  server.requestTimeout=90000;server.headersTimeout=10000;server.maxConnections=32;
  await new Promise((yes,no)=>{server.once('error',no);server.listen(port,'0.0.0.0',yes);});
  return {server,async close(){await new Promise(done=>server.close(done));await auth.close();await db.terminate();await deleteApp(app);}};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const service=await startHosted();let stopping=false;
  for(const signal of ['SIGTERM','SIGINT'])process.on(signal,async()=>{if(stopping)return;stopping=true;await service.close();});
}
