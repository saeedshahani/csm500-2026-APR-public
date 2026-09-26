// Read-only, explicitly project-scoped inspection. Never print raw provider responses.
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { target } from './deployment-target.mjs';

export function authSummary(value) {
  return {
    email_password_enabled: value.signIn?.email?.enabled === true && value.signIn?.email?.passwordRequired === true,
    anonymous_enabled: value.signIn?.anonymous?.enabled === true,
    email_enumeration_protection: value.emailPrivacyConfig?.enableImprovedEmailPrivacy === true,
    authorized_domains: Array.isArray(value.authorizedDomains) ? value.authorizedDomains.filter(x => typeof x === 'string') : [],
    password_policy: value.passwordPolicyConfig ? {
      enforcement: value.passwordPolicyConfig.passwordPolicyEnforcementState,
      minimum_lengths: value.passwordPolicyConfig.passwordPolicyVersions?.map(x => x.customStrengthOptions?.minPasswordLength),
    } : null,
  };
}

export function budgetSummary(value, projectId, projectNumber) {
  const scopes = new Set([`projects/${projectId}`, `projects/${projectNumber}`]);
  return {
    project_only: value.budgetFilter?.projects?.length === 1 && scopes.has(value.budgetFilter.projects[0]),
    amount: value.amount?.specifiedAmount ? {
      currency: value.amount.specifiedAmount.currencyCode,
      units: value.amount.specifiedAmount.units,
      nanos: value.amount.specifiedAmount.nanos,
    } : null,
    calendar_period: value.budgetFilter?.calendarPeriod,
    service_restricted: !!value.budgetFilter?.services?.length,
    thresholds: value.thresholdRules?.map(x => x.thresholdPercent),
    default_notifications_disabled: value.allUpdatesRule?.disableDefaultIamRecipients === true,
    // Provider API omits some console fields. Do not infer spend-cap enforcement.
    enforcement: 'console-verification-required',
  };
}

export async function audit(projectId, get) {
  const config = target(projectId), project = config.project_id;
  const report = { schema: 'reflective-firebase-audit-v1', project_id: project, checked_at: new Date().toISOString(), checks: {} };
  async function check(name, url, select) {
    try {
      const response = await get(url);
      report.checks[name] = { status: response.status, ...(response.status === 200
        ? { result: select(response.body) } : { error: response.body?.error?.status ?? 'REQUEST_FAILED',
          reason: response.body?.error?.details?.find(x => x['@type'] === 'type.googleapis.com/google.rpc.ErrorInfo')?.reason }) };
      return response.status === 200 ? response.body : null;
    } catch { report.checks[name] = { status: null, error: 'REQUEST_FAILED' }; return null; }
  }
  const identity = await check('project', `https://firebase.googleapis.com/v1beta1/projects/${project}`, b => ({
    project_id: b.projectId, project_number: b.projectNumber, state: b.state,
  }));
  if (!identity || identity.projectId !== project || String(identity.projectNumber) !== config.project_number)
    throw Error('Project identity could not be verified; audit stopped');
  await check('web_apps', `https://firebase.googleapis.com/v1beta1/projects/${project}/webApps`, b => ({
    apps: (b.apps ?? []).map(x => ({ app_id: x.appId, name: x.displayName, state: x.state })), incomplete: !!b.nextPageToken,
  }));
  const fields = 'signIn/email,signIn/anonymous,authorizedDomains,passwordPolicyConfig,emailPrivacyConfig';
  await check('authentication', `https://identitytoolkit.googleapis.com/admin/v2/projects/${project}/config?fields=${encodeURIComponent(fields)}`, authSummary);
  const billing = await check('billing', `https://cloudbilling.googleapis.com/v1/projects/${project}/billingInfo`, b => ({ enabled: b.billingEnabled === true }));
  if (/^billingAccounts\/[A-Z0-9-]+$/.test(billing?.billingAccountName ?? '')) {
    const url = new URL(`https://billingbudgets.googleapis.com/v1/${billing.billingAccountName}/budgets`);
    url.searchParams.set('scope', `projects/${project}`);
    await check('budgets', url.href, b => ({ budgets: (b.budgets ?? []).map(x => budgetSummary(x, project, config.project_number)), incomplete: !!b.nextPageToken }));
  } else report.checks.budgets = { status: null, error: 'NO_VERIFIED_BILLING_ACCOUNT' };
  await check('firestore', `https://firestore.googleapis.com/v1/projects/${project}/databases`, b => ({
    databases: (b.databases ?? []).map(x => ({ name: x.name, location: x.locationId, type: x.type, edition: x.databaseEdition })), incomplete: !!b.nextPageToken,
  }));
  await check('storage', `https://storage.googleapis.com/storage/v1/b?project=${project}`, b => ({
    buckets: (b.items ?? []).map(x => ({ name: x.name, location: x.location, public_access_prevention: x.iamConfiguration?.publicAccessPrevention,
      uniform_access: x.iamConfiguration?.uniformBucketLevelAccess?.enabled })), incomplete: !!b.nextPageToken,
  }));
  await check('hosting', `https://firebasehosting.googleapis.com/v1beta1/projects/${project}/sites`, b => ({
    sites: (b.sites ?? []).map(x => ({ name: x.name, url: x.defaultUrl })), incomplete: !!b.nextPageToken,
  }));
  await check('run_services_london', `https://run.googleapis.com/v2/projects/${project}/locations/europe-west2/services`, b => ({
    services: (b.services ?? []).map(x => ({ name: x.name, url: x.uri, ingress: x.ingress })), incomplete: !!b.nextPageToken,
  }));
  report.limitations = ['Read-only inventory, not deployment acceptance or a full IAM/security audit.',
    'Budget enforcement requires console confirmation; a successful API read does not prove a spending cap.',
    'API denials and incomplete pages remain unresolved, not absence or successful verification.',
    'No user records, password hashes, private keys or raw provider configuration are included.'];
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  if (args.length !== 2 || args[0] !== '--project') throw Error('Usage: node platform/firebase-audit.mjs --project reflective-graph-f1cd4');
  target(args[1]); // Reject other projects before reading credentials.
  const require = createRequire(import.meta.url), auth = require('firebase-tools/lib/auth');
  const account = auth.getProjectDefaultAccount(process.cwd());
  if (!account) throw Error('Existing Firebase CLI login required; no credentials should be pasted into chat');
  auth.setActiveAccount({ project: args[1] }, account);
  const token = await require('firebase-tools/lib/apiv2').getAccessToken();
  const report = await audit(args[1], async url => {
    const response = await fetch(url, { headers: { Authorization: `Bearer ${token}`, 'x-goog-user-project': args[1] }, redirect: 'error', signal: AbortSignal.timeout(20000) });
    return { status: response.status, body: await response.json() };
  });
  console.log(JSON.stringify(report, null, 2));
  if (Object.values(report.checks).some(x => x.status !== 200 || x.result?.incomplete)) process.exitCode = 2;
}
