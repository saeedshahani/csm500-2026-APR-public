import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { identity, emulatorConfig, authorize, productionConfig, firebaseAuthenticator } from '../identity.mjs';
import { Store } from '../store.mjs';

test('identity configuration is explicit, demo-only and loopback-only', async () => {
  for (const [project, host] of [['real-project', '127.0.0.1:9099'], ['demo-test', 'example.com:9099'],
    ['demo-test', 'http://127.0.0.1:9099'], ['demo-test', '127.0.0.1:0'], ['demo-test', '127.0.0.1:65536']])
    assert.throws(() => emulatorConfig(project, host));
  assert.deepEqual(emulatorConfig('demo-test', '127.0.0.1:9099'), { projectId: 'demo-test', host: '127.0.0.1:9099' });
  await assert.rejects(identity({ mode: 'production' }), /Unsupported/);
  const auth = await identity({ token: 'local_token_01234567890123456789012' });
  assert.equal(await auth.authenticate('Bearer local_token_01234567890123456789012'), 'local-owner');
  await assert.rejects(auth.authenticate('Bearer invalid'), { status: 401 });
});

test('role matrix separates visibility, writing and membership administration', () => {
  const p = { owner_uid: 'owner', members: [{ uid: 'editor', role: 'editor' }, { uid: 'viewer', role: 'viewer' }] };
  for (const [uid, permission, status] of [['owner', 'owner', 200], ['editor', 'editor', 200],
    ['viewer', 'viewer', 200], ['viewer', 'editor', 403], ['editor', 'owner', 403], ['stranger', 'viewer', 404]]) {
    if (status === 200) assert.equal(authorize(p, uid, 'firebase-emulator', permission), p);
    else assert.throws(() => authorize(p, uid, 'firebase-emulator', permission), { status });
  }
});

test('production identity pins its project and refuses emulator or conflicting environment', () => {
  const project = 'reflective-graph-f1cd4';
  assert.equal(productionConfig(project, {}), project);
  for (const value of [undefined, '', 'demo-test', 'other-project']) assert.throws(() => productionConfig(value, {}));
  for (const key of ['FIREBASE_AUTH_EMULATOR_HOST', 'FIRESTORE_EMULATOR_HOST', 'FIREBASE_STORAGE_EMULATOR_HOST'])
    for (const value of ['', '127.0.0.1:9099']) assert.throws(() => productionConfig(project, { [key]: value }), /emulator/);
  for (const key of ['GOOGLE_CLOUD_PROJECT', 'GCLOUD_PROJECT', 'GCP_PROJECT']) {
    assert.throws(() => productionConfig(project, { [key]: 'other-project' }), /Conflicting/);
    assert.equal(productionConfig(project, { [key]: project }), project);
  }
});

test('production authentication requires revoked-token checking and verified password accounts', async () => {
  const projectId = 'reflective-graph-f1cd4';
  const claims = { uid: 'alice', sub: 'alice', aud: projectId, iss: `https://securetoken.google.com/${projectId}`,
    email: 'alice@example.test', email_verified: true, firebase: { sign_in_provider: 'password' } };
  let decoded = claims, calls = 0, unavailable = false;
  const authenticate = firebaseAuthenticator({ async verifyIdToken(token, revoked) {
    calls++; assert.equal(token, 'signed.token'); assert.equal(revoked, true);
    if (unavailable) throw Error('Unavailable or revoked');
    return decoded;
  } }, { projectId, production: true });
  for (const header of [undefined, '', 'Basic signed.token', 'Bearer a b', `Bearer ${'x'.repeat(16385)}`])
    await assert.rejects(authenticate(header), { status: 401 });
  assert.equal(calls, 0);
  assert.equal(await authenticate('Bearer signed.token'), 'alice');
  for (const change of [{ uid: '' }, { sub: 'bob' }, { aud: 'other' }, { iss: 'other' }]) {
    decoded = { ...claims, ...change };
    await assert.rejects(authenticate('Bearer signed.token'), { status: 401 });
  }
  for (const change of [{ email_verified: false }, { email_verified: 'true' }, { email: null },
    { firebase: { sign_in_provider: 'anonymous' } }, { firebase: { sign_in_provider: 'google.com' } }, { firebase: undefined }]) {
    decoded = { ...claims, ...change };
    await assert.rejects(authenticate('Bearer signed.token'), { status: 403, code: 'verified_email_required' });
  }
  unavailable = true;
  await assert.rejects(authenticate('Bearer signed.token'), { status: 401 });
});

test('production stores require explicit ownership and cannot be opened as emulator stores', t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'graph-production-identity-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const namespace = 'firebase-production:reflective-graph-f1cd4';
  const store = new Store(directory, { namespace });
  try {
    assert.throws(() => store.update(s => s.projects.push({ id: randomUUID(), name: 'Missing owner' })), /access/);
    store.update(s => s.projects.push({ id: randomUUID(), name: 'Owned', owner_uid: 'alice', members: [] }));
    assert.equal(authorize(store.state.projects[0], 'alice', 'firebase-production', 'owner').name, 'Owned');
    assert.throws(() => authorize(store.state.projects[0], 'bob', 'firebase-production'), { status: 404 });
  } finally { store.close(); }
  assert.throws(() => new Store(directory, { namespace: 'firebase-emulator:demo-test' }), /namespace mismatch/);
});

test('identity namespaces cannot reinterpret local projects or another emulator project', t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'graph-access-test-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  let store = new Store(directory);
  store.update(s => s.projects.push({ id: randomUUID(), name: 'Legacy', created_at: new Date().toISOString() }));
  store.close();
  assert.throws(() => new Store(directory, { namespace: 'firebase-emulator:demo-test' }), /namespace mismatch/);
  store = new Store(directory); assert.equal(store.state.projects.length, 1); store.close();
  const shared = path.join(directory, 'shared');
  store = new Store(shared, { namespace: 'firebase-emulator:demo-test' });
  const p = { id: randomUUID(), name: 'Shared', created_at: new Date().toISOString(), owner_uid: 'owner', members: [] };
  store.update(s => s.projects.push(p));
  for (const members of [[{ uid: 'owner', role: 'viewer' }], [{ uid: 'x', role: 'owner' }],
    [{ uid: 'x', role: 'viewer' }, { uid: 'x', role: 'editor' }]])
    assert.throws(() => store.update(s => { s.projects[0].members = members; }), /project access/);
  store.close();
  assert.throws(() => new Store(shared), /namespace mismatch/);
  assert.throws(() => new Store(shared, { namespace: 'firebase-emulator:demo-other' }), /namespace mismatch/);
});
