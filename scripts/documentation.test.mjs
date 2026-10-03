// Documentation moves must preserve live links and frozen experiment verification.
import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync, spawnSync} from 'node:child_process';
import {readFileSync, writeFileSync, existsSync, mkdtempSync, mkdirSync, cpSync, rmSync, symlinkSync} from 'node:fs';
import {resolve, dirname, relative, join} from 'node:path';
import {tmpdir} from 'node:os';
import {gzipSync, gunzipSync} from 'node:zlib';
import {createHash} from 'node:crypto';
import {sourceFiles} from './research-source-files.mjs';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
test('research source collection is recursive, deterministic and refuses nonregular entries', () => {
  const dir=mkdtempSync(join(tmpdir(),'reflective-source-files-'));
  try {
    mkdirSync(join(dir,'nested'));
    for(const path of ['z.rs','a.rs','nested/b.rs','.hidden'])writeFileSync(join(dir,path),'fixture');
    const expected=['.hidden','a.rs','nested/b.rs','z.rs'].map(p=>`${dir}/${p}`);
    assert.deepEqual(sourceFiles([dir]),expected);
    // Exercise the collector with no executable search path, not just verification.
    const script=`import {sourceFiles} from ${JSON.stringify(new URL('./research-source-files.mjs',import.meta.url).href)}; console.log(JSON.stringify(sourceFiles(${JSON.stringify([dir])})));`;
    const output=execFileSync(process.execPath,['--input-type=module','-e',script],{env:{...process.env,PATH:''},encoding:'utf8'});
    assert.deepEqual(JSON.parse(output),expected);
    symlinkSync(join(dir,'nested'),join(dir,'link'),'dir');
    assert.throws(()=>sourceFiles([dir]),/Unsupported source entry/);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
test('frozen research verification needs neither rg nor any executable on PATH', () => {
  for(const [script,collection] of [['lambda-open-study.mjs','lambda-open-01'],['admission-study.mjs','admission-01']]) {
    const output=execFileSync(process.execPath,[`scripts/${script}`,'verify',`benchmarks/results/${collection}`],{env:{...process.env,PATH:''},encoding:'utf8'});
    assert.match(output,/Verified|verified/);
  }
});
test('admission assurance retains exact scope, results and both standalone accounts', () => {
  const report=JSON.parse(readFileSync('benchmarks/results/admission-01/report.json','utf8'));
  assert.deepEqual(report.counts,{abstract_cases:4930,closure_accepted:2080,countermodels:6,native_queries:3738,candidates:11739,admitted:7153,rejected:4586,boundary_queries:4});
  assert.equal(Object.keys(report.proof.axioms).length,7);
  assert.equal(report.proof.negative.length,6);
  for(const path of ['docs/dissertation.md','docs/archive/dissertation-full.md']) {
    const text=readFileSync(path,'utf8').split('# Bibliography')[0];
    for(const required of ['4,930','11,739','3,738','7,153','4,586','fresh','refinement','Söldner'])assert.ok(text.includes(required),`${path}: ${required}`);
    assert.doesNotMatch(text,/\]\((?:\.\.\/)*docs\//);
  }
  execFileSync(process.execPath,['scripts/admission-study.mjs','verify','benchmarks/results/admission-01']);
});
test('open lambda extension retains scoped evidence, compatibility and standalone paper coverage', () => {
  const evidence=JSON.parse(readFileSync('examples/lambda/open/evidence.json','utf8'));
  assert.equal(evidence.profile,'open-cbn-v1');assert.equal(evidence.cases.length,450);
  assert.equal(new Set(evidence.cases.map(r=>r.source)).size,450);
  assert.equal(evidence.cases.filter(r=>!/[xy]/.test(r.source)).length,62);
  assert.equal(evidence.cases.reduce((n,r)=>n+r.transitions.length,0),704);
  for(const row of evidence.cases)assert.equal(row.readbacks.length,row.transitions.length+1);
  for(const path of ['docs/dissertation.md','docs/archive/dissertation-full.md']) {
    const text=readFileSync(path,'utf8').split('# Bibliography')[0];
    for(const required of ['450','704','capture','neutral','Γ'])assert.ok(text.includes(required),`${path}: ${required}`);
  }
  execFileSync(process.execPath,['scripts/lambda-open-study.mjs','verify','benchmarks/results/lambda-open-01']);
});
test('minimality papers retain the verified comparison and condensed word budget', () => {
  const short=readFileSync('docs/dissertation.md','utf8').split('# Bibliography')[0];
  const full=readFileSync('docs/archive/dissertation-full.md','utf8').split('# Bibliography')[0];
  assert.ok(short.trim().split(/\s+/).length<=8000,'condensed body exceeds its working budget');
  for(const text of [short,full]) {
    for(const required of ['U_M','U_T','U_P','238,480','1,431'])assert.ok(text.includes(required),required);
    assert.match(text,/variable-heavy|128 additional unowned/);
  }
  const report=JSON.parse(readFileSync('benchmarks/results/minimality-03/report.json','utf8'));
  assert.equal(report.native_cases,477);assert.equal(report.translated_queries,1431);
  assert.equal(report.gadgets.reduce((sum,row)=>sum+row.total,0),238480);
});
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
