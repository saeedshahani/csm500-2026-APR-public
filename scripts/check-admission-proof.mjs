import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdtempSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';

export function checkProof(lean=process.env.LEAN_BIN??'lean') {
  const run=(args,options={})=>spawnSync(lean,args,{encoding:'utf8',timeout:60000,...options});
  const version=run(['--version']);assert.equal(version.status,0,version.error?.message??version.stderr);
  assert.match(version.stdout,/version 4\.19\.0\b/,'pinned Lean 4.19.0 required');
  const source='crates/engine/proofs/Admission.lean';
  const content=readFileSync(source,'utf8');
  assert.doesNotMatch(content,/\b(sorry|admit|axiom|native_decide|implemented_by|extern|unsafe)\b/);
  const dir=mkdtempSync(join(tmpdir(),'reflective-admission-'));
  const proof=run(['-o',join(dir,'Admission.olean'),source]);
  assert.equal(proof.status,0,proof.stdout+proof.stderr);
  assert.doesNotMatch(proof.stdout,/sorryAx|Lean\.ofReduceBool|warning:/);
  const names=['preservation','exact_admission','checker_exact','retained_immutable','unique_preserved','assignments_exact','search_exact'];
  const axiomSets={};
  for(const name of names) {
    const line=proof.stdout.split('\n').find(l=>l.startsWith(`'Admission.${name}' depends on axioms:`));
    assert.ok(line,`missing axiom audit: ${name}`);
    const axioms=line.match(/\[(.*)\]/)[1].split(', ').filter(Boolean);
    assert.deepEqual(axioms,name==='assignments_exact'||name==='search_exact'?['propext','Classical.choice','Quot.sound']:['propext','Quot.sound']);
    axiomSets[name]=axioms;
  }
  const env={...process.env,LEAN_PATH:dir};
  const falseObligations=[
    'closedCheck (patch [(0,[1]),(1,[])] [1] []) = true',
    'admissionCheck [(0,[])] [] [(1,[2])] = true',
    'admissionCheck [] [] [(0,[1]),(1,[0])] = false',
    'freshCheck [(0,[])] [(0,[])] = true',
    'closedCheck (patch [(0,[1])] [] []) = true',
    '(patch [] [] [(0,[]),(0,[])]).map Prod.fst |>.Nodup',
  ];
  const negative=[];
  for(const [i,statement] of falseObligations.entries()) {
    const path=join(dir,`False${i}.lean`);
    writeFileSync(path,`import Admission\nopen Admission\nexample : ${statement} := by decide\n`);
    const result=run([path],{env});
    assert.notEqual(result.status,0);
    // A syntax/import failure is not evidence of a false statement.
    assert.match(result.stdout,/tactic 'decide' proved that the proposition.*is false/s,result.stdout);
    negative.push({statement,diagnostic:result.stdout.replaceAll(dir,'<temporary>')});
  }
  const runner=run(['crates/engine/proofs/AdmissionRunner.lean'],{env});
  assert.equal(runner.status,0,runner.stdout+runner.stderr);
  return {lean,dir,env,evidence:{version:version.stdout.trim(),axioms:axiomSets,negative}};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href) console.log(JSON.stringify(checkProof().evidence,null,2));
