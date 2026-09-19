import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { identity, emulatorConfig, authorize } from '../identity.mjs';
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
