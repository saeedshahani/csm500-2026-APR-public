import test from 'node:test';
import assert from 'node:assert/strict';
import {PlatformClient,readBounded,defaultJobSettings,type VersionRecord} from '../src/platform-client.ts';
const token='local_test_token_012345678901234567890';
const identity='00000000-0000-0000-0000-000000000001';

test('client refuses external, credential-bearing and path-bearing API addresses',()=>{
  for(const url of ['https://example.com','http://localhost:8787','http://127.0.0.1:8787/other','http://name:pass@127.0.0.1:8787','http://127.0.0.1:8787/?token=x'])assert.throws(()=>new PlatformClient(url,token));
  assert.throws(()=>new PlatformClient('http://127.0.0.1:8787','short'));
});
test('pause and resume are explicit authenticated requests without changed settings',async t=>{
  const calls:{url:string;body:string;authorization:string}[]=[];
  t.mock.method(globalThis,'fetch',async(url:unknown,options:RequestInit)=>{calls.push({url:String(url),body:options.body as string,authorization:(options.headers as Record<string,string>).Authorization});return Response.json({id:identity,status:'paused'});});
  const client=new PlatformClient('http://127.0.0.1:8787',token);
  await client.pause(identity);await client.resume(identity);
  assert.deepEqual(calls.map(c=>c.url),[`http://127.0.0.1:8787/v1/jobs/${identity}/pause`,`http://127.0.0.1:8787/v1/jobs/${identity}/resume`]);
  assert.ok(calls.every(c=>c.body==='{}'&&c.authorization===`Bearer ${token}`));client.close();
});
test('submission reuses its key after an uncertain response but deliberate new jobs receive new keys',async t=>{
  const requests:RequestInit[]=[];let calls=0;
  t.mock.method(globalThis,'fetch',async(_url:unknown,options:RequestInit)=>{requests.push(options);if(calls++===0)throw new TypeError('network');return Response.json({id:identity});});
  const client=new PlatformClient('http://127.0.0.1:8787',token);
  await assert.rejects(client.submit(identity,identity,defaultJobSettings),/Cannot reach/);
  await client.submit(identity,identity,defaultJobSettings);await client.submit(identity,identity,defaultJobSettings);
  const bodies=requests.map(r=>JSON.parse(r.body as string));assert.equal(bodies[0].idempotency_key,bodies[1].idempotency_key);assert.notEqual(bodies[1].idempotency_key,bodies[2].idempotency_key);
  assert.equal(requests[0].redirect,'error');assert.equal(requests[0].credentials,'omit');client.close();await assert.rejects(client.health(),/disconnected/);
});
test('artefact content and advertised digest both have to match the saved identity',async t=>{
  const bytes=new TextEncoder().encode('program {}');
  const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
  const version={id:identity,project_id:identity,source_sha256:digest} as VersionRecord;
  const client=new PlatformClient('http://127.0.0.1:8787',token);let body='program {}',header=digest;
  t.mock.method(globalThis,'fetch',async()=>new Response(body,{headers:{'X-Content-SHA256':header}}));
  assert.equal(await client.source(version),'program {}');body='corrupt';await assert.rejects(client.source(version),/checksum/);
  body='program {}';header='bad';await assert.rejects(client.source(version),/checksum/);client.close();
});
test('byte limits cover advertised and chunked responses',async()=>{
  await assert.rejects(readBounded(new Response('abcd',{headers:{'Content-Length':'4'}}),3),/byte limit/);
  await assert.rejects(readBounded(new Response('abcd'),3),/byte limit/);
  assert.equal(new TextDecoder().decode(await readBounded(new Response('abc'),3)),'abc');
});
test('disconnect aborts requests and rejects late responses',async t=>{
  let resolve!:(value:Response)=>void;let signal:AbortSignal|undefined;
  t.mock.method(globalThis,'fetch',async(_url:unknown,options:RequestInit)=>{signal=options.signal!;return new Promise<Response>(yes=>{resolve=yes;});});
  const client=new PlatformClient('http://127.0.0.1:8787',token),request=client.health();client.close();assert.equal(signal?.aborted,true);
  resolve(Response.json({api:'v1'}));await assert.rejects(request,/disconnected/);
});
test('structured API errors are preserved without automatic retries',async t=>{
  let calls=0;t.mock.method(globalThis,'fetch',async()=>{calls++;return Response.json({error:{code:'busy',message:'Native slot busy'}},{status:429});});
  const client=new PlatformClient('http://127.0.0.1:8787',token);await assert.rejects(client.validate('program {}'),/busy: Native slot busy/);assert.equal(calls,1);client.close();
});

test('refreshable identities are obtained per request and never sent after disconnect',async t=>{
  let count=0;const headers:string[]=[];
  t.mock.method(globalThis,'fetch',async(_url:unknown,options:RequestInit)=>{headers.push((options.headers as Record<string,string>).Authorization);return Response.json({api:'v1'});});
  const client=new PlatformClient('http://127.0.0.1:8787',async()=>`token.${++count}.`);
  await client.health();await client.projects();assert.deepEqual(headers,['Bearer token.1.','Bearer token.2.']);client.close();
  let resolve!:(token:string)=>void;
  const pendingClient=new PlatformClient('http://127.0.0.1:8787',()=>new Promise(yes=>{resolve=yes;}));
  const pending=pendingClient.health();pendingClient.close();resolve('token.3.');
  await assert.rejects(pending,/disconnected/);assert.equal(headers.length,2);
});
