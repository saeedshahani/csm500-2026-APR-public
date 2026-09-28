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
test('S6 loop plans have one task ledger and valid planning anchors', () => {
  const loops = readFileSync('docs/research/loop-plans.md', 'utf8');
  const tasks = readFileSync('docs/task-list.md', 'utf8');
  const sprint = readFileSync('docs/sprint-plan.md', 'utf8');
  const research = readFileSync('docs/research-plan.md', 'utf8');
  const ids = [...tasks.matchAll(/^- \[[ x]\] (S6-\d+) —/gm)].map(m => m[1]);
  assert.equal(ids.length, new Set(ids).size, 'S6 task completion entries must be unique');
  assert.doesNotMatch(loops, /^- \[[ x]\]/m, 'The loop plan must not duplicate the completion ledger');
  const headings = [...loops.matchAll(/^## Loop ([B-N]) — (.+)$/gm)];
  assert.deepEqual(headings.map(m => m[1]), [...'BCDEFGHIJKLMN']);
  const allocations = {B:[208,209,212],C:[210,211],D:[221,222],E:[223,224,225],F:[226],G:[227],H:[181],I:[228],J:[229,230],K:[182,231],L:[183,232],M:[233],N:[234]};
  for (const [i, heading] of headings.entries()) {
    const section = loops.slice(heading.index, headings[i+1]?.index ?? loops.length);
    for (const number of allocations[heading[1]]) {
      const id = `S6-${number}`;
      assert.ok(section.includes(id), `${id} must have a detailed Loop ${heading[1]} plan`);
      assert.equal(ids.filter(x => x === id).length, 1, `${id} must retain one completion entry`);
    }
  }
  // GitHub-style anchors for the simple, non-duplicate headings used in these plans.
  const slug = heading => heading.toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, '').replace(/\s/g, '-');
  for (const [source, text] of [['docs/research/loop-plans.md',loops], ['docs/task-list.md',tasks], ['docs/sprint-plan.md',sprint], ['docs/research-plan.md',research]]) {
    for (const [, pathname, anchor] of text.matchAll(/\]\(([^)#]+\.md)#([^)]*)\)/g)) {
      const target = resolve(dirname(source),pathname);
      if (!['loop-plans.md','task-list.md','sprint-plan.md'].some(name => target.endsWith('/'+name))) continue;
      const content = readFileSync(target,'utf8');
      const anchors = new Set([...content.matchAll(/^#{1,6} (.+)$/gm)].map(m => slug(m[1])));
      assert.ok(anchors.has(anchor) || content.includes(`id="${anchor}"`), `${source}: missing ${pathname}#${anchor}`);
    }
  }
  assert.ok(sprint.includes('research/loop-plans.md'));
  assert.ok(research.includes('research/loop-plans.md'));
});

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
