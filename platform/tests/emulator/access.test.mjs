import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { setTimeout as delay } from 'node:timers/promises';
import { initializeApp, deleteApp } from 'firebase-admin/app';
import { getAuth } from 'firebase-admin/auth';
import { start } from '../../server.mjs';
import { terminal, hash } from '../../store.mjs';
import { emulatorConfig } from '../../identity.mjs';

const projectId = 'demo-reflective-graph', authHost = process.env.FIREBASE_AUTH_EMULATOR_HOST;
emulatorConfig(projectId, authHost); // Direct execution without the emulator is an error, not a skipped test.
const binary = path.resolve('target/release/cli'), source = fs.readFileSync('examples/programs/arithmetic.graph', 'utf8');
async function fixture(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'graph-emulator-access-'));
  const options = { directory, binary, authMode: 'firebase-emulator', projectId, authHost };
  let app = await start(options);
  const admin = initializeApp({ projectId }, randomUUID()), auth = getAuth(admin), users = [];
  t.after(async () => { await app.close(); await Promise.all(users.map(uid => auth.deleteUser(uid))); await deleteApp(admin); fs.rmSync(directory, { recursive: true, force: true }); });
  async function user() {
    const result = await fetch(`http://${authHost}/identitytoolkit.googleapis.com/v1/accounts:signUp?key=emulator-only`, {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ email: `${randomUUID()}@example.test`, password: 'test-password-only', returnSecureToken: true }) });
    assert.equal(result.status, 200); const account = await result.json(); users.push(account.localId);
    return { uid: account.localId, token: account.idToken };
  }
  async function request(user, route, data, raw = false) {
    const response = await fetch(app.url + route, { method: data === undefined ? 'GET' : 'POST', headers: {
      Authorization: `Bearer ${user.token}`, ...(data === undefined ? {} : { 'Content-Type': 'application/json' }) },
    ...(data === undefined ? {} : { body: JSON.stringify(data) }) });
    return { status: response.status, headers: response.headers, body: raw ? await response.text() : await response.json() };
  }
  return { user, request, auth, get app() { return app; }, async restart() { await app.close(); app = await start(options); } };
}

test('real emulator users: project isolation, role matrix, immutable versions and native verified artefacts', async t => {
  const f = await fixture(t), owner = await f.user(), editor = await f.user(), viewer = await f.user(), stranger = await f.user();
  const health = await f.request(owner, '/v1/health'); assert.equal(health.body.uid, owner.uid);
  const p = (await f.request(owner, '/v1/projects', { name: 'Shared experiment' })).body;
  assert.equal(p.owner_uid, owner.uid); assert.equal(p.role, 'owner');
  const base = `/v1/projects/${p.id}`;
  assert.equal((await f.request(stranger, '/v1/projects')).body.total, 0);
  assert.equal((await f.request(stranger, base + '/versions')).status, 404);
  for (const [account, role] of [[editor, 'editor'], [viewer, 'viewer']])
    assert.equal((await f.request(owner, base + '/members', { uid: account.uid, role })).status, 200);
  assert.equal((await f.request(viewer, '/v1/projects')).body.items[0].role, 'viewer');
  for (const account of [editor, viewer]) {
    assert.equal((await f.request(account, base + '/members')).status, 403);
    assert.equal((await f.request(account, base + '/members', { uid: stranger.uid, role: 'editor' })).status, 403);
  }
  assert.equal((await f.request(owner, base + '/members', { uid: owner.uid, role: 'viewer' })).status, 400);
  assert.equal((await f.request(owner, base + '/members', { uid: stranger.uid, role: 'owner' })).status, 400);
  const document=JSON.stringify({schema:'work-items-v1',policy:'fixed',items:[]});
  assert.equal((await f.request(viewer,base+'/datasets',{name:'denied',document})).status,403);
  const dataset=await f.request(editor,base+'/datasets',{name:'allowed',document});assert.equal(dataset.status,201);
  assert.equal((await f.request(viewer,base+`/datasets/${dataset.body.id}/document`,undefined,true)).body,document);
  assert.equal((await f.request(stranger,base+`/datasets/${dataset.body.id}/document`)).status,404);
  assert.equal((await f.request(viewer,base+'/versions',{name:'denied',dataset_id:dataset.body.id})).status,403);
  assert.equal((await f.request(viewer, base + '/versions', { name: 'Denied', source })).status, 403);
  const saved = await f.request(editor, base + '/versions', { name: 'v1', source }); assert.equal(saved.status, 201);
  const v = saved.body, submission = { version_id: v.id, idempotency_key: 'same-key', settings: { max_steps: 100 } };
  assert.equal((await f.request(viewer, base + '/jobs', submission)).status, 403);
  const job = (await f.request(editor, base + '/jobs', submission)).body;
  assert.equal(job.submitted_by, editor.uid);
  assert.equal((await f.request(editor, base + '/jobs', submission)).body.id, job.id);
  const ownerJob = (await f.request(owner, base + '/jobs', submission)).body; assert.notEqual(ownerJob.id, job.id);
  assert.equal((await f.request(viewer, `/v1/jobs/${job.id}/cancel`, {})).status, 403);
  for (const route of [`${base}/versions/${v.id}/source`, base + '/jobs', `/v1/jobs/${job.id}`, `/v1/jobs/${job.id}/trace`])
    assert.equal((await f.request(stranger, route)).status, 404);
  assert.equal((await f.request(stranger, `/v1/jobs/${job.id}/cancel`, {})).status, 404);
  let done;
  for (let i = 0; i < 500; i++) { done = (await f.request(viewer, `/v1/jobs/${job.id}`)).body; if (terminal.has(done.status)) break; await delay(10); }
  assert.equal(done.status, 'completed'); assert.equal(done.outcome, 'NormalForm');
  for(const suffix of ['report','events']){
    assert.equal((await f.request(viewer,`/v1/jobs/${job.id}/${suffix}`)).status,200);
    assert.equal((await f.request(stranger,`/v1/jobs/${job.id}/${suffix}`)).status,404);
  }
  const trace = await f.request(viewer, `/v1/jobs/${job.id}/trace`, undefined, true);
  assert.equal(hash(trace.body), done.trace_sha256); assert.equal(trace.headers.get('X-Content-SHA256'), done.trace_sha256);
  assert.equal((await f.request(viewer, `${base}/versions/${v.id}/source`, undefined, true)).body, source);
  await f.restart();
  assert.equal((await f.request(viewer, `/v1/jobs/${job.id}/trace`, undefined, true)).body, trace.body);
  assert.equal((await f.request(editor, base + '/jobs', submission)).body.id, job.id);
  assert.equal((await f.request(owner, base + '/members', { uid: editor.uid, role: null })).status, 200);
  assert.equal((await f.request(editor, '/v1/projects')).body.total, 0);
  assert.equal((await f.request(editor, `/v1/jobs/${job.id}`)).status, 404);
});

