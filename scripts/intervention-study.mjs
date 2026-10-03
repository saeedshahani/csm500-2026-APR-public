// Retained finite-universe oracle and diagnostic timing; never overwrite evidence.
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, existsSync, mkdirSync} from 'node:fs';
import {execFileSync, spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {gzipSync, gunzipSync} from 'node:zlib';
import {resolve} from 'node:path';
import os from 'node:os';
const [mode, directory] = process.argv.slice(2);
assert.ok(['collect', 'verify'].includes(mode) && directory, 'collect|verify DIRECTORY');
const sha = x => createHash('sha256').update(x).digest('hex');
const binary = resolve('target/release/examples/interventions');
const cli = resolve('target/release/cli');
function oracle(cell) {
  const n = cell.prefix, total = 2 ** n, target = cell.trace.events[n];
  assert.equal(cell.reports.length, 2);
  assert.deepEqual(cell.reports.map(x=>x.report.query), ['ExactOccurrence','SourceRevision']);
  for (const {report} of cell.reports) {
    assert.equal(report.schema, 'reflective-occurrence-interventions-v1');
    assert.equal(report.trials.length, total);
    assert.equal(report.target, String(n));
    const exact = report.query === 'ExactOccurrence';
    const targetAtoms = new Set(target.redex.binding.map(x => x[1]));
    const required = cell.trace.events.slice(0, n).flatMap((e, i) => Object.keys(e.fresh).some(id => targetAtoms.has(id)) ? [i] : []);
    const successful = [];
    for (let mask = 0; mask < total; mask++) {
      const retained = Array.from({length:n}, (_,i) => i).filter(i => mask & 2 ** i);
      const admitted = n === 0 || (cell.family === 'conjunction' ? retained.length === n : exact ? required.every(i => retained.includes(i)) : retained.length > 0);
      const trial = report.trials[mask];
      assert.deepEqual(trial.retained, retained.map(String));
      assert.equal(trial.outcome.kind === 'Admitted', admitted);
      assert.notEqual(trial.outcome.kind, 'InfeasiblePrefix');
      assert.equal(trial.witness_binding !== null, admitted);
      if (admitted) {
        successful.push(retained);
        if (exact) assert.deepEqual(trial.witness_binding, target.redex.binding);
        const available = new Set(Object.keys(cell.trace.initial.atoms));
        for (const i of retained) for (const id of Object.keys(cell.trace.events[i].fresh)) available.add(id);
        for (const [a,b] of trial.witness_binding) assert.ok(available.has(a) && available.has(b));
      }
    }
    const minimal = successful.filter(s => !successful.some(t => t.length < s.length && t.every(i => s.includes(i))));
    assert.deepEqual(report.minimal_supports, minimal.map(s => s.map(String)));
    assert.deepEqual(report.necessary_in_this_universe, Array.from({length:n}, (_,i) => i).filter(i => successful.every(s => s.includes(i))).map(String));
  }
}
function nativeCheck(cell) {
  const result = spawnSync(cli, ['trace-check', '-'], {input:JSON.stringify(cell.trace), encoding:'utf8', timeout:60000});
  assert.equal(result.status, 0, result.stderr);
}
if (mode === 'collect') {
  assert.equal(os.platform(), 'darwin', 'RSS protocol uses macOS time -l bytes');
  assert.ok(!existsSync(directory), 'evidence directory already exists');
  mkdirSync(directory, {recursive:true});
  const paths = [...new Set(execFileSync('git', ['ls-files','-co','--exclude-standard'], {encoding:'utf8'}).trim().split('\n'))].filter(p => existsSync(p) && (/^(Cargo\.(toml|lock)|rust-toolchain.toml)$/.test(p) || /^crates\/.*\.(rs|toml)$/.test(p) || p === 'scripts/intervention-study.mjs' || p === 'scripts/intervention-model.test.mjs' || p === 'docs/research/intervention-protocol.md')).sort();
  const source = gzipSync(JSON.stringify({files:Object.fromEntries(paths.map(p => [p, readFileSync(p).toString('base64')]))}));
  writeFileSync(resolve(directory,'sources.json.gz'),source);
  const manifest = {schema:'intervention-study-v1',complete:false,created:new Date().toISOString(),parent:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),source_sha256:sha(source),binary_sha256:sha(readFileSync(binary)),cli_sha256:sha(readFileSync(cli)),environment:{platform:os.platform(),arch:os.arch(),cpu:os.cpus()[0].model,memory_bytes:os.totalmem(),node:process.version,rust:execFileSync('rustc',['--version'],{encoding:'utf8'}).trim()},rows:[],failure:null};
  try {
    for (const family of ['conjunction','alternatives']) for (let n=0;n<=8;n++) {
      const samples=[];
      for(let repeat=0;repeat<3;repeat++) {
        const result=spawnSync('/usr/bin/time',['-l',binary,family,String(n)],{encoding:'utf8',timeout:60000,maxBuffer:32*1024*1024});
        assert.equal(result.status,0,result.error?.message??result.stderr);
        const cell=JSON.parse(result.stdout); oracle(cell); nativeCheck(cell);
        const rss=/([0-9]+)\s+maximum resident set size/.exec(result.stderr); assert.ok(rss);
        samples.push({cell,peak_rss_bytes:Number(rss[1]),time_stderr:result.stderr,report_bytes:cell.reports.map(x=>Buffer.byteLength(JSON.stringify(x.report)))});
      }
      const filename=`${family}-${n}.json.gz`, bytes=gzipSync(JSON.stringify(samples));writeFileSync(resolve(directory,filename),bytes);
      const row={family,prefix:n,filename,sha256:sha(bytes),trials_per_query:2**n,elapsed_ns:samples.map(s=>s.cell.reports.map(x=>x.elapsed_ns)),peak_rss_bytes:samples.map(s=>s.peak_rss_bytes),report_bytes:samples[0].report_bytes};
      manifest.rows.push(row); console.log(JSON.stringify(row));
    }
    assert.equal(sha(readFileSync(binary)),manifest.binary_sha256,'analysis binary changed during collection');
    assert.equal(sha(readFileSync(cli)),manifest.cli_sha256,'replay binary changed during collection');
    manifest.complete=true;
  } catch(e) { manifest.failure=String(e.stack);process.exitCode=1; }
  writeFileSync(resolve(directory,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
} else {
  const m=JSON.parse(readFileSync(resolve(directory,'manifest.json')));assert.equal(m.complete,true);assert.equal(m.rows.length,18);
  assert.equal(sha(readFileSync(resolve(directory,'sources.json.gz'))),m.source_sha256);
  if(process.argv.includes('--native')) {
    assert.equal(sha(readFileSync(binary)),m.binary_sha256,'native verification requires the recorded analysis binary');
    assert.equal(sha(readFileSync(cli)),m.cli_sha256,'native verification requires the recorded replay binary');
  }
  let trials=0;
  for(const row of m.rows){const bytes=readFileSync(resolve(directory,row.filename));assert.equal(sha(bytes),row.sha256);const samples=JSON.parse(gunzipSync(bytes));assert.equal(samples.length,3);for(const sample of samples){oracle(sample.cell);if(process.argv.includes('--native'))nativeCheck(sample.cell);trials+=sample.cell.reports.reduce((s,x)=>s+x.report.trials.length,0);}}
  console.log(`Verified 18 cells, 54 process samples, ${trials} query/subset outcomes; native trace replay ${process.argv.includes('--native')?'repeated':'not repeated'}.`);
  if(process.argv.includes('--live')) {
    for(const family of ['conjunction','alternatives']) for(let n=0;n<=8;n++) {
      const result=spawnSync(binary,[family,String(n)],{encoding:'utf8',timeout:60000,maxBuffer:32*1024*1024});
      assert.equal(result.status,0,result.stderr);const cell=JSON.parse(result.stdout);oracle(cell);nativeCheck(cell);
    }
    console.log('Fresh-build Boolean oracle and native replay passed for all 18 cells (no cross-platform timing comparison).');
  }
}
