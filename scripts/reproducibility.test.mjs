import test from 'node:test';
import assert from 'node:assert/strict';
import {safePath,median,agreement,binaryInfo,PROTOCOL} from './reproducibility.mjs';
import {proposals,nativePanel} from './reproduction-panel.mjs';
import {resolve} from 'node:path';

test('source extraction cannot escape its fresh directory',()=>{
  for(const p of ['/tmp/escape','../escape','a/../b','a\\b','./a','a//b',''])assert.throws(()=>safePath(p));
  assert.equal(safePath('crates/engine/src/lib.rs'),'crates/engine/src/lib.rs');
});
test('predeclared tolerances preserve adverse performance findings',()=>{
  assert.equal(median([3,1,2]),2);assert.equal(median([1,3]),2);
  assert.equal(agreement([100],[125],0.25,0).accepted,true);assert.equal(agreement([100],[126],0.25,0).accepted,false);
  assert.equal(agreement([1],[2],0,1).accepted,true);assert.equal(PROTOCOL.repetitions,5);
});
test('binary identity is separate from embedded source paths and format',()=>{
  const a=Buffer.from('test /source-one/file.rs'),b=Buffer.from('test /source-two/file.rs');
  assert.notEqual(binaryInfo(a).sha256,binaryInfo(b).sha256);assert.equal(binaryInfo(a,'/source-one').source_path_present,true);
  assert.equal(binaryInfo(a,'/source-two').source_path_present,false);
  assert.equal(binaryInfo(Buffer.from([0x7f,69,76,70])).format,'ELF');
});
test('all learned proposals reproduce without native execution',()=>{
  const a=proposals(),b=proposals();assert.deepEqual(a,b);assert.equal(a.calendar.length,768);assert.equal(a.context.length,144);
});
test('representative native panel checks history, interpretation, codecs and corrupted evidence',()=>{
  const result=nativePanel(resolve('target/release/cli'),resolve('target/release/examples/protocol'));
  assert.equal(result.runs.length,14);assert.equal(result.corrupt_trace_rejected,true);
  assert.ok(result.runs.some(r=>r.trace.report.outcome==='StepLimit'));
});
