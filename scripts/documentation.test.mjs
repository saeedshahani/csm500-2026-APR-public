// Documentation moves must preserve live links and frozen experiment verification.
import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync, spawnSync} from 'node:child_process';
import {readFileSync, writeFileSync, existsSync, mkdtempSync, cpSync, rmSync} from 'node:fs';
import {resolve, dirname, relative, join} from 'node:path';
import {tmpdir} from 'node:os';
import {gzipSync, gunzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
test('live repository Markdown links resolve after consolidation', () => {
  const paths = [...new Set(execFileSync('git', ['ls-files','-co','--exclude-standard'], {encoding:'utf8'}).trim().split('\n'))]
    .filter(p => p.endsWith('.md') && !p.startsWith('benchmarks/results/') && existsSync(p));
  const missing = [];
  for (const file of paths) {
    const text = readFileSync(file,'utf8').replace(/^```[^\n]*\n[\s\S]*?^```\s*$/gm,'')
      .replace(/(`+)[^\n]*?\1/g,'');
    for (const [,ref] of text.matchAll(/\[[^\]\n]*\]\(([^\n)]+)\)/g)) {
      const pathname = ref.split('#')[0];
      if (!pathname || /^(?:[a-z]+:|\/\/)/i.test(pathname)) continue;
      const target = resolve(dirname(file),decodeURIComponent(pathname));
      // Explainers are intentionally local/ignored; not a checkout dependency.
      if (relative(process.cwd(),target).startsWith('docs/explainers/')) continue;
      if (!existsSync(target)) missing.push(`${file}: ${ref}`);
    }
  }
  assert.deepEqual(missing,[]);
  assert.match(readFileSync('frontend/src/Guide.tsx','utf8'),/docs\/guides\/workbench\.md\?raw/);
  for (const [path,anchors] of [
    ['docs/research/evaluation.md',['performance','scalability']],
    ['docs/research/reflective-recovery.md',['methods','recovery']],
  ]) for (const anchor of anchors) assert.ok(readFileSync(path,'utf8').includes(`<a id="${anchor}"></a>`));
});

for (const [name,collection,verifier,oldPath,newPath] of [
  ['scale','s6-cycle02','verify-scale-study.mjs','docs/scalability-study.md','docs/research/evaluation.md'],
  ['recovery','recovery-c01-rss','verify-recovery-study.mjs','docs/recovery-study.md','docs/research/reflective-recovery.md'],
]) test(`${name} verifier accepts historical and explicit relocated protocol paths`, () => {
  const source = resolve('benchmarks/results',collection);
  // First verify the actual immutable evidence, without rewriting it.
  execFileSync(process.execPath,[`scripts/${verifier}`,source]);
  const temporary = mkdtempSync(join(tmpdir(),'reflective-doc-protocol-'));
  try {
    cpSync(source,temporary,{recursive:true});
    const environment = JSON.parse(readFileSync(join(temporary,'environment.json')));
    const bundle = JSON.parse(gunzipSync(readFileSync(join(temporary,'sources.json.gz'))));
    // Synthetic path-migration fixture, not a new measurement collection.
    bundle.files[newPath] = bundle.files[oldPath]; delete bundle.files[oldPath];
    const bytes = gzipSync(JSON.stringify(bundle));
    environment.protocol_source_path = newPath;
    environment.source_bundle_sha256 = sha(bytes);
    writeFileSync(join(temporary,'sources.json.gz'),bytes);
    writeFileSync(join(temporary,'environment.json'),JSON.stringify(environment));
    execFileSync(process.execPath,[`scripts/${verifier}`,temporary]);
    environment.protocol_source_path = 'unrecognised.md';
    writeFileSync(join(temporary,'environment.json'),JSON.stringify(environment));
    assert.notEqual(spawnSync(process.execPath,[`scripts/${verifier}`,temporary]).status,0);
  } finally {
    rmSync(temporary,{recursive:true,force:true});
  }
});
