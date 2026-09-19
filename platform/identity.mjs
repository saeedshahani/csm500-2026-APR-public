// Emulator identities are a development boundary, not production credentials.
import { randomUUID, timingSafeEqual } from 'node:crypto';
import { fail } from './store.mjs';

export const validUid = uid => typeof uid === 'string' && uid.length > 0 && uid.length <= 128 && !/[\x00-\x1f\x7f]/.test(uid);
export function emulatorConfig(projectId, host) {
  if (!/^demo-[a-z0-9-]{1,24}$/.test(projectId ?? '') || !/^127\.0\.0\.1:\d+$/.test(host ?? '')
      || Number(host.split(':')[1]) < 1 || Number(host.split(':')[1]) > 65535)
    throw Error('Emulator authentication requires a demo- project and 127.0.0.1:port Auth host');
  return { projectId, host };
}
export async function identity({ mode = 'local-single-owner', token, projectId, host } = {}) {
  if (mode === 'local-single-owner') {
    if (typeof token !== 'string' || !/^[A-Za-z0-9_-]{32,128}$/.test(token)) throw Error('A 32–128 character local bearer token is required');
    const expected = Buffer.from(`Bearer ${token}`);
    return { mode, namespace: 'local-single-owner', close: async () => {}, async authenticate(header) {
      const supplied = Buffer.from(header ?? '');
      if (supplied.length !== expected.length || !timingSafeEqual(supplied, expected)) fail(401, 'unauthorized', 'Local bearer token required');
      return 'local-owner';
    } };
  }
  if (mode !== 'firebase-emulator') throw Error('Unsupported authentication mode');
  emulatorConfig(projectId, host);
  // Admin reads this process-wide setting. Refuse mismatches rather than silently redirecting it.
  if (process.env.FIREBASE_AUTH_EMULATOR_HOST !== host) throw Error('FIREBASE_AUTH_EMULATOR_HOST must match the configured emulator host');
  const { initializeApp, deleteApp } = await import('firebase-admin/app');
  const { getAuth } = await import('firebase-admin/auth');
  const app = initializeApp({ projectId }, `graph-emulator-${randomUUID()}`), auth = getAuth(app);
  return { mode, projectId, namespace: `firebase-emulator:${projectId}`, close: () => deleteApp(app), async authenticate(header) {
    if (typeof header !== 'string' || !/^Bearer [A-Za-z0-9_.-]{1,16384}$/.test(header)) fail(401, 'unauthorized', 'Firebase emulator ID token required');
    try {
      // Revocation/disabled-account checking contacts the emulator; an outage fails closed.
      const decoded = await auth.verifyIdToken(header.slice(7), true);
      if (!validUid(decoded.uid)) throw Error('Invalid UID');
      return decoded.uid;
    } catch { fail(401, 'unauthorized', 'Invalid, revoked or unavailable Firebase emulator identity'); }
  } };
}

export function role(project, uid, mode) {
  if (mode === 'local-single-owner') return 'owner';
  if (project.owner_uid === uid) return 'owner';
  return project.members.find(member => member.uid === uid)?.role ?? null;
}
export function authorize(project, uid, mode, required = 'viewer') {
  const actual = role(project, uid, mode);
  if (!actual) fail(404, 'not_found', 'Project not found');
  if ((required === 'owner' && actual !== 'owner') || (required === 'editor' && actual === 'viewer'))
    fail(403, 'forbidden', `Project ${required} role required`);
  return project;
}
