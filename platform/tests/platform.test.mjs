import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import { start } from '../server.mjs';
import { Store, hash, terminal } from '../store.mjs';
import { settings, native } from '../worker.mjs';

const binary = path.resolve('target/release/cli');
const token = 'local_test_token_012345678901234567890';
const program = fs.readFileSync('examples/programs/arithmetic.graph', 'utf8');
const oscillator = fs.readFileSync('examples/behaviour/oscillating.graph', 'utf8');

async function fixture(t, options = {}) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'graph-platform-test-'));
  let app = await start({ directory, binary, token, ...options });
  t.after(async () => { await app.close(); fs.rmSync(directory, { recursive: true, force: true }); });
  const request = async (route, data, { auth = true, headers = {}, raw = false } = {}) => {
    const response = await fetch(app.url + route, { method: data === undefined ? 'GET' : 'POST',
      headers: { ...(auth ? { Authorization: `Bearer ${token}` } : {}), ...(data === undefined ? {} : { 'Content-Type': 'application/json' }), ...headers },
      ...(data === undefined ? {} : { body: JSON.stringify(data) }) });
    return { status: response.status, headers: response.headers, body: raw ? await response.text() : await response.json() };
  };
  const create = async (text = program) => {
    const p = await request('/v1/projects', { name: 'Experiment' }); assert.equal(p.status, 201);
    const v = await request(`/v1/projects/${p.body.id}/versions`, { name: 'v1', source: text }); assert.equal(v.status, 201);
    return { project: p.body, version: v.body };
  };
  const submit = async (p, v, config = {}, key = 'first') => request(`/v1/projects/${p.id}/jobs`, { version_id: v.id, settings: config, idempotency_key: key });
  const wait = async id => {
    for (let i = 0; i < 1500; i++) {
      const r = await request(`/v1/jobs/${id}`);
      if (terminal.has(r.body.status)) return r.body;
      await delay(10);
    }
    throw Error('Job did not finish');
  };
  return { request, create, submit, wait, directory, get app() { return app; },
    restart: async () => { await app.close(); app = await start({ directory, binary, token, ...options }); } };
}

test('named datasets compile to immutable native inputs, retain parents and survive restart', async t => {
  const f = await fixture(t), {project} = await f.create();
  const document = JSON.stringify({schema:'work-items-v1',policy:'guarded_enable',items:[{id:'one',status:'pending'},{id:'two',status:'processed'}]});
  const saved = await f.request(`/v1/projects/${project.id}/datasets`, {name:'tasks',document});
  assert.equal(saved.status,201); assert.equal(saved.body.items,2);
  assert.equal(saved.body.document_sha256,hash(document));
  const fork = await f.request(`/v1/projects/${project.id}/datasets`, {name:'fork',document,parent_dataset_id:saved.body.id});
  assert.equal(fork.status,201); assert.equal(fork.body.parent_dataset_id,saved.body.id);
  const version = await f.request(`/v1/projects/${project.id}/versions`, {name:'compiled',dataset_id:saved.body.id});
  assert.equal(version.status,201); assert.equal(version.body.dataset_id,saved.body.id);
  const compiled = spawnSync(binary,['dataset-compile','-'],{input:document,encoding:'utf8'});
  assert.equal(compiled.status,0,compiled.stderr); assert.equal(hash(compiled.stdout),version.body.source_sha256);
  const submitted = await f.submit(project,version.body), job = await f.wait(submitted.body.id);
  assert.equal(job.outcome,'NormalForm'); assert.equal(job.commits,2);
  const report = await f.request(`/v1/jobs/${job.id}/report`);
  assert.equal(report.status,200); assert.equal(report.body.commits,2); assert.equal(report.body.altered_rule_identities.length,1);
  assert.equal(report.body.final.atoms,report.body.initial.atoms-report.body.consumed_total+report.body.produced_total);
  const page = await f.request(`/v1/jobs/${job.id}/events?limit=1`);
  assert.equal(page.body.total,2); assert.equal(page.body.next_offset,1); assert.equal(page.body.items[0].id,'0');
  assert.equal((await f.request(`/v1/jobs/${job.id}/events?limit=1&offset=1`)).body.items[0].id,'1');
  await f.restart();
  assert.equal((await f.request(`/v1/projects/${project.id}/datasets/${saved.body.id}/document`,undefined,{raw:true})).body,document);
  assert.equal((await f.request(`/v1/projects/${project.id}/datasets`)).body.total,2);
  const other = await f.request('/v1/projects',{name:'other'});
  assert.equal((await f.request(`/v1/projects/${other.body.id}/versions`,{name:'bad',dataset_id:saved.body.id})).status,404);
  assert.equal((await f.request(`/v1/projects/${project.id}/versions`,{name:'bad',source:program,dataset_id:saved.body.id})).status,400);
  for(const bad of ['broken',JSON.stringify({schema:'work-items-v1',policy:'fixed',items:[{id:'same',status:'pending'},{id:'same',status:'pending'}]}),JSON.stringify({schema:'work-items-v1',policy:'fixed',items:[],extra:true})])
    assert.equal((await f.request(`/v1/projects/${project.id}/datasets`,{name:'bad',document:bad})).status,422);
  assert.equal(f.app.store.state.datasets.length,2);
  assert.throws(()=>f.app.store.update(s=>{s.versions.find(v=>v.id===version.body.id).source_sha256=hash('bad');}),/dataset identity/);
});

