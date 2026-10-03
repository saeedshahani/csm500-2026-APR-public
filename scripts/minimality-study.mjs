// Immutable representation evidence. collect|verify DIR [--live]
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,existsSync,mkdirSync} from 'node:fs';
import {execFileSync,spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {gzipSync,gunzipSync} from 'node:zlib';
import {resolve} from 'node:path';
import os from 'node:os';
import {encode,decode,restore,rename,counts,edgeEncode,edgeDecode} from './minimality-codecs.mjs';
import {enumerate,verifyWitness} from './minimality-gadgets.mjs';
const [mode,directory]=process.argv.slice(2);
assert.ok(['collect','verify'].includes(mode)&&directory,'collect|verify DIR [--live]');
const binary=resolve('target/release/examples/minimality');
const sha=x=>createHash('sha256').update(x).digest('hex');
function native(mode,input='') {
  const r=spawnSync(binary,[mode],{input,encoding:'utf8',timeout:120000,maxBuffer:128*1024*1024});
  assert.equal(r.status,0,r.error?.message??r.stderr);return r.stdout;
}
function translated(graph,variant,offset=0n) {
  const {wire,mapping}=encode(graph,variant), actual=counts(graph,variant);
  assert.equal(Object.keys(wire.nodes).length,actual.nodes);
  assert.equal(Object.values(wire.nodes).reduce((s,n)=>s+n.arcs.length,0),actual.arcs);
  const {wire:renamed,map}=rename(wire);
  const mapping2=Object.fromEntries(Object.entries(mapping).map(([a,b])=>[a,map[b]]));
  assert.deepEqual(restore(decode(renamed,variant),mapping2,graph.next_identity),graph);
  const decoded=decode(wire,variant);
  assert.deepEqual(restore(decoded,mapping,graph.next_identity),graph);
  return restore(decoded,mapping,graph.next_identity,offset);
}
function queryFor(cell,variant) {
  const offset=1000000n, id=x=>String(BigInt(x)+offset);
  return {graph:translated(cell.query.graph,variant,offset),source:id(cell.query.source),binding:cell.query.binding.map(p=>p.map(id))};
}
function unshift(answer) {
  const offset=1000000n,id=x=>String(BigInt(x)-offset),a=structuredClone(answer);
  a.graph={next_identity:id(a.graph.next_identity),atoms:Object.fromEntries(Object.entries(a.graph.atoms).map(([key,atom])=>[id(key),{kind:atom.kind,fields:atom.fields.map(id)}]))};
  if(a.status==='Admitted') {
    a.source=id(a.source);a.revision=id(a.revision);
    a.footprint=Object.fromEntries(Object.entries(a.footprint).map(([k,v])=>[k,v.map(id)]));
  }
  return a;
}
function check(corpus,answers) {
  assert.equal(corpus.schema,'minimality-native-v1');
  const variants=answers.length===corpus.cases.length*3?['M','T','P']:['T','P'];
  const rows=[];let index=0;
  for(const s of corpus.snapshots) {
    for(const variant of variants)translated(s.graph,variant);
    const e=edgeEncode(s.graph);assert.deepEqual(edgeDecode(e),s.graph.atoms);
    const values=Object.values(s.graph.atoms),n=values.length,f=values.reduce((sum,x)=>sum+x.fields.length,0),records=values.filter(x=>x.kind.Record).length;
    const t=counts(s.graph,'T'),p=counts(s.graph,'P');
    assert.equal(t.nodes-p.nodes,2+p.f0);assert.equal(t.arcs-p.arcs,2+2*p.f0+p.f1);
    const pattern=s.pattern??[], pAtoms=pattern.map(id=>s.graph.atoms[id]);
    const owned=variant=>pattern.length+pAtoms.filter(a=>a.kind.PatternVariable).length+pAtoms.reduce((sum,a)=>sum+(variant==='T'?a.fields.length:Number(a.fields.length>1)),0);
    rows.push({name:s.name,prefix:s.prefix,n,f,edge_vertices:n-records,edge_entities:records,T:t,P:p,
      ...(s.pattern?{pattern_atoms:pattern.length,pattern_owned_T:owned('T'),pattern_owned_P:owned('P')}: {}),
      ...(variants.includes('M')?{M:counts(s.graph,'M'),pattern_owned_M:pattern.length+pAtoms.reduce((sum,a)=>sum+a.fields.length,0)}:{})});
  }
  const statuses={};
  for(const cell of corpus.cases) {
    statuses[cell.answer.status]=(statuses[cell.answer.status]??0)+1;
    for(const variant of variants) {
      assert.deepEqual(unshift(answers[index++]),cell.answer);
      translated(cell.answer.graph,variant);
    }
  }
  assert.equal(index,answers.length);
  return {snapshots:rows.length,native_cases:corpus.cases.length,translated_queries:index,statuses,rows};
}
function gadgets() {
  const rows=[1,2,3,4].map(i=>enumerate(i));
  for(const row of rows){verifyWitness(row);if(row.internal<=3){const alt=enumerate(row.internal,true);for(const k of ['total','reachable','ordered','min_edges'])assert.equal(row[k],alt[k]);}}
  return rows;
}
if(mode==='collect') {
  assert.ok(!existsSync(directory),'never overwrite a collection');mkdirSync(directory,{recursive:true});
  const paths=[...new Set(execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n'))].filter(p=>existsSync(p)&&(/^(Cargo\.(toml|lock)|rust-toolchain.toml)$/.test(p)||/^crates\/.*\.(rs|toml|lean)$/.test(p)||/^scripts\/minimality.*\.mjs$/.test(p)||p==='docs/research/minimality.md'||p==='examples/manifest.json'||/^examples\/.*\.(graph|lambda|atoms)$/.test(p))).sort();
  const sources=gzipSync(JSON.stringify({files:Object.fromEntries(paths.map(p=>[p,readFileSync(p).toString('base64')]))}));
  writeFileSync(resolve(directory,'sources.json.gz'),sources);
  const manifest={schema:'minimality-study-v1',complete:false,parent:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),created:new Date().toISOString(),source_sha256:sha(sources),binary_sha256:sha(readFileSync(binary)),environment:{node:process.version,platform:os.platform(),arch:os.arch(),cpu:os.cpus()[0].model,rust:execFileSync('rustc',['--version'],{encoding:'utf8'}).trim()},files:{},failure:null};
  try {
    const corpus=JSON.parse(native('corpus'));
    const queries=corpus.cases.flatMap(c=>['M','T','P'].map(v=>JSON.stringify(queryFor(c,v)))).join('\n')+'\n';
    const answers=native('admit',queries).trim().split('\n').map(x=>JSON.parse(x));
    const report={...check(corpus,answers),gadgets:gadgets()};
    for(const [name,value] of [['native.json.gz',{corpus,answers}],['report.json',report]]) {
      const bytes=name.endsWith('.gz')?gzipSync(JSON.stringify(value)):Buffer.from(JSON.stringify(value,null,2)+'\n');
      writeFileSync(resolve(directory,name),bytes);manifest.files[name]=sha(bytes);
    }
    assert.equal(sha(readFileSync(binary)),manifest.binary_sha256);
    manifest.complete=true;console.log(JSON.stringify({...report,rows:undefined},null,2));
  } catch(e){manifest.failure=String(e.stack);process.exitCode=1;console.error(manifest.failure);}
  writeFileSync(resolve(directory,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
} else {
  const manifest=JSON.parse(readFileSync(resolve(directory,'manifest.json')));assert.equal(manifest.complete,true);
  assert.equal(sha(readFileSync(resolve(directory,'sources.json.gz'))),manifest.source_sha256);
  for(const [file,hash] of Object.entries(manifest.files))assert.equal(sha(readFileSync(resolve(directory,file))),hash);
  const {corpus,answers}=JSON.parse(gunzipSync(readFileSync(resolve(directory,'native.json.gz'))));
  const report=JSON.parse(readFileSync(resolve(directory,'report.json')));
  assert.deepEqual({...check(corpus,answers),gadgets:gadgets()},report);
  if(process.argv.includes('--live')) {
    assert.deepEqual(JSON.parse(native('corpus')),corpus);
    const variants=answers.length===corpus.cases.length*3?['M','T','P']:['T','P'];
    const input=corpus.cases.flatMap(c=>variants.map(v=>JSON.stringify(queryFor(c,v)))).join('\n')+'\n';
    assert.deepEqual(native('admit',input).trim().split('\n').map(x=>JSON.parse(x)),answers);
  }
  console.log(`Verified ${report.snapshots} snapshots, ${report.native_cases} native cases, ${report.translated_queries} translated queries and 238480 gadgets; live bridge ${process.argv.includes('--live')?'repeated':'not repeated'}.`);
}
