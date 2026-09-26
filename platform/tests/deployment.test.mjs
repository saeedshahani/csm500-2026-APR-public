import test from 'node:test';
import assert from 'node:assert/strict';
import {target} from '../deployment-target.mjs';
test('deployment preflight refuses defaults and every non-authorised project',()=>{
  for(const id of [undefined,'','default','reflective-graph','demo-reflective-graph','another-project']) assert.throws(()=>target(id),/Explicit/);
  const config=target('reflective-graph-f1cd4');
  assert.equal(config.project_number,'463455802523');
  assert.equal(config.deployment_approved,false);assert.equal(config.billing_changes_approved,false);
  assert.deepEqual(config.monthly_budget,{currency:'GBP',amount:5});
  assert.equal(config.audience,'public-email-signup');
  assert.equal(config.identity_provider,'email-password');
  assert.equal(config.require_verified_email,true);
  assert.equal(config.billing_status,'blaze-user-confirmed');
  assert.equal(config.budget_enforcement,'unverified');
});
