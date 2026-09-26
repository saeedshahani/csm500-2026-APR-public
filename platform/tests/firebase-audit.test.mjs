import test from 'node:test';
import assert from 'node:assert/strict';
import { audit, authSummary, budgetSummary } from '../firebase-audit.mjs';

const project = 'reflective-graph-f1cd4', number = '463455802523';
test('authentication audit never serializes credential or hash configuration', () => {
  const marker = 'must-not-appear';
  const summary = authSummary({ signIn: { email: { enabled: true, passwordRequired: true, secret: marker }, hashConfig: { signerKey: marker } },
    notification: { smtp: { password: marker } }, passwordPolicyConfig: { passwordPolicyVersions: [{ customStrengthOptions: { minPasswordLength: 12, unknownSecret: marker } }] } });
  assert.equal(summary.email_password_enabled, true);
  assert.deepEqual(summary.password_policy.minimum_lengths, [12]);
  assert.equal(JSON.stringify(summary).includes(marker), false);
});
test('budget audit does not infer enforcement or disclose billing identifiers', () => {
  const summary = budgetSummary({ name: 'private-billing-id', budgetFilter: { projects: [`projects/${number}`], calendarPeriod: 'MONTH' },
    amount: { specifiedAmount: { currencyCode: 'GBP', units: '5' } }, allUpdatesRule: { monitoringNotificationChannels: ['private-channel'] },
    thresholdRules: [{ thresholdPercent: 0.5 }] }, project, number);
  assert.equal(summary.project_only, true); assert.equal(summary.amount.units, '5');
  assert.equal(summary.enforcement, 'console-verification-required');
  assert.equal(JSON.stringify(summary).includes('private-'), false);
  assert.equal(budgetSummary({ budgetFilter: { projects: [`projects/${project}`, 'projects/other'] } }, project, number).project_only, false);
});
test('audit refuses wrong targets before any request and stops on project-number mismatch', async () => {
  let calls = 0;
  await assert.rejects(audit('other', async () => { calls++; }), /Explicit/); assert.equal(calls, 0);
  await assert.rejects(audit(project, async () => { calls++; return { status: 200, body: { projectId: project, projectNumber: 'other' } }; }), /identity/);
  assert.equal(calls, 1);
});
test('audit scopes budgets and preserves permission denials without raw response leaks', async () => {
  const urls = [];
  const report = await audit(project, async url => {
    urls.push(url);
    if (url.endsWith(`/projects/${project}`)) return { status: 200, body: { projectId: project, projectNumber: number } };
    if (url.endsWith('/billingInfo')) return { status: 200, body: { billingEnabled: true, billingAccountName: 'billingAccounts/ABC-DEF' } };
    if (url.includes('/budgets?')) { assert.equal(new URL(url).searchParams.get('scope'), `projects/${project}`); return { status: 403, body: { error: { status: 'PERMISSION_DENIED', message: 'must-not-appear' } } }; }
    if (url.includes('/config?')) assert.equal(new URL(url).searchParams.get('fields').includes('hashConfig'), false);
    return { status: 200, body: {} };
  });
  assert.equal(urls.length, 9); assert.equal(report.checks.budgets.status, 403);
  assert.equal(JSON.stringify(report).includes('must-not-appear'), false);
  assert.equal(JSON.stringify(report).includes('ABC-DEF'), false);
});
