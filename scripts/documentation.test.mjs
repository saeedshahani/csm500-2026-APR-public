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
test('both manuscripts are standalone academic accounts of the adopted platform', () => {
  for (const path of ['docs/dissertation.md', 'docs/archive/dissertation-full.md']) {
    const body = readFileSync(path, 'utf8').split('# Bibliography')[0];
    assert.deepEqual([...body.matchAll(/^# (\d+)\. /gm)].map(m => Number(m[1])), [1,2,3,4,5,6,7,8,9]);
    assert.doesNotMatch(body, /\bdocs\/|\b[\w-]+\.md\b|\bADR-\d+|\bLoop [A-N]\b|\bS[1-7]-\d{3}\b/);
    for (const concept of ['Firestore', 'Cloud Tasks', 'Cloud Run', 'walkthrough', 'ephemeral', 'intrinsic', 'weak-head']) {
      assert.ok(body.includes(concept), `${path}: missing integrated scope ${concept}`);
    }
    assert.doesNotMatch(body, /I, M and O are distinct record kinds/);
  }
});

test('literature reconciliation preserves provenance and the admission boundary', () => {
  const full = readFileSync('docs/archive/dissertation-full.md', 'utf8');
  assert.match(full, /Tomita, Kurokawa and Murata \(2002, §2\)/);
  assert.match(full, /detailed metanode execution and modification rules are left for future work/);
  assert.ok(full.includes('Barendregt and Barendsen, 2000, Definitions 2.4–2.6'));
  assert.doesNotMatch(full, /The substitution is capture-avoiding \(Barendregt, 1984\)/);
  for (const path of ['docs/dissertation.md', 'docs/archive/dissertation-full.md']) {
    const text = readFileSync(path, 'utf8');
    assert.ok(text.includes('Ehrig, Prange and Ehrig, 2006'));
    assert.ok(text.includes('Plotkin, 2004'));
    assert.ok(text.includes('G —e→ G′'));
  }
  const formalism = readFileSync('docs/formalism.md', 'utf8');
  assert.ok(formalism.includes('### 5.4 Labelled transition judgment'));
  for (const premise of ['Ground(G,p,σ,Lσ,Kσ,Rσ)', 'Visible(G,p,Lσ)', 'Lifecycle(G,p,σ,G′)', 'EventOf(G,p,σ,G′)=e']) {
    assert.ok(formalism.includes(premise), premise);
  }
  const literature = readFileSync('docs/archive/literature.md', 'utf8');
  assert.ok(literature.includes('Closest reflective precedents: claim-to-source comparison'));
  assert.equal((literature.match(/^Plump, D\. \(2010\)/gm) ?? []).length, 1);
  assert.ok(readFileSync('docs/project-proposal.md', 'utf8').includes('Bibliographic correction (4 October 2026)'));
});

test('planning preserves sprint dates, task identities and explicit final-candidate gates', () => {
  const expected = ['13 Jul – 26 Jul', '27 Jul – 9 Aug', '10 Aug – 23 Aug', '24 Aug – 6 Sep', '7 Sep – 20 Sep', '21 Sep – 1 Oct', '2 Oct – 13 Oct'];
  for (const path of ['docs/sprint-plan.md', 'docs/task-list.md']) {
    const text = readFileSync(path, 'utf8');
    assert.deepEqual([...text.matchAll(/^## Sprint (\d+): (.+) 2026$/gm)].map(m => [Number(m[1]), m[2]]), expected.map((date, i) => [i+1, date]));
  }
  const tasks = readFileSync('docs/task-list.md', 'utf8');
  const ids = [...tasks.matchAll(/^- \[[ x]\] (S\d+-\d+) —/gm)].map(m => m[1]);
  assert.equal(ids.length, new Set(ids).size, 'Task identifiers must remain unique');
  for (const id of ['S6-047', 'S6-048', 'S6-078', 'S6-084', 'S6-093', 'S6-127']) {
    assert.ok(ids.includes(id), `Final-candidate obligation ${id} must remain traceable`);
  }
  const architecture = readFileSync('docs/architecture.md', 'utf8');
  assert.ok(architecture.includes('### 2.1 Interface and persistence boundaries'));
  assert.ok(architecture.includes('### 21.4 Recovery analysis and external repair'));
  assert.doesNotMatch(architecture, /optional caches await profiling/);
  const formalism = readFileSync('docs/formalism.md', 'utf8');
  assert.ok(formalism.includes('## 15. Task-relative interpretation and policy compilation'));
  assert.ok(formalism.includes('Selected abstract mutation'));
});

test('reproduction claims separate cross-environment semantics from byte identity and theorem scope',()=>{
  const r=JSON.parse(readFileSync('benchmarks/results/reproduction-m/report.json'));
  assert.equal(r.counts.native_builds,12);assert.equal(r.counts.measurements,90);assert.equal(r.counts.warmups,18);
  assert.equal(r.primary.semantic_equal,true);assert.equal(r.primary.wasm_bytes_equal,false);assert.equal(r.linux_bytes_equal,true);
  for(const file of ['docs/dissertation.md','docs/archive/dissertation-full.md']) {
    const body=readFileSync(file,'utf8').split('# Bibliography')[0];
    for(const text of ['Lamb and Zacchiroli','macOS','Linux','768','144','theorem domains'])assert.ok(body.includes(text),`${file}: ${text}`);
    assert.doesNotMatch(body,/\]\((?:\.\.\/)*docs\//);
  }
  assert.ok(readFileSync('docs/task-list.md','utf8').includes('- [x] S6-233 —'));
  assert.ok(readFileSync('docs/task-list.md','utf8').includes('S6-234 —'));
  execFileSync(process.execPath,['scripts/verify-reproduction.mjs','--check','benchmarks/results/reproduction-m/report.json'],{env:{...process.env,PATH:''}});
});
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
  for(const [script,collection] of [['lambda-open-study.mjs','lambda-open-01'],['admission-study.mjs','admission-01'],['learning-study.mjs','learning-02'],['meaning-study.mjs','meaning-01']]) {
    const output=execFileSync(process.execPath,[`scripts/${script}`,'verify',`benchmarks/results/${collection}`],{env:{...process.env,PATH:''},encoding:'utf8'});
    assert.match(output,/Verified|verified/);
  }
});
test('meaning claims distinguish normality, useful interpretation and intrinsic grounding',()=>{
  const r=JSON.parse(readFileSync('benchmarks/results/meaning-01/report.json'));
  assert.equal(r.native_runs,444);assert.equal(r.main_commits,3600);assert.equal(r.intervention_runs,120);assert.equal(r.intervention_commits,1092);
  for(const [method,correct]of [['adaptive',1296],['fixed',1296],['blind',468],['shuffled',36],['rule-free',0]])assert.equal(r.summary[method].correct,correct);
  for(const path of ['docs/dissertation.md','docs/archive/dissertation-full.md']) {
    const body=readFileSync(path,'utf8').split('# Bibliography')[0];
    for(const required of ['1,296','444','3,600','120','1,092','Harnad','context','unresolved'])assert.ok(body.includes(required),`${path}: ${required}`);
    assert.doesNotMatch(body,/\]\((?:\.\.\/)*docs\//);
  }
  const tasks=readFileSync('docs/task-list.md','utf8');for(const id of ['S6-183','S6-232'])assert.ok(tasks.includes(`- [x] ${id} —`));
});
test('learning claims retain strong baselines, native scope and both standalone papers',()=>{
  const r=JSON.parse(readFileSync('benchmarks/results/learning-02/report.json'));
  assert.equal(r.native_cases,201);assert.equal(r.native_commits,2412);
  for(const method of ['learned','consensus','human','search'])assert.equal(r.summary[method].positive_correct,2304);
  assert.equal(r.summary.random.positive_correct,504);assert.equal(r.summary.memory.positive_correct,0);
  for(const path of ['docs/dissertation.md','docs/archive/dissertation-full.md']) {
    const body=readFileSync(path,'utf8').split('# Bibliography')[0];
    for(const required of ['2,304','2,412','201','64','Gulwani','external'])assert.ok(body.includes(required),`${path}: ${required}`);
    assert.doesNotMatch(body,/\]\((?:\.\.\/)*docs\//);
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
