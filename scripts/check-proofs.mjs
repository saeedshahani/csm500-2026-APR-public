// Abstract proof acceptance. Native refinement is a separate obligation.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdtempSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
const lean=process.env.LEAN_BIN??'lean';
assert.match(execFileSync(lean,['--version'],{encoding:'utf8'}),/version 4\.19\.0\b/);
const files=[
  'crates/engine/proofs/Admission.lean',
  'crates/engine/proofs/MinimalityBounds.lean',
  'crates/engine/proofs/WorkflowContracts.lean',
  'proofs/organization/Composition.lean',
  'proofs/versioned/Publication.lean',
  'proofs/action_learning/Safety.lean',
  'proofs/observation/Certificates.lean',
  'crates/engine/proofs/RepresentationCosts.lean',
  'crates/engine/proofs/SelectedProvenance.lean',
  'proofs/numerical/LocalCycles.lean',
];
const temporary=mkdtempSync(join(tmpdir(),'graph-proof-audit-'));
let total=0;
for(const [index,file] of files.entries()) {
  const source=readFileSync(file,'utf8');
  assert.doesNotMatch(source,/\bsorry\b|^axiom\s|\bnative_decide\b|\bimplemented_by\b/m,file);
  const namespace=source.match(/^namespace (\w+)/m)?.[1];
  const names=[...source.matchAll(/^theorem (\w+)/gm)].map(m=>`${namespace}.${m[1]}`);
  assert.ok(namespace&&names.length,file);
  // Appended commands audit every theorem, including those without authored prints.
  const audit=join(temporary,`Audit${index}.lean`);
  writeFileSync(audit,source+'\n'+names.map(n=>`#print axioms ${n}`).join('\n')+'\n');
  let output;
  if(file.startsWith('proofs/numerical/')) {
    assert.ok(process.env.NUMERICAL_MATHLIB&&process.env.NUMERICAL_LEAN_BIN,'set pinned Mathlib and Lean paths; numerical proof is mandatory');
    execFileSync('bash',['scripts/check-numerical-proof.sh'],{stdio:'pipe',timeout:180000});
    output=execFileSync(join(process.env.NUMERICAL_LEAN_BIN,'lake'),['--dir',process.env.NUMERICAL_MATHLIB,'env','lean',audit],{encoding:'utf8',timeout:180000});
  } else output=execFileSync(lean,[audit],{encoding:'utf8',timeout:180000});
  assert.doesNotMatch(output,/sorryAx|Lean\.ofReduceBool|warning:|error:/);
  for(const name of names) {
    const line=output.split('\n').find(l=>l.startsWith(`'${name}' `));
    assert.ok(line,`missing axiom report: ${name}`);
    if(!line.endsWith('does not depend on any axioms')) {
      const axioms=line.match(/depends on axioms: \[(.*)\]/)?.[1];
      assert.notEqual(axioms,undefined,line);
      for(const axiom of axioms.split(', ').filter(Boolean))
        assert.ok(['propext','Classical.choice','Quot.sound'].includes(axiom),`${name}: ${axiom}`);
    }
    console.log(line);
  }
  total+=names.length;
}
console.log(`Checked ${total} abstract statements in ${files.length} sources. No native-refinement claim.`);
