import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { hash, fail, terminal } from './store.mjs';

export const DEFAULTS = Object.freeze({ max_steps: 100, max_work: 1000000, max_candidates: 1000,
  seed: null, detect_cycles: false, deadline_ms: 10000 });
export function settings(value = {}) {
  if (!value || typeof value !== 'object' || Array.isArray(value)
      || Object.keys(value).some(k => !Object.hasOwn(DEFAULTS, k) && k !== 'execution_profile')) fail(400, 'invalid_settings', 'Unknown settings');
  const s = { ...DEFAULTS, ...value };
  if (Object.hasOwn(s, 'execution_profile') && !['reference-v1', 'indexed-v2', 'framed-v3'].includes(s.execution_profile))
    fail(400, 'invalid_settings', 'Unsupported execution profile');
  // Preserve the canonical pre-profile default settings and idempotency hashes.
  if (s.execution_profile === 'reference-v1') delete s.execution_profile;
  for (const [key, maximum] of Object.entries({ max_steps: 1000, max_work: 1000000, max_candidates: 1000, deadline_ms: 30000 })) {
    if (!Number.isSafeInteger(s[key]) || s[key] < (key === 'deadline_ms' ? 1 : 0) || s[key] > maximum)
      fail(400, 'invalid_settings', `Invalid ${key}; maximum ${maximum}`);
  }
  if (typeof s.detect_cycles !== 'boolean' || !(s.seed === null || (typeof s.seed === 'string'
      && /^(0|[1-9][0-9]{0,19})$/.test(s.seed) && BigInt(s.seed) <= 18446744073709551615n)))
    fail(400, 'invalid_settings', 'Invalid cycle flag or decimal u64 seed');
  return s;
}

export function processLimits(env = process.env) {
  if (env.GRAPH_NATIVE_MEMORY_BYTES === undefined && env.GRAPH_NATIVE_CPU_SECONDS === undefined) return null;
  const memory = Number(env.GRAPH_NATIVE_MEMORY_BYTES), cpu = Number(env.GRAPH_NATIVE_CPU_SECONDS);
  if (process.platform !== 'linux' || !Number.isSafeInteger(memory) || memory < 64 * 1024 * 1024 || memory > 512 * 1024 * 1024
      || !Number.isSafeInteger(cpu) || cpu < 1 || cpu > 30) throw Error('Native process limits require Linux, 64–512 MiB address space and 1–30 CPU seconds');
  return { memory, cpu };
}

export function native(binary, args, input, { signal, deadline = 10000, maximum = 16 * 1024 * 1024, limits = null } = {}) {
  return new Promise((resolve, reject) => {
    const command = limits ? '/usr/bin/prlimit' : binary;
    const argv = limits ? [`--as=${limits.memory}`, `--cpu=${limits.cpu}`, '--', binary, ...args] : args;
    const child = spawn(command, argv, { stdio: ['pipe', 'pipe', 'pipe'], shell: false });
    let cause = null, size = 0, errors = 0;
    const chunks = [], messages = [];
    const stop = why => { cause ??= why; child.kill('SIGKILL'); };
    const cancelled = () => stop(signal.reason === 'interrupted' ? 'interrupted' : 'cancelled');
    signal?.addEventListener('abort', cancelled, { once: true });
    if (signal?.aborted) cancelled();
    const timer = setTimeout(() => stop('timed_out'), deadline);
    child.stdout.on('data', b => { size += b.length; if (size > maximum) stop('output_limit'); else chunks.push(b); });
    child.stderr.on('data', b => { errors += b.length; if (errors <= 65536) messages.push(b); });
    child.stdin.on('error', e => { if (e.code !== 'EPIPE') stop('input_failure'); });
    child.on('error', reject);
    child.on('close', (code, exitSignal) => {
      clearTimeout(timer); signal?.removeEventListener('abort', cancelled);
      resolve({ code, signal: exitSignal, cause, stdout: Buffer.concat(chunks), stderr: Buffer.concat(messages).toString('utf8') });
    });
    child.stdin.end(input);
  });
}