test('browser access is opt-in, exact-origin and still requires the bearer token', async t => {
  const origin = 'http://127.0.0.1:4173', f = await fixture(t, { origins: [origin] });
  const preflight = await fetch(f.app.url + '/v1/projects', { method: 'OPTIONS', headers: { Origin: origin,
    'Access-Control-Request-Method': 'POST', 'Access-Control-Request-Headers': 'authorization, content-type' } });
  assert.equal(preflight.status, 204); assert.equal(preflight.headers.get('Access-Control-Allow-Origin'), origin);
  assert.equal(preflight.headers.get('Access-Control-Allow-Credentials'), null);
  assert.equal((await f.request('/v1/health', undefined, { auth: false, headers: { Origin: origin } })).status, 401);
  const allowed = await f.request('/v1/health', undefined, { headers: { Origin: origin } });
  assert.equal(allowed.status, 200); assert.equal(allowed.headers.get('Access-Control-Expose-Headers'), 'X-Content-SHA256');
  for (const Origin of ['null', 'http://127.0.0.1:4174', 'https://evil.invalid'])
    assert.equal((await f.request('/v1/health', undefined, { headers: { Origin } })).status, 403);
  const denied = await fetch(f.app.url + '/v1/projects', { method: 'OPTIONS', headers: { Origin: origin,
    'Access-Control-Request-Method': 'POST', 'Access-Control-Request-Headers': 'x-unapproved' } });
  assert.equal(denied.status, 403);
  await assert.rejects(start({ directory: f.directory, binary, token, origins: ['*'] }), /Browser origins/);
  await assert.rejects(start({ directory: f.directory, binary, token, origins: ['https://example.com'] }), /Browser origins/);
});

test('fork and retry provenance is project-scoped, immutable, durable and included in idempotency', async t => {
  const f = await fixture(t), { project, version } = await f.create();
  const fork = await f.request(`/v1/projects/${project.id}/versions`, { name: 'fork', source: program, parent_version_id: version.id });
  assert.equal(fork.status, 201); assert.equal(fork.body.parent_version_id, version.id); assert.notEqual(fork.body.id, version.id);
  const original = await f.submit(project, version); const first = await f.wait(original.body.id);
  const input = { version_id: fork.body.id, settings: { max_steps: 0 }, idempotency_key: 'branch', parent_job_id: first.id };
  const branch = await f.request(`/v1/projects/${project.id}/jobs`, input); assert.equal(branch.status, 202);
  const result = await f.wait(branch.body.id); assert.equal(result.parent_job_id, first.id); assert.equal(result.commits, 0);
  assert.equal((await f.request(`/v1/projects/${project.id}/jobs`, { ...input, parent_job_id: null })).status, 409);
  const other = await f.request('/v1/projects', { name: 'Other' });
  assert.equal((await f.request(`/v1/projects/${other.body.id}/versions`, { name: 'bad', source: program, parent_version_id: version.id })).status, 404);
  assert.equal((await f.request(`/v1/projects/${project.id}/jobs`, { ...input, parent_job_id: other.body.id, idempotency_key: 'bad' })).status, 409);
  await f.restart();
  assert.equal((await f.request(`/v1/projects/${project.id}/jobs`, input)).body.id, result.id);
  assert.deepEqual((await f.request(`/v1/jobs/${first.id}`)).body, first);
  assert.throws(() => f.app.store.update(s => { s.versions[0].parent_version_id = fork.body.id; }), /version parent/);
  assert.throws(() => f.app.store.update(s => { s.jobs[0].parent_job_id = result.id; }), /job parent/);
});

