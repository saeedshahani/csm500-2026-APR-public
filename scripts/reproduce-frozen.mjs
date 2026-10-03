// Execute the archived orchestrator, not whatever sources now occupy the checkout.
// Only run reviewed bundles: integrity hashes do not authenticate executable code.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,existsSync,mkdtempSync,mkdirSync,rmSync} from 'node:fs';
import {resolve,join,dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {gunzipSync} from 'node:zlib';
import {spawnSync,execFileSync} from 'node:child_process';
import {verify,safePath,sha} from './reproducibility.mjs';
const [source,out,...extra]=process.argv.slice(2);
assert.ok(source&&out&&!extra.length,'node scripts/reproduce-frozen.mjs COLLECTION NEW_OUTPUT');
verify(source);const original=JSON.parse(readFileSync(join(source,'manifest.json')));assert.equal(original.historical,false,'Use the historical command for the older source family');
const output=resolve(out);assert.ok(!existsSync(output),'Never overwrite a collection');
const temporary=mkdtempSync(join(tmpdir(),'reflective-frozen-rebuild-'));
try {
  const bytes=readFileSync(join(source,'sources.json.gz')),bundle=JSON.parse(gunzipSync(bytes));
  for(const[p,content]of Object.entries(bundle)){safePath(p);assert.ok(!p.split('/').includes('.git'),'Git metadata is not a permitted source');const file=join(temporary,p);mkdirSync(dirname(file),{recursive:true});writeFileSync(file,Buffer.from(content,'base64'),{flag:'wx'});}
  // The archived collector queries HEAD. Give it an explicitly synthetic local
  // source-only commit, and identify that fact in the reconstruction receipt.
  const git=args=>execFileSync('git',args,{cwd:temporary,encoding:'utf8'});
  git(['init','--quiet']);git(['add','.']);git(['-c','user.name=Frozen source reconstruction','-c','user.email=reproduction@invalid','-c','commit.gpgsign=false','commit','--quiet','-m','Synthetic snapshot of verified research sources']);
  const synthetic=git(['rev-parse','HEAD']).trim();
  const binder=process.env.REPRO_BINDGEN??resolve('target/tools/bin/wasm-bindgen');
  const p=spawnSync(process.execPath,['scripts/reproducibility.mjs','collect',output],{cwd:temporary,stdio:'inherit',env:{...process.env,REPRO_BINDGEN:binder},timeout:3600000});
  assert.ok(!p.error&&!p.signal,p.error?.message??p.signal);assert.equal(p.status,0,'Frozen-source reconstruction failed; its incomplete output is retained');
  const m=JSON.parse(readFileSync(join(output,'manifest.json')));assert.deepEqual(m.sources,original.sources,'Reconstruction changed source bytes');assert.equal(m.base_commit,synthetic);
  m.reconstruction={source_bundle_sha256:sha(bytes),original_base_commit:original.base_commit,synthetic_commit:synthetic,meaning:'base_commit is a temporary source snapshot, not a published project revision',wrapper_sha256:sha(readFileSync(new URL(import.meta.url)))};
  writeFileSync(join(output,'manifest.json'),JSON.stringify(m,null,2)+'\n');
  console.log(JSON.stringify(verify(output),null,2));
}finally{rmSync(temporary,{recursive:true,force:true});}
