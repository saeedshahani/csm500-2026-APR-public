// Operator-only deployment tooling. Every resource is in the pinned project.
// Uses an existing Firebase CLI login; never writes or logs credentials.
import {createRequire} from 'node:module';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {target} from './deployment-target.mjs';
const args=process.argv.slice(2),action=args[0];
if(args[1]!=='--project'||args.length!==3)throw Error('Usage: node platform/cloud-admin.mjs prepare|build|deploy|inspect|runtime|logs|password-policy|reconcile|disable|enable|backup-reader|backup-reader-remove|backup-download|rollback-check --project reflective-graph-f1cd4');
const config=target(args[2]),P=config.project_id,N=config.project_number,R='europe-west2';
if(!config.deployment_approved)throw Error('Deployment approval is required');
const require=createRequire(import.meta.url),auth=require('firebase-tools/lib/auth');
const account=auth.getProjectDefaultAccount(process.cwd());if(!account)throw Error('Firebase CLI login required');
auth.setActiveAccount({project:P},account);
const token=()=>require('firebase-tools/lib/apiv2').getAccessToken();
async function request(url,method='GET',body,allowed=[]) {
  const response=await fetch(url,{method,headers:{Authorization:`Bearer ${await token()}`,'x-goog-user-project':P,
    ...(body?{'Content-Type':Buffer.isBuffer(body)?'application/octet-stream':'application/json'}:{})},
    ...(body?{body:Buffer.isBuffer(body)?body:JSON.stringify(body)}:{}),signal:AbortSignal.timeout(60000)});
  const text=await response.text();let value;try{value=JSON.parse(text);}catch{value={};}
  if(!response.ok&&!allowed.includes(response.status))throw Error(`${method} ${new URL(url).pathname}: HTTP ${response.status} ${value.error?.status??''} ${value.error?.message??''}`);
  return {status:response.status,value};
}
async function api(url,method='GET',body,allowed=[]){return (await request(url,method,body,allowed)).value;}
const service=name=>`https://${name}.googleapis.com`;
async function operation(origin,value){
  if(!value.name?.includes('operations/'))return value;
  for(let i=0;i<180;i++){
    if(value.done){if(value.error)throw Error(`Operation failed: ${value.error.code} ${value.error.message}`);return value.response;}
    await new Promise(done=>setTimeout(done,2000));value=await api(`${origin}/${value.name.startsWith('operations/')?'v1/':''}${value.name}`);
  }
  throw Error('Operation still pending; inspect before retrying');
}
const runtime=`graph-runtime@${P}.iam.gserviceaccount.com`,dispatch=`graph-dispatch@${P}.iam.gserviceaccount.com`,builder=`graph-build@${P}.iam.gserviceaccount.com`;
const bucket=`${P}.firebasestorage.app`,buildBucket=`${P}-builds`,repo=`projects/${P}/locations/${R}/repositories/graph`;
const base=`https://graph-api-${N}.${R}.run.app`,image=`${R}-docker.pkg.dev/${P}/graph/api`;
async function projectRoles(bindings){
  const url=`${service('cloudresourcemanager')}/v1/projects/${P}`;
  const policy=await api(url+':getIamPolicy','POST',{});policy.bindings??=[];
  for(const [role,member]of bindings){let entry=policy.bindings.find(b=>b.role===role&&!b.condition);if(!entry){entry={role,members:[]};policy.bindings.push(entry);}if(!entry.members.includes(member))entry.members.push(member);}
  await api(url+':setIamPolicy','POST',{policy});
}
async function bucketRole(name,member,role){
  const url=`${service('storage')}/storage/v1/b/${name}/iam`,policy=await api(url);policy.bindings??=[];
  let binding=policy.bindings.find(x=>x.role===role&&!x.condition);if(!binding){binding={role,members:[]};policy.bindings.push(binding);}if(!binding.members.includes(member))binding.members.push(member);
  await api(url,'PUT',policy);
}
if(action==='prepare'){
  const services=['firestore.googleapis.com','firebasestorage.googleapis.com','run.googleapis.com','cloudtasks.googleapis.com','cloudscheduler.googleapis.com','cloudbuild.googleapis.com','artifactregistry.googleapis.com','iam.googleapis.com','iamcredentials.googleapis.com'];
  await operation(service('serviceusage'),await api(`${service('serviceusage')}/v1/projects/${N}/services:batchEnable`,'POST',{serviceIds:services}));
  console.log('Required project APIs enabled');
  for(const id of ['graph-runtime','graph-dispatch','graph-build'])await api(`${service('iam')}/v1/projects/${P}/serviceAccounts`,'POST',{accountId:id,serviceAccount:{displayName:`Reflective Graph ${id}`}},[409]);
  const db=`${service('firestore')}/v1/projects/${P}/databases/(default)`;
  if((await request(db,'GET',undefined,[404])).status===404){
    const created=await api(`${service('firestore')}/v1/projects/${P}/databases?databaseId=(default)`,'POST',{locationId:R,type:'FIRESTORE_NATIVE',databaseEdition:'STANDARD',deleteProtectionState:'DELETE_PROTECTION_ENABLED'});
    await operation(service('firestore')+'/v1',created);
  }
  const actual=await api(db);if(actual.locationId!==R||actual.type!=='FIRESTORE_NATIVE')throw Error('Existing database location/type differs; not overwritten');
  const storage=`${service('storage')}/storage/v1/b`;
  for(const name of [bucket,buildBucket]){
    const found=await request(`${storage}/${name}`,'GET',undefined,[404]);
    if(found.status===404){
      if(name===bucket)await api(`${service('firebasestorage')}/v1alpha/projects/${P}/defaultBucket`,'POST',{location:R});
      else await api(`${storage}?project=${P}`,'POST',{name,location:R,storageClass:'STANDARD'});
      await api(`${storage}/${name}`,'PATCH',{iamConfiguration:{uniformBucketLevelAccess:{enabled:true},publicAccessPrevention:'enforced'},
        lifecycle:{rule:[{action:{type:'Delete'},condition:{age:7,...(name===bucket?{matchesPrefix:['backups/']}:{})}}]}});
    }else if(String(found.value.projectNumber)!==N||found.value.location!=='EUROPE-WEST2')throw Error('Existing bucket target/location differs');
  }
  await api(`${service('firebasestorage')}/v1beta/projects/${P}/buckets/${bucket}:addFirebase`,'POST',{},[409]);
  await operation(service('artifactregistry')+'/v1',await api(`${service('artifactregistry')}/v1/projects/${P}/locations/${R}/repositories?repositoryId=graph`,'POST',{format:'DOCKER',description:'Reflective Graph assessed native service'},[409]));
  await projectRoles([['roles/datastore.user',`serviceAccount:${runtime}`],['roles/firebaseauth.viewer',`serviceAccount:${runtime}`],['roles/cloudtasks.enqueuer',`serviceAccount:${runtime}`],['roles/logging.logWriter',`serviceAccount:${builder}`]]);
  await bucketRole(bucket,`serviceAccount:${runtime}`,'roles/storage.objectUser');
  await bucketRole(buildBucket,`serviceAccount:${builder}`,'roles/storage.objectViewer');
  const repourl=`${service('artifactregistry')}/v1/${repo}`;
  const policy=await api(repourl+':getIamPolicy');policy.bindings??=[];
  const member=`serviceAccount:${builder}`;let binding=policy.bindings.find(x=>x.role==='roles/artifactregistry.writer');
  if(!binding){binding={role:'roles/artifactregistry.writer',members:[]};policy.bindings.push(binding);}if(!binding.members.includes(member))binding.members.push(member);
  await api(repourl+':setIamPolicy','POST',{policy});
  const sa=`${service('iam')}/v1/projects/${P}/serviceAccounts/${dispatch}`;
  const sapolicy=await api(sa+':getIamPolicy','POST',{});sapolicy.bindings??=[];let sab=sapolicy.bindings.find(x=>x.role==='roles/iam.serviceAccountUser');
  if(!sab){sab={role:'roles/iam.serviceAccountUser',members:[]};sapolicy.bindings.push(sab);}if(!sab.members.includes(`serviceAccount:${runtime}`))sab.members.push(`serviceAccount:${runtime}`);
  await api(sa+':setIamPolicy','POST',{policy:sapolicy});
  const parent=`projects/${P}/locations/${R}`;
  await api(`${service('cloudtasks')}/v2/${parent}/queues`,'POST',{name:`${parent}/queues/graph-jobs`,rateLimits:{maxDispatchesPerSecond:1,maxConcurrentDispatches:1},retryConfig:{maxAttempts:5,minBackoff:'5s',maxBackoff:'60s',maxDoublings:3}},[409]);
  console.log(JSON.stringify({prepared:true,project:P,region:R,bucket,service_url:base}));
}else if(action==='build'){
  const bytes=readFileSync('output/hosted-source.tar.gz'),sha=createHash('sha256').update(bytes).digest('hex'),object=`source/${sha}.tar.gz`;
  await request(`${service('storage')}/upload/storage/v1/b/${buildBucket}/o?uploadType=media&ifGenerationMatch=0&name=${encodeURIComponent(object)}`,'POST',bytes,[412]);
  const build=await api(`${service('cloudbuild')}/v1/projects/${P}/locations/${R}/builds`,'POST',{
    source:{storageSource:{bucket:buildBucket,object}},serviceAccount:`projects/${P}/serviceAccounts/${builder}`,
    steps:[{name:'gcr.io/cloud-builders/docker',args:['build','-f','platform/Dockerfile','-t',`${image}:${sha.slice(0,16)}`,'.']},
      {name:'gcr.io/cloud-builders/docker',args:['run','--rm','--network=none','--memory=512m','--cpus=1','--pids-limit=128','--read-only','--tmpfs=/tmp:rw,noexec,nosuid,size=128m',`${image}:${sha.slice(0,16)}`,'node','--test','platform/tests/platform.test.mjs']}],
    images:[`${image}:${sha.slice(0,16)}`],timeout:'1200s',options:{logging:'CLOUD_LOGGING_ONLY',machineType:'E2_MEDIUM'}});
  console.log(JSON.stringify({source_sha256:sha,operation:build.name,build_id:build.metadata?.build?.id,image:`${image}:${sha.slice(0,16)}`}));
}else if(action==='deploy'){
  const sha=createHash('sha256').update(readFileSync('output/hosted-source.tar.gz')).digest('hex');
  const builds=await api(`${service('cloudbuild')}/v1/projects/${P}/locations/${R}/builds?pageSize=10`);
  const selected=builds.builds?.find(x=>x.status==='SUCCESS'&&x.source?.storageSource?.object===`source/${sha}.tar.gz`&&x.results?.images?.[0]?.name===`${image}:${sha.slice(0,16)}`);
  if(!selected)throw Error('No successful tested build for the exact current source archive');
  const pinned=`${image}@${selected.results.images[0].digest}`;
  const name=`projects/${P}/locations/${R}/services/graph-api`,url=`${service('run')}/v2/${name}`;
  const spec={name,ingress:'INGRESS_TRAFFIC_ALL',scaling:{minInstanceCount:0,maxInstanceCount:2},template:{serviceAccount:runtime,timeout:'120s',maxInstanceRequestConcurrency:1,
    scaling:{minInstanceCount:0,maxInstanceCount:2},containers:[{image:pinned,command:['node','platform/hosted.mjs'],ports:[{containerPort:8080}],resources:{limits:{cpu:'1',memory:'512Mi'},cpuIdle:true,startupCpuBoost:false},env:[{name:'GRAPH_FIREBASE_PROJECT',value:P},{name:'GRAPH_SERVICE_URL',value:base}]}]}};
  const exists=await request(url,'GET',undefined,[404]);
  const op=exists.status===404?await api(`${service('run')}/v2/projects/${P}/locations/${R}/services?serviceId=graph-api`,'POST',{...spec,name:undefined}):await api(url,'PATCH',spec);
  await operation(service('run')+'/v2',op);
  const policy=await api(`${service('run')}/v2/${name}:getIamPolicy`);policy.bindings??=[];
  if(!policy.bindings.some(x=>x.role==='roles/run.invoker'&&x.members.includes('allUsers')))policy.bindings.push({role:'roles/run.invoker',members:['allUsers']});
  await api(`${service('run')}/v2/${name}:setIamPolicy`,'POST',{policy});
  const job={name:`projects/${P}/locations/${R}/jobs/graph-reconcile`,schedule:'*/15 * * * *',timeZone:'Etc/UTC',attemptDeadline:'120s',
    httpTarget:{uri:base+'/_reconcile',httpMethod:'POST',body:Buffer.from('{}').toString('base64'),headers:{'Content-Type':'application/json'},oidcToken:{serviceAccountEmail:dispatch,audience:base}}};
  await api(`${service('cloudscheduler')}/v1/projects/${P}/locations/${R}/jobs`,'POST',job,[409]);
  console.log(JSON.stringify({deployed:true,image:pinned,build:selected.id,url:base}));
}else if(action==='password-policy'){
  const policy=await api(`${service('identitytoolkit')}/admin/v2/projects/${P}/config?updateMask=passwordPolicyConfig&fields=passwordPolicyConfig`,'PATCH',{
    passwordPolicyConfig:{passwordPolicyEnforcementState:'ENFORCE',forceUpgradeOnSignin:false,passwordPolicyVersions:[{customStrengthOptions:{minPasswordLength:12}}]}});
  console.log(JSON.stringify(policy));
}else if(action==='reconcile'){
  await api(`${service('cloudscheduler')}/v1/projects/${P}/locations/${R}/jobs/graph-reconcile:run`,'POST',{});
  console.log('Reconciliation requested');
}else if(action==='backup-download'){
  const list=await api(`${service('storage')}/storage/v1/b/${bucket}/o?prefix=backups%2Fpilot%2F`);
  const selected=list.items?.sort((a,b)=>b.name.localeCompare(a.name))[0];if(!selected)throw Error('No pilot backup exists');
  const response=await fetch(`${service('storage')}/storage/v1/b/${bucket}/o/${encodeURIComponent(selected.name)}?alt=media`,{headers:{Authorization:`Bearer ${await token()}`,'x-goog-user-project':P},signal:AbortSignal.timeout(60000)});
  if(!response.ok)throw Error(`Backup download failed: HTTP ${response.status}; inspect the operator's backup-reader permission`);const bytes=Buffer.from(await response.arrayBuffer());
  const sha=createHash('sha256').update(bytes).digest('hex');if(sha!==selected.metadata?.sha256)throw Error('Archive checksum mismatch');
  mkdirSync('output/hosted',{recursive:true});const path=`output/hosted/backup-${sha}.json.gz`;writeFileSync(path,bytes,{flag:'wx',mode:0o600});
  console.log(JSON.stringify({path,sha256:sha,source:selected.name}));
}else if(['backup-reader','backup-reader-remove'].includes(action)){
  const email=account.user?.email;if(typeof email!=='string'||!email.includes('@'))throw Error('Logged-in operator identity unavailable');
  const url=`${service('storage')}/storage/v1/b/${bucket}/iam`;
  const policy=await api(url+'?optionsRequestedPolicyVersion=3');policy.version=3;
  policy.bindings=(policy.bindings??[]).filter(b=>!(b.condition?.title==='graph-pilot-backup-restore'&&b.members?.length===1&&b.members[0]===`user:${email}`));
  if(action==='backup-reader')policy.bindings.push({role:'roles/storage.objectViewer',members:[`user:${email}`],condition:{title:'graph-pilot-backup-restore',description:'Explicitly authorised isolated restore; automatically expires after one hour',expression:`resource.name.startsWith("projects/_/buckets/${bucket}/objects/backups/pilot/") && request.time < timestamp("${new Date(Date.now()+3600000).toISOString()}")`}});
  await api(url,'PUT',policy);console.log(JSON.stringify({project:P,temporary_backup_reader:action==='backup-reader'}));
}else if(action==='runtime'){
  const run=await api(`${service('run')}/v2/projects/${P}/locations/${R}/services/graph-api`);
  const queue=await api(`${service('cloudtasks')}/v2/projects/${P}/locations/${R}/queues/graph-jobs`);
  const scheduler=await api(`${service('cloudscheduler')}/v1/projects/${P}/locations/${R}/jobs/graph-reconcile`);
  const policy=await api(`${service('cloudresourcemanager')}/v1/projects/${P}:getIamPolicy`,'POST',{});
  const storage=await api(`${service('storage')}/storage/v1/b/${bucket}`);
  console.log(JSON.stringify({project:P,run:{uri:run.uri,revision:run.latestReadyRevision,scaling:run.scaling,template:run.template},queue:{state:queue.state,rateLimits:queue.rateLimits,retryConfig:queue.retryConfig},scheduler:{state:scheduler.state,schedule:scheduler.schedule,lastAttemptTime:scheduler.lastAttemptTime,status:scheduler.status},storage:{location:storage.location,lifecycle:storage.lifecycle,softDeletePolicy:storage.softDeletePolicy,iamConfiguration:storage.iamConfiguration},runtime_roles:(policy.bindings??[]).filter(b=>b.members?.includes(`serviceAccount:${runtime}`)).map(b=>b.role)},null,2));
}else if(action==='rollback-check'){
  const url=`https://firebasehosting.googleapis.com/v1beta1/sites/${P}/releases`;
  const releases=(await api(url+'?pageSize=10')).releases??[];
  const current=releases[0]?.version,previous=releases.find(x=>x.version?.name!==current?.name)?.version;
  if(!current?.name||!previous?.name)throw Error('Two existing hosted candidates required');
  // This test permits only a frontend rollback with the identical API pin.
  const [a,b]=await Promise.all([api(`https://firebasehosting.googleapis.com/v1beta1/${current.name}`),api(`https://firebasehosting.googleapis.com/v1beta1/${previous.name}`)]);
  if(JSON.stringify(a.config?.rewrites)!==JSON.stringify(b.config?.rewrites))throw Error('API pins differ; use a separately reviewed full-service rollback');
  try{
    await api(`${url}?versionName=${encodeURIComponent(previous.name)}`,'POST',{message:'Bounded first-loop rollback acceptance'});
    const response=await fetch(`https://${P}.web.app/`,{cache:'no-store'});if(!response.ok)throw Error('Rolled-back site unavailable');await response.body?.cancel();
  }finally{await api(`${url}?versionName=${encodeURIComponent(current.name)}`,'POST',{message:'Restore accepted candidate after rollback check'});}
  console.log(JSON.stringify({rollback_verified:true,restored_version:current.name,previous_version:previous.name}));
}else if(['enable','disable'].includes(action)){
  await api(`${service('firestore')}/v1/projects/${P}/databases/(default)/documents/reflectiveControl/pilot?updateMask.fieldPaths=disabled`,'PATCH',{fields:{disabled:{booleanValue:action==='disable'}}});
  console.log(JSON.stringify({project:P,execution_disabled:action==='disable'}));
}else if(action==='logs'){
  const logs=await api(`${service('logging')}/v2/entries:list`,'POST',{resourceNames:[`projects/${P}`],filter:'resource.type="cloud_run_revision" AND resource.labels.service_name="graph-api" AND severity>=ERROR',orderBy:'timestamp desc',pageSize:15});
  console.log(JSON.stringify((logs.entries??[]).map(x=>({time:x.timestamp,message:x.textPayload??x.jsonPayload,status:x.httpRequest?.status}))));
}else if(action==='inspect'){
  const builds=await api(`${service('cloudbuild')}/v1/projects/${P}/locations/${R}/builds?pageSize=5`);
  console.log(JSON.stringify({builds:builds.builds?.map(x=>({id:x.id,status:x.status,images:x.results?.images,logUrl:x.logUrl,failure:x.failureInfo}))}));
}else throw Error('Unknown action');