test('permission revocation during native validation prevents publication', async t => {
  for (const [method, route, input] of [
    ['validate', 'versions', { name: 'Revoked source', source }],
    ['compileDataset', 'datasets', { name: 'Revoked data', document: JSON.stringify({ schema: 'work-items-v1', policy: 'fixed', items: [] }) }]
  ]) {
  const f = await fixture(t), owner = await f.user(), editor = await f.user();
  const p = (await f.request(owner, '/v1/projects', { name: 'Race' })).body, base = `/v1/projects/${p.id}`;
  await f.request(owner, base + '/members', { uid: editor.uid, role: 'editor' });
  let release, entered;
  const waiting = new Promise(resolve => { entered = resolve; }), gate = new Promise(resolve => { release = resolve; });
  const original = f.app.worker[method].bind(f.app.worker);
  f.app.worker[method] = async text => { const value = await original(text); entered(); await gate; return value; };
  const pending = f.request(editor, base + '/' + route, input);
  try { await waiting; await f.request(owner, base + '/members', { uid: editor.uid, role: null }); }
  finally { release(); }
  assert.equal((await pending).status, 404);
  assert.equal((await f.request(owner, base + '/' + route)).body.total, 0);
  assert.equal(fs.readdirSync(f.app.store.blobs).length, 0);
  }
});

test('missing, wrong-audience, expired, disabled and revoked identities fail closed', async t => {
  const f = await fixture(t), account = await f.user();
  for (const token of ['', 'local_token_01234567890123456789012', 'not.a.token'])
    assert.equal((await f.request({ token }, '/v1/health')).status, 401);
  const parts = account.token.split('.'), payload = JSON.parse(Buffer.from(parts[1], 'base64url'));
  for (const changes of [{ aud: 'demo-wrong-project' }, { exp: 1 }]) {
    const token = `${parts[0]}.${Buffer.from(JSON.stringify({ ...payload, ...changes })).toString('base64url')}.`;
    assert.equal((await f.request({ token }, '/v1/health')).status, 401);
  }
  await f.auth.updateUser(account.uid, { disabled: true });
  assert.equal((await f.request(account, '/v1/health')).status, 401);
  await f.auth.updateUser(account.uid, { disabled: false });
  // Advance beyond the token's whole-second auth_time before revocation.
  await delay(1100); await f.auth.revokeRefreshTokens(account.uid);
  assert.equal((await f.request(account, '/v1/health')).status, 401);
});