test('real native job exports a verified trace with pinned identities and recovers after restart', async t => {
  const f = await fixture(t), { project, version } = await f.create();
  const submitted = await f.submit(project, version, { max_steps: 1 }); assert.equal(submitted.status, 202);
  const j = await f.wait(submitted.body.id);
  assert.equal(j.status, 'completed'); assert.equal(j.outcome, 'NormalForm'); assert.equal(j.commits, 1);
  assert.equal(j.engine_sha256, hash(fs.readFileSync(binary))); assert.equal(j.source_sha256, hash(program));
  const trace = await f.request(`/v1/jobs/${j.id}/trace`, undefined, { raw: true });
  assert.equal(hash(trace.body), j.trace_sha256);
  const checked = spawnSync(binary, ['trace-check', '-'], { input: trace.body, encoding: 'utf8' }); assert.equal(checked.status, 0, checked.stderr);
  const direct = spawnSync(binary, ['export', '-', '--max-steps', '1'], { input: program, encoding: 'utf8' });
  assert.deepEqual(JSON.parse(trace.body), JSON.parse(direct.stdout));
  const saved = await f.request(`/v1/projects/${project.id}/versions/${version.id}/source`, undefined, { raw: true }); assert.equal(saved.body, program);
  await f.restart();
  assert.deepEqual((await f.request(`/v1/jobs/${j.id}`)).body, j);
  assert.equal((await f.request(`/v1/jobs/${j.id}/trace`, undefined, { raw: true })).body, trace.body);
});

test('idempotent submission is durable; altered request and cross-project version are rejected', async t => {
  const f = await fixture(t), { project, version } = await f.create();
  const requests = await Promise.all(Array.from({ length: 5 }, () => f.submit(project, version)));
  assert.equal(new Set(requests.map(r => r.body.id)).size, 1);
  assert.equal(requests.filter(r => r.status === 202).length, 1);
  assert.equal((await f.submit(project, version, { max_steps: 0 })).status, 409);
  await f.wait(requests[0].body.id); await f.restart();
  assert.equal((await f.submit(project, version)).body.id, requests[0].body.id);
  const other = await f.request('/v1/projects', { name: 'Other' });
  assert.equal((await f.submit(other.body, version)).status, 404);
});

test('invalid source, settings, unknown fields, unauthorised and cross-origin requests are rejected', async t => {
  const f = await fixture(t);
  assert.equal((await f.request('/v1/health', undefined, { auth: false })).status, 401);
  assert.equal((await f.request('/v1/health', undefined, { headers: { Origin: 'https://example.invalid' } })).status, 403);
  assert.equal((await f.request('/v1/projects', { name: 'test', surprise: true })).status, 400);
  assert.equal((await f.request('/v1/validate', { source: 'broken' })).status, 422);
  assert.equal((await f.request('/v1/validate', { source: '\ud800' })).status, 400);
  assert.equal((await f.request('/v1/validate', { source: 'x'.repeat(1024 * 1024) })).status, 413);
  const { project, version } = await f.create();
  for (const config of [{ max_steps: -1 }, { max_steps: 1001 }, { max_work: 1000001 }, { seed: 42 }, { seed: '18446744073709551616' }, { detect_cycles: 'yes' }, { arbitrary: true }])
    assert.equal((await f.submit(project, version, config)).status, 400);
  assert.equal((await f.request('/v1/projects?limit=101')).status, 400);
  assert.equal((await f.request('/v1/projects?surprise=yes')).status, 400);
  assert.equal((await f.request('/v1/projects?limit=1')).body.items.length, 1);
});