export class Worker {
  constructor(store, binary) {
    this.store = store; this.binary = binary; this.identity = hash(readFileSync(binary));
    this.limits = processLimits();
    this.active = null; this.stopping = false;
    for (const j of store.state.jobs) settings(j.settings);
  }
  assertBinary() {
    if (hash(readFileSync(this.binary)) !== this.identity) throw Error('Native binary changed; restart after rebuilding');
  }
  async validate(source) {
    this.assertBinary();
    const r = await native(this.binary, ['check', '-'], source, { maximum: 4 * 1024 * 1024, limits: this.limits });
    if (r.cause) fail(422, r.cause, 'Validation did not complete');
    if (r.code !== 0) fail(422, r.code === 3 ? 'resource_limit' : 'invalid_program', r.stderr.slice(0, 8192));
    return { valid: true, source_sha256: hash(source), engine_sha256: this.identity };
  }
  async compileDataset(document) {
    this.assertBinary();
    const r = await native(this.binary, ['dataset-compile', '-'], document, { maximum: 256 * 1024, limits: this.limits });
    if (r.cause) fail(422, r.cause, 'Dataset compilation did not complete');
    if (r.code !== 0) fail(422, 'invalid_dataset', r.stderr.slice(0, 8192));
    return { source: r.stdout, engine_sha256: this.identity };
  }
  kick() {
    if (this.active || this.stopping || this.canDispatch?.() === false) return;
    const job = this.store.state.jobs.find(j => j.status === 'queued');
    if (!job) return;
    const control = new AbortController();
    this.active = { id: job.id, control };
    this.active.promise = this.execute(job, control.signal).finally(() => {
      this.active = null; if (!this.stopping) setImmediate(() => this.kick());
    });
    // Storage failure stops dispatch. Never continue on a potentially divergent store.
    this.active.promise.catch(e => { this.stopping = true; console.error('Worker stopped:', e.message); });
  }
  async execute(job, signal) {
    const number = job.attempts.length + 1, started = performance.now();
    if (number > 256) {
      await this.store.update(s => Object.assign(s.jobs.find(j => j.id === job.id), { status: 'failed', failure: 'Attempt capacity reached', finished_at: new Date().toISOString() }));
      return;
    }
    await this.store.update(s => {
      const j = s.jobs.find(j => j.id === job.id);
      if (j.status !== 'queued' || j.attempts.length + 1 !== number) throw Error('Dispatch fence rejected');
      j.status = 'running'; j.started_at ??= new Date().toISOString(); j.finished_at = null;
      j.attempts.push({ number, status: 'running', started_at: new Date().toISOString(), finished_at: null,
        input_checkpoint_sha256: j.checkpoint_sha256 });
    });
    let patch;
    try {
      this.assertBinary();
      if (job.engine_sha256 !== this.identity) throw Error('Queued job requires a different native binary');
      const s = settings(job.settings);
      const version = this.store.state.versions.find(v => v.id === job.version_id);
      const input = await this.store.read(job.checkpoint_sha256 ?? version.source_sha256);
      const args = ['export', '-', '--max-steps', String(s.max_steps), '--max-work', String(s.max_work),
        '--max-candidates', String(s.max_candidates), '--detect-cycles', String(s.detect_cycles), '--batch-steps', '10',
        '--execution-profile', s.execution_profile ?? 'reference-v1'];
      if (s.seed !== null) args.push('--strategy', 'seeded', '--seed', s.seed);
      const remainingTime = () => Math.max(0, s.deadline_ms - job.elapsed_ms - Math.ceil(performance.now() - started));
      if (!remainingTime()) throw Object.assign(Error('Execution allowance exhausted'), { code: 'timed_out' });
      const run = await native(this.binary, job.checkpoint_sha256 ? ['resume', '-', '--batch-steps', '10'] : args,
        input, { signal, deadline: remainingTime(), limits: this.limits });
      if (run.cause) {
        patch = { status: ['cancelled', 'timed_out', 'interrupted'].includes(run.cause) ? run.cause : 'failed', failure: run.cause };
      } else if (![0, 3, 4, 5, 6].includes(run.code)) {
        patch = { status: 'failed', failure: run.stderr.slice(0, 8192) || 'Native execution failed' };
      } else {
        // JSON is not trusted until the engine validates replay and scheduler claims.
        const remaining = remainingTime();
        if (remaining <= 0) { patch = { status: 'timed_out', failure: 'No time left for replay verification' }; }
        else {
          const checked = await native(this.binary, ['trace-check', '-'], run.stdout, { signal, deadline: remaining, limits: this.limits });
          if (checked.cause) patch = { status: ['cancelled', 'timed_out', 'interrupted'].includes(checked.cause) ? checked.cause : 'failed', failure: checked.cause };
          else if (checked.code !== 0) throw Error('Native result failed replay verification');
          else {
            const trace = JSON.parse(run.stdout);
            const expected = { NormalForm: 0, ResourceLimit: 3, StepLimit: 3, RequestedRuleUnavailable: 4, Cancelled: 5, CycleDetected: 6 };
            if (trace.source_sha256 !== version.source_sha256
                || !['yielded', 'finished'].includes(trace.status)
                || (trace.status === 'yielded' ? run.code !== 0 : expected[trace.report?.outcome] !== run.code))
              throw Error('Trace identity/outcome mismatch');
            if (signal.aborted) patch = { status: signal.reason === 'interrupted' ? 'interrupted' : 'cancelled', failure: 'Stopped before publication' };
            else if (trace.status === 'yielded') patch = { status: 'queued', checkpoint_sha256: await this.store.put(run.stdout),
              checkpoint_commits: trace.events.length, commits: trace.events.length, implementation: trace.implementation, failure: null };
            else patch = { status: 'completed', outcome: trace.report.outcome, commits: trace.events.length,
              trace_sha256: await this.store.put(run.stdout), implementation: trace.implementation, failure: null };
          }
        }
      }
    } catch (e) { patch = { status: e.code === 'timed_out' ? 'timed_out' : 'failed', failure: e.message }; }
    await this.store.update(s => {
      const j = s.jobs.find(j => j.id === job.id), attempt = j.attempts.at(-1);
      if (j.status !== 'running' || attempt?.number !== number || attempt.status !== 'running') throw Error('Publication fence rejected');
      if(signal.aborted || j.cancellation_requested_at)
        patch={status:!j.cancellation_requested_at&&signal.reason==='interrupted'?'interrupted':'cancelled',failure:'Stopped before publication'};
      j.elapsed_ms += Math.ceil(performance.now() - started);
      if (patch.status === 'queued' && j.elapsed_ms >= j.settings.deadline_ms) patch.status = 'timed_out';
      if (patch.status === 'completed' && j.elapsed_ms >= j.settings.deadline_ms)
        patch={status:'timed_out',failure:'Execution allowance exhausted before result publication'};
      attempt.status = patch.status === 'queued' ? 'yielded' : patch.status;
      attempt.finished_at = new Date().toISOString();
      attempt.output_sha256 = patch.trace_sha256 ?? patch.checkpoint_sha256 ?? null;
      if (patch.status === 'queued' && j.pause_requested_at) patch.status = 'paused';
      Object.assign(j, patch, { finished_at: ['queued', 'paused'].includes(patch.status) ? null : new Date().toISOString() });
    });
  }
  async pause(id, authorize = () => {}) {
    await this.store.update(s => {
      authorize();
      const j = s.jobs.find(j => j.id === id);
      if (!['queued', 'running', 'paused'].includes(j.status)) fail(409, 'not_pausable', 'Only queued or running jobs can pause');
      j.pause_requested_at ??= new Date().toISOString();
      if (j.status === 'queued') j.status = 'paused';
    });
  }
  async resume(id, authorize = () => {}) {
    this.assertBinary();
    await this.store.update(s => {
      authorize();
      const j = s.jobs.find(j => j.id === id);
      if (['queued', 'running'].includes(j.status) && !j.pause_requested_at) return;
      if (!['paused', 'interrupted'].includes(j.status)) fail(409, 'not_resumable', 'Only paused or interrupted jobs can resume');
      if (j.status === 'interrupted' && !j.checkpoint_sha256) fail(409, 'no_checkpoint', 'No verified checkpoint; submit a new job');
      if (j.status === 'interrupted' && s.jobs.filter(item=>!terminal.has(item.status)).length>=32)
        fail(429, 'capacity', 'Outstanding job capacity reached');
      if (s.jobs.some(child => child.parent_job_id === id)) fail(409, 'frozen_parent', 'A retry already references this interrupted run; submit a new job');
      if (j.engine_sha256 !== this.identity) fail(409, 'engine_changed', 'Continuation requires the original native binary');
      if (j.elapsed_ms >= j.settings.deadline_ms || j.attempts.length >= 256) fail(409, 'budget_exhausted', 'Continuation allowance exhausted');
      j.status = 'queued'; j.pause_requested_at = null; j.finished_at = null; j.failure = null;
    });
  }
  async cancel(id, authorize = () => {}) {
    if (this.active?.id === id) {
      const active=this.active;
      await this.store.update(s => { authorize();s.jobs.find(j => j.id === id).cancellation_requested_at = new Date().toISOString(); });
      active.control.abort('cancelled');
    }
    else await this.store.update(s => { authorize();const j = s.jobs.find(j => j.id === id); if (['queued', 'paused'].includes(j.status)) {
      j.status = 'cancelled'; j.finished_at = new Date().toISOString(); j.failure = 'Cancelled before dispatch';
    } });
  }
  async close() {
    this.stopping = true;
    if (this.active) { this.active.control.abort('interrupted'); await this.active.promise; }
  }
}
