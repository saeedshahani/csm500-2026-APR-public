// Recompute the completed Loop M disposition from immutable collections.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {gunzipSync} from 'node:zlib';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
import {verify,compare,sha} from './reproducibility.mjs';
import {sections} from './wasm-sections.mjs';

const base='benchmarks/results/';
const primary=['reproduction-m-macos-02','reproduction-m-linux-one','reproduction-m-linux-two'];
const read=p=>JSON.parse(fs.readFileSync(p));
export function audit(){
  const result=compare(primary.map(d=>base+d));
  const frozen=compare([base+primary[0],base+'reproduction-m-frozen-01']);
  const historical=verify(base+'reproduction-m-historical-01');
  assert.equal(read(base+'reproduction-m-macos-01/manifest.json').complete,false);
  assert.throws(()=>verify(base+'reproduction-m-macos-01'));
  const manifests=primary.map(d=>read(base+d+'/manifest.json'));
  const panel=JSON.parse(gunzipSync(fs.readFileSync(base+primary[0]+'/remap-a-panel.json.gz')));
  const wasm=[];
  for(const name of ['reproduction-m-wasm-macos','reproduction-m-wasm-linux']){
    const dir=base+name,r=read(dir+'/report.json');assert.equal(r.complete,true);assert.equal(r.failure,null);
    assert.equal(r.source_digest,result.environments[0].source_digest);assert.equal(r.results.length,2);
    for(const row of r.results){
      for(const [file,key] of [['raw.wasm','raw_sha256'],['bound.wasm','bound_sha256'],['binding.js','binding_sha256']])assert.equal(sha(gunzipSync(fs.readFileSync(`${dir}/${row.variant}-${file}.gz`))),row[key]);
      for(const kind of ['raw','bound'])assert.deepEqual(sections(gunzipSync(fs.readFileSync(`${dir}/${row.variant}-${kind}.wasm.gz`))),row[`${kind}_sections`]);
      assert.deepEqual(row.semantic,panel.runs.map(r=>({id:r.case.id,trace_sha256:sha(JSON.stringify(r.trace))})));
    }
    const target=manifests.find(m=>m.environment.platform===r.environment.platform);assert.ok(target);
    assert.equal(r.results[0].bound_sha256,target.wasm[0].binary_sha256,'Diagnostic must reconstruct the original mismatch');
    wasm.push({directory:name,...r});
  }
  const pairs=['raw_sections','bound_sections'].map(key=>{
    const [a,b]=wasm.map(r=>r.results[0][key]);assert.deepEqual(a.map(s=>[s.id,s.name]),b.map(s=>[s.id,s.name]));
    return {kind:key,differing:a.filter((s,i)=>s.sha256!==b[i].sha256).map(s=>({id:s.id,name:s.name})),equal:a.filter((s,i)=>s.sha256===b[i].sha256).map(s=>({id:s.id,name:s.name}))};
  });
  return {schema:'loop-m-audit-v1',primary:result,counts:{environments:3,platform_architectures:2,native_builds:12,native_case_executions:12*panel.runs.length,events_per_panel:panel.runs.reduce((n,r)=>n+r.trace.events.length,0),wasm_builds:6,wasm_case_executions:84,calendar_tasks:panel.proposals.calendar.length,context_tasks:panel.proposals.context.length,measurements:manifests.reduce((n,m)=>n+m.measurements.filter(r=>!r.warmup).length,0),warmups:manifests.reduce((n,m)=>n+m.measurements.filter(r=>r.warmup).length,0)},within_host_tolerances_pass:manifests.every(m=>m.performance.every(p=>p.time.accepted&&p.rss.accepted)),linux_bytes_equal:manifests[1].builds[2].binary.sha256===manifests[2].builds[2].binary.sha256,historical,frozen_source_reconstruction:{semantic_equal:frozen.semantic_equal,native_bytes_equal:frozen.native_bytes_equal,wasm_bytes_equal:frozen.wasm_bytes_equal},wasm_diagnostic:{sections:pairs,sysroot_remapping_changes_bytes:wasm.map(r=>({platform:r.environment.platform,changed:r.results[0].bound_sha256!==r.results[1].bound_sha256}))}};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href){
  const result=audit(),[mode,file]=process.argv.slice(2);
  if(mode==='--write'){assert.ok(file);fs.writeFileSync(file,JSON.stringify(result,null,2)+'\n',{flag:'wx'});}
  else if(mode==='--check'){assert.deepEqual(read(file),result);console.log('Verified Loop M report, exact cross-environment panels and retained mismatches.');}
  else {assert.equal(mode,undefined);console.log(JSON.stringify(result,null,2));}
}