test('engine limits and recurrence remain semantic outcomes, not platform failures', async t => {
  const f = await fixture(t), { project, version } = await f.create(oscillator);
  for (const [config, outcome] of [[{ max_work: 0 }, 'ResourceLimit'], [{ max_steps: 0 }, 'StepLimit'], [{ detect_cycles: true, seed: '42' }, 'CycleDetected']]) {
    const r = await f.submit(project, version, config, outcome), j = await f.wait(r.body.id);
    assert.equal(j.status, 'completed'); assert.equal(j.outcome, outcome); assert.ok(j.trace_sha256);
    if (outcome !== 'CycleDetected') assert.equal(j.commits, 0);
  }
});

test('reflective programs and seeded execution use the exact native trace path', async t => {
  const f = await fixture(t);
  const text = fs.readFileSync('examples/programs/self-modify.graph', 'utf8');
  const { project, version } = await f.create(text);
  const r = await f.submit(project, version, { max_steps: 3, seed: '42' });
  const j = await f.wait(r.body.id); assert.equal(j.status, 'completed');
  const exported = await f.request(`/v1/jobs/${j.id}/trace`);
  const direct = spawnSync(binary, ['export', '-', '--max-steps', '3', '--strategy', 'seeded', '--seed', '42'], { input: text, encoding: 'utf8' });
  assert.deepEqual(exported.body, JSON.parse(direct.stdout));
});

test('client example produces independently checkable evidence without overwriting output', async t => {
  const f = await fixture(t);
  const output = path.join(f.directory, 'client-trace.json');
  const invoke = () => new Promise((yes, no) => {
    const child = spawn(process.execPath, ['platform/client-example.mjs', 'examples/programs/arithmetic.graph', output], {
      env: { ...process.env, GRAPH_API_URL: f.app.url, GRAPH_API_TOKEN: token }, stdio: ['ignore', 'pipe', 'pipe'] });
    child.on('error', no); child.on('close', yes);
  });
  assert.equal(await invoke(), 0);
  const bytes = fs.readFileSync(output);
  assert.equal(spawnSync(binary, ['trace-check', output]).status, 0);
  assert.notEqual(await invoke(), 0);
  assert.deepEqual(fs.readFileSync(output), bytes);
});

test('deadline and cancellation do not publish a partial trace or normal-form claim', async t => {
  const f = await fixture(t), { project, version } = await f.create(oscillator);
  const r = await f.submit(project, version, { max_steps: 1000, deadline_ms: 1 }, 'timeout');
  const timed = await f.wait(r.body.id); assert.equal(timed.status, 'timed_out'); assert.equal(timed.outcome, null); assert.equal(timed.trace_sha256, null);
  const running = await f.submit(project, version, { max_steps: 1000 }, 'running');
  const queued = await f.submit(project, version, { max_steps: 1000 }, 'queued');
  await f.request(`/v1/jobs/${queued.body.id}/cancel`, {});
  assert.equal((await f.wait(queued.body.id)).status, 'cancelled');
  await f.request(`/v1/jobs/${running.body.id}/cancel`, {});
  const cancelled = await f.wait(running.body.id); assert.equal(cancelled.status, 'cancelled'); assert.equal(cancelled.trace_sha256, null);
  assert.equal((await f.request(`/v1/jobs/${cancelled.id}/trace`)).status, 409);
});

test('only one store owner can run; corrupted artefacts fail closed', async t => {
  const f = await fixture(t), { version } = await f.create();
  await assert.rejects(start({ directory: f.directory, binary, token }), /EEXIST/);
  await assert.rejects(start({ directory: f.directory, binary, token, recover: true }), /still alive/);
  fs.writeFileSync(path.join(f.directory, 'blobs', version.source_sha256), 'corrupt');
  assert.throws(() => f.app.store.read(version.source_sha256), /checksum/);
});

