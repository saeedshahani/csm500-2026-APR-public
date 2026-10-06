// Native acceptance with an independent pairwise occurrence-DAG oracle.
// Run: cargo build --release --locked -p cli
//      node scripts/selected-provenance-study.mjs NEW_RESULT.json
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync} from 'node:fs';
import {execFileSync, spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {sourceFiles} from './research-source-files.mjs';

const output=process.argv[2];
assert.ok(output, 'provide a new result path');
const binary=process.env.GRAPH_CLI ?? 'target/release/cli';
const sha=b=>createHash('sha256').update(b).digest('hex');
function call(args,input) {
  const p=spawnSync(binary,args,{input,encoding:'utf8',maxBuffer:32*1024*1024,timeout:120000});
  assert.ifError(p.error);
  assert.ok([0,3,6].includes(p.status), `${args}: ${p.status} ${p.stderr}`);
  return {status:p.status, bytes:p.stdout, value:JSON.parse(p.stdout)};
}
const files=['behaviour/stable','behaviour/oscillating','behaviour/collapse','independent/disjoint','independent/shared-read','programs/staging','programs/self-modify','programs/restoration'];
const rows=[];
let events=0,agreements=0,selectedReports=0;
const selections=[[],['--select',''],['--classes','']];
for(const name of files) for(const seed of [7,42,6917]) {
  const file=`examples/${name}.graph`;
  const source=readFileSync(file);
  const exported=call(['export','-','--max-steps','6','--strategy','seeded','--seed',String(seed)],source);
  const trace=exported.value;
  assert.notEqual(trace.report?.outcome,'ResourceLimit',`${name}: unexpected censored matcher`);
  const ancestors=trace.events.map((event,i)=>{
    const pending=[i],seen=new Set();
    while(pending.length) {
      const j=pending.pop();
      for(let k=0;k<j;k++) if(trace.events[k].footprint.produced.some(a=>trace.events[j].footprint.read.includes(a))&&!seen.has(k)) {seen.add(k);pending.push(k);}
    }
    return [...seen].sort((a,b)=>a-b);
  });
  const even=trace.events.filter((_,i)=>i%2===0).map(e=>e.id);
  const classSpec=trace.events.map((e,i)=>`${e.id}=phase-${i%2}`).join(',');
  const choices=[...selections,['--select',even.join(',')],['--classes',classSpec]];
  for(const flags of choices) {
    const result=call(['trace-provenance','-',...flags],exported.bytes);
    assert.equal(result.status,0);
    const report=result.value.analysis;
    assert.equal(result.value.source_trace_sha256,sha(exported.bytes));
    assert.equal(report.global_necessity,'unknown');
    assert.equal(report.alternative_execution_coverage,'not_explored');
    assert.equal(report.observations.length,trace.events.length);
    const tags=trace.events.map((e,i)=> {
      if(flags[0]==='--select') return flags[1].split(',').includes(e.id)?[{kind:'occurrence',value:e.id}]:[];
      if(flags[0]==='--classes') return flags[1]? [{kind:'class',value:`phase-${i%2}`}]:[];
      return [{kind:'occurrence',value:e.id}];
    });
    const encoded=labels=>[...new Set(labels.map(l=>JSON.stringify(l)))].sort();
    const decoded=ids=>encoded(ids.map(id=>report.labels[Number(id)]));
    for(let i=0;i<trace.events.length;i++) {
      const observation=report.observations[i];
      assert.deepEqual(observation.redex,trace.events[i].redex);
      assert.deepEqual(decoded(observation.selected_birth_read_ancestry),encoded(ancestors[i].flatMap(k=>tags[k])));
      assert.deepEqual(decoded(observation.selected_pre_action_history),encoded(tags.slice(0,i).flat()));
      assert.deepEqual(decoded(observation.selected_labels),encoded(tags[i]));
      const expectedOrigins=[];
      for(const atom of trace.events[i].footprint.read) {
        const creator=trace.events.findIndex(e=>e.footprint.produced.includes(atom));
        if(creator>=0) {
          assert.ok(creator<i);
          expectedOrigins.push([atom,String(creator),encoded(tags[creator])]);
        }
      }
      assert.deepEqual(observation.input_origins.map(o=>[o.atom,o.created_by,decoded(o.selected_direct_creator)]),expectedOrigins);
      agreements++;
    }
    selectedReports++;
  }
  events+=trace.events.length;
  rows.push({name,seed,source_sha256:sha(source),events:trace.events.length,status:exported.status,outcome:trace.report?.outcome,trace_sha256:sha(exported.bytes),initial_sha256:trace.initial_sha256,implementation:trace.implementation});
}
const pins=Object.fromEntries(sourceFiles(['crates']).filter(f=>/\.(rs|toml)$/.test(f)).concat(['Cargo.toml','Cargo.lock','rust-toolchain.toml','scripts/selected-provenance-study.mjs','crates/engine/proofs/SelectedProvenance.lean']).sort().map(f=>[f,sha(readFileSync(f))]));
const report={schema:'selected-provenance-study-v1',scope:'Current native traces and selected observations; no alternative-execution completeness, compression benchmark or independent-team replication.',counts:{histories:rows.length,events,selection_reports:selectedReports,event_projection_agreements:agreements},execution_limits:'engine defaults; max_steps=6',analysis_limits:{max_work:2000000,max_units:2000000},binary_sha256:sha(readFileSync(binary)),rustc:execFileSync('rustc',['--version'],{encoding:'utf8'}).trim(),source_sha256:pins,rows};
writeFileSync(output,JSON.stringify(report,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(report.counts));
