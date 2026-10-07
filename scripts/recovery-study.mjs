// AV-03/C01: one bounded diagnostic sample per cell, not a final benchmark.
import {spawn, execFileSync} from 'node:child_process';
import {readFileSync, writeFileSync, mkdirSync, existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import {resolve} from 'node:path';
import os from 'node:os';

const [directory] = process.argv.slice(2);
if (!directory || process.argv.length !== 3 || existsSync(directory))
  throw Error('node scripts/recovery-study.mjs NEW_OUTPUT_DIRECTORY');
if (process.platform !== 'darwin') throw Error('Collection uses macOS time -l; verification is portable.');
const binary = resolve('target/release/examples/recovery_probe');
const sha = x => createHash('sha256').update(x).digest('hex');
const protocol = readFileSync('benchmarks/PROTOCOL.md');
const paths = [...new Set(execFileSync('git', ['ls-files', '-co', '--exclude-standard'], {encoding:'utf8'}).trim().split('\n'))]
  .filter(p => existsSync(p) && (/^(Cargo\.(lock|toml)|rust-toolchain.toml|rustfmt.toml)$/.test(p)
    || /^crates\/.*\.(rs|toml)$/.test(p) || /^examples\//.test(p)
    || ['scripts/recovery-study.mjs', 'scripts/verify-recovery-study.mjs', 'benchmarks/PROTOCOL.md'].includes(p))).sort();
const bundle = gzipSync(JSON.stringify({schema:'source-bundle-v1', encoding:'base64',
  files:Object.fromEntries(paths.map(p => [p, readFileSync(p).toString('base64')]))}));
mkdirSync(directory, {recursive:true});
const save = (name, value) => writeFileSync(resolve(directory, name), JSON.stringify(value, null, 2)+'\n');
writeFileSync(resolve(directory, 'sources.json.gz'), bundle, {flag:'wx'});
writeFileSync(resolve(directory, 'protocol.md'), protocol, {flag:'wx'});
save('environment.json', {schema:'recovery-study-v1', date:new Date().toISOString(),
  commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),
  dirty_status:execFileSync('git',['status','--porcelain'],{encoding:'utf8'}),
  source_bundle_sha256:sha(bundle), protocol_sha256:sha(protocol), binary_sha256:sha(readFileSync(binary)),
  protocol_source_path:'benchmarks/PROTOCOL.md',
  os:os.release(), cpu:os.cpus()[0].model, memory:os.totalmem(), node:process.version,
  rust:execFileSync('rustc',['-Vv'],{encoding:'utf8'}), samples:1, warmups:0, timeout_ms:30000,
  memory_ceiling:null, output_limit_bytes:1048576});
let interrupted = false, cancel = null;
process.on('SIGINT', () => { interrupted = true; cancel?.('Cancelled'); });
async function sample(args) {
  return new Promise(done => {
    const start = performance.now();
    const child = spawn('/usr/bin/time', ['-l', binary, ...args.map(String)],
      {detached:true, stdio:['ignore','pipe','pipe']});
    let stdout = '', stderr = '', censored = null;
    const kill = why => { if (censored) return; censored = why;
      try { process.kill(-child.pid, 'SIGKILL'); } catch {} };
    cancel = kill;
    const timer = setTimeout(() => kill('Timeout'), 30000);
    child.stdout.on('data', b => { stdout += b; if (Buffer.byteLength(stdout) > 1048576) kill('OutputLimit'); });
    child.stderr.on('data', b => { stderr += b; if (Buffer.byteLength(stderr) > 1048576) kill('OutputLimit'); });
    child.on('error', e => { stderr += String(e); });
    child.on('close', (exit_code, signal) => {
      clearTimeout(timer); cancel = null;
      let result = null; try { result = JSON.parse(stdout); } catch {}
      const rss = stderr.match(/(\d+)\s+maximum resident set size/);
      done({args, exit_code, signal, censored, elapsed_ms:performance.now()-start,
        peak_rss_bytes:rss ? Number(rss[1]) : null, result, stdout, stderr});
    });
  });
}
const rows = [];
outer: for (const n of [0,1,3,10,100,1000]) for (const mode of ['finite','modulating']) {
  if (interrupted) break outer;
  const row = await sample([mode,n,2*n+4]); rows.push(row); save('raw.json', rows);
  console.log(row.args.join('/'), row.censored ?? row.result?.outcome ?? 'ProcessFailure');
}
if (interrupted || rows.some(r => !r.censored && (r.exit_code !== 0 || !r.result))) process.exitCode = 1;