test('restart marks abandoned execution interrupted and never fabricates a result', async t => {
  const f = await fixture(t), { project, version } = await f.create();
  const r = await f.submit(project, version); await f.wait(r.body.id);
  // Inject the exact persisted crash boundary before publication, not a claimed process-kill test.
  f.app.store.update(s => Object.assign(s.jobs[0], { status: 'running', trace_sha256: null, outcome: null, commits: null }));
  await f.restart();
  const recovered = (await f.request(`/v1/jobs/${r.body.id}`)).body;
  assert.equal(recovered.status, 'interrupted'); assert.equal(recovered.outcome, null); assert.equal(recovered.trace_sha256, null);
  assert.equal((await f.submit(project, version)).body.id, recovered.id);
  const retry = await f.submit(project, version, {}, 'explicit-retry');
  assert.notEqual(retry.body.id, recovered.id); assert.equal((await f.wait(retry.body.id)).outcome, 'NormalForm');
});

test('subprocess output limit and timeout are separate from engine outcomes', async () => {
  const overflow = await native(process.execPath, ['-e', 'process.stdout.write("x".repeat(10000))'], '', { maximum: 10 });
  assert.equal(overflow.cause, 'output_limit');
  const timeout = await native(process.execPath, ['-e', 'setTimeout(()=>{},10000)'], '', { deadline: 10 });
  assert.equal(timeout.cause, 'timed_out');
  assert.deepEqual(settings({ seed: '0' }).seed, '0');
});

test('real server crash requires explicit dead-owner recovery; queued jobs survive', { timeout: 15000 }, async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'graph-platform-crash-'));
  const launch = recover => spawn(process.execPath, ['platform/server.mjs', ...(recover ? ['--recover'] : [])], {
    env: { ...process.env, GRAPH_STORE: directory, GRAPH_ENGINE: binary, GRAPH_API_TOKEN: token, GRAPH_PORT: '0' },
    stdio: ['ignore', 'pipe', 'pipe'] });
  let child = launch(false);
  const ready = process => new Promise((yes, no) => {
    let errors = '';
    process.stderr.on('data', b => { errors += b; });
    process.once('exit', () => no(Error(`Server exited before readiness: ${errors}`)));
    process.once('error', no);
    process.stdout.once('data', b => { try { yes(JSON.parse(b)); } catch (e) { no(e); } });
  });
  t.after(async () => { if (child.exitCode === null && child.signalCode === null) { child.kill('SIGTERM'); await once(child, 'exit'); } fs.rmSync(directory, { recursive: true, force: true }); });
  const first = await ready(child);
  const call = async (url, route, data) => { const r = await fetch(url + route, { method: data ? 'POST' : 'GET', headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, ...(data ? { body: JSON.stringify(data) } : {}) }); return r.json(); };
  const p = await call(first.url, '/v1/projects', { name: 'Crash' });
  const v = await call(first.url, `/v1/projects/${p.id}/versions`, { name: 'oscillator', source: oscillator });
  const a = await call(first.url, `/v1/projects/${p.id}/jobs`, { version_id: v.id, settings: { max_steps: 1000 }, idempotency_key: 'active' });
  const b = await call(first.url, `/v1/projects/${p.id}/jobs`, { version_id: v.id, settings: { max_steps: 0 }, idempotency_key: 'queued' });
  child.kill('SIGKILL'); await once(child, 'exit');
  child = launch(true); const second = await ready(child);
  assert.equal((await call(second.url, `/v1/jobs/${a.id}`)).status, 'interrupted');
  let recovered;
  for (let i = 0; i < 500; i++) { recovered = await call(second.url, `/v1/jobs/${b.id}`); if (terminal.has(recovered.status)) break; await delay(10); }
  assert.equal(recovered.status, 'completed'); assert.equal(recovered.outcome, 'StepLimit');
});

test('invalid metadata does not get silently reset', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'graph-platform-corrupt-'));
  try {
    fs.writeFileSync(path.join(directory, 'metadata.json'), '{broken');
    assert.throws(() => new Store(directory));
    assert.equal(fs.readFileSync(path.join(directory, 'metadata.json'), 'utf8'), '{broken');
    assert.equal(fs.existsSync(path.join(directory, 'owner.lock')), false);
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});
