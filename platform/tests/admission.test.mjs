import test from 'node:test';
import assert from 'node:assert/strict';
import { admitProject, admitJob, admitOutstanding } from '../admission.mjs';

const now = new Date('2026-09-26T12:00:00.000Z');
const job = (overrides = {}) => ({ submitted_by: 'alice', status: 'completed',
  created_at: now.toISOString(), settings: { deadline_ms: 30000 }, ...overrides });
const state = jobs => ({ projects: [], jobs });

test('pilot project and outstanding allowances are per owner, including paused work', () => {
  admitProject({ projects: Array.from({ length: 3 }, () => ({ owner_uid: 'bob' })) }, 'alice');
  assert.throws(() => admitProject({ projects: Array.from({ length: 3 }, () => ({ owner_uid: 'alice' })) }, 'alice'), { code: 'owner_project_limit' });
  const data = state([job({ id: 'a', status: 'paused' }), job({ id: 'b', status: 'queued' }), job({ id: 'c', status: 'interrupted' })]);
  assert.throws(() => admitOutstanding(data, 'alice', 'c'), { code: 'outstanding_job_limit' });
  admitOutstanding(data, 'alice', 'a');
  admitOutstanding(data, 'bob');
});

test('cancelled and failed submissions retain their full daily reservation', () => {
  const data = state([job({ status: 'cancelled' }), job({ status: 'failed' }), job()]);
  admitJob(data, 'alice', { deadline_ms: 30000 }, now);
  data.jobs.push(job({ status: 'timed_out' }));
  assert.throws(() => admitJob(data, 'alice', { deadline_ms: 1 }, now), { code: 'user_daily_limit' });
  admitJob(data, 'bob', { deadline_ms: 30000 }, now);
});

test('global daily and monthly reservations cannot be bypassed with extra accounts', () => {
  const data = state(Array.from({ length: 20 }, (_, i) => job({ submitted_by: `user-${i}` })));
  assert.throws(() => admitJob(data, 'new-user', { deadline_ms: 1 }, now), { code: 'platform_execution_limit' });
  for (const entry of data.jobs) entry.created_at = '2026-09-25T00:00:00.000Z';
  admitJob(data, 'alice', { deadline_ms: 30000 }, now);
  data.jobs = Array.from({ length: 120 }, () => job({ created_at: '2026-09-01T00:00:00.000Z' }));
  assert.throws(() => admitJob(data, 'alice', { deadline_ms: 1 }, now), { code: 'platform_execution_limit' });
  admitJob(data, 'alice', { deadline_ms: 30000 }, new Date('2026-10-01T00:00:00.000Z'));
});

test('admission bounds tiny-job counts and fails closed on invalid reservation records', () => {
  const data = state(Array.from({ length: 20 }, () => job({ settings: { deadline_ms: 1 } })));
  assert.throws(() => admitJob(data, 'alice', { deadline_ms: 1 }, now), { code: 'user_daily_limit' });
  for (const entry of [job({ created_at: 'invalid' }), job({ settings: { deadline_ms: NaN } })])
    assert.throws(() => admitJob(state([entry]), 'alice', { deadline_ms: 1 }, now), /Invalid job/);
  assert.throws(() => admitJob(state([]), 'alice', { deadline_ms: 30001 }, now), /validated/);
});

test('clock rollback does not release future-dated reservations', () => {
  const data = state(Array.from({ length: 4 }, () => job({ created_at: '2026-10-01T00:00:00.000Z' })));
  assert.throws(() => admitJob(data, 'alice', { deadline_ms: 1 }, now), { code: 'user_daily_limit' });
});
