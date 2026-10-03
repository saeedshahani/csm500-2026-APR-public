// Reject incomplete or damaged evidence without building or executing its payload.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdtempSync,cpSync,rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {verify,compare} from './reproducibility.mjs';

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
