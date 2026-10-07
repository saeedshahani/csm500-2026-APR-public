import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {gunzipSync} from 'node:zlib';
const run=paths=>spawnSync('target/release/examples/minimality',['corpus','--paths'],{
  input:JSON.stringify(paths),encoding:'utf8',timeout:120000,maxBuffer:128*1024*1024,
});
test('frozen minimality input selection survives catalogue growth',()=>{
  const {corpus}=JSON.parse(gunzipSync(readFileSync('benchmarks/results/minimality-03/native.json.gz')));
  const paths=[...new Set(corpus.snapshots.map(s=>s.name).filter(p=>!p.startsWith('fixture/')))];
  assert.equal(paths.length,18);
  const result=run(paths);assert.equal(result.status,0,result.stderr);
  assert.deepEqual(JSON.parse(result.stdout),corpus);
});
test('versioned JSON is compiled and included, not silently skipped',()=>{
  const result=run(['reflection/versioned.json']);assert.equal(result.status,0,result.stderr);
  const corpus=JSON.parse(result.stdout);
  const names=new Set(corpus.snapshots.map(s=>s.name));
  assert.equal(names.size,5);assert.ok(names.has('reflection/versioned.json'));
  assert.ok(corpus.cases.some(c=>c.name==='reflection/versioned.json'&&c.answer.status==='Admitted'));
});
test('missing, duplicate and unsupported corpus paths fail without a report',()=>{
  for(const paths of [[],['missing.graph'],['reflection/versioned.txt'],['reflection/versioned.json','reflection/versioned.json']]) {
    const result=run(paths);assert.notEqual(result.status,0);assert.equal(result.stdout,'');
    assert.match(result.stderr,/distinct existing authored paths/);
  }
});
