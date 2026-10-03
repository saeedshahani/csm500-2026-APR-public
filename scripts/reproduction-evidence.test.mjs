// Reject incomplete or damaged evidence without building or executing its payload.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdtempSync,cpSync,rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {verify,compare} from './reproducibility.mjs';
import {sections} from './wasm-sections.mjs';
import {audit} from './verify-reproduction.mjs';

const collection='benchmarks/results/reproduction-m-macos-02';
function damaged(change) {
  const directory=mkdtempSync(join(tmpdir(),'reflective-evidence-test-'));
  try {
    cpSync(collection,directory,{recursive:true});
    const file=join(directory,'manifest.json'),manifest=JSON.parse(readFileSync(file));
    change(manifest,directory);writeFileSync(file,JSON.stringify(manifest));
    assert.throws(()=>verify(directory));
  } finally {rmSync(directory,{recursive:true,force:true});}
}
test('completed evidence verifies and incomplete memory-instrument pilot stays rejected',()=>{
  assert.equal(verify(collection).verified,true);
  assert.throws(()=>verify('benchmarks/results/reproduction-m-macos-01'));
});
test('missing artifact hashes cannot silently disable integrity checking',()=>damaged(m=>{m.artifacts={};}));
test('changed compressed panel is rejected',()=>damaged((m,d)=>writeFileSync(join(d,'plain-a-panel.json.gz'),'changed')));
test('changed source identity is rejected',()=>damaged(m=>{m.sources['Cargo.toml']='0'.repeat(64);}));
test('false binary equality is rejected',()=>damaged(m=>{m.binary_equal.remap=!m.binary_equal.remap;}));
test('favourable replacement of measured performance is rejected',()=>damaged(m=>{m.performance[0].time.a=0;}));
test('missing measured process is rejected',()=>damaged(m=>{m.measurements.pop();}));
test('failure labels cannot be concealed by setting complete true',()=>damaged(m=>{m.failure='instrument failed';}));
test('comparison requires at least two verified collections',()=>{
  assert.throws(()=>compare([collection]));
  const result=compare([collection,collection]);assert.equal(result.semantic_equal,true);
  // Comparing a collection with itself tests the comparator, not independent replication.
});
test('WASM section comparison rejects malformed boundaries and preserves custom-section identity',()=>{
  const header=Buffer.from('0061736d01000000','hex');assert.deepEqual(sections(header),[]);
  const valid=Buffer.concat([header,Buffer.from([0,5,3,102,111,111,1])]);
  assert.equal(sections(valid)[0].name,'foo');
  for(const bytes of [Buffer.alloc(8),Buffer.concat([header,Buffer.from([10,3,1])]),Buffer.concat([header,Buffer.from([0,1,9])]),Buffer.concat([header,Buffer.from([10,128])])])assert.throws(()=>sections(bytes));
});
test('integrated audit retains positive results and the unsuccessful cross-host WASM alternative',()=>{
  const r=audit();assert.equal(r.primary.semantic_equal,true);assert.equal(r.primary.wasm_bytes_equal,false);
  assert.equal(r.linux_bytes_equal,true);assert.equal(r.within_host_tolerances_pass,true);
  assert.equal(r.counts.measurements,90);assert.equal(r.counts.warmups,18);
  assert.ok(r.wasm_diagnostic.sections.every(s=>s.differing.some(d=>d.id===10)&&s.differing.some(d=>d.id===11)));
  assert.ok(r.wasm_diagnostic.sysroot_remapping_changes_bytes.every(r=>!r.changed));
});
