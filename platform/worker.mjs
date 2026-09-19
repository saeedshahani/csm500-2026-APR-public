import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { hash, fail } from './store.mjs';

export const DEFAULTS = Object.freeze({ max_steps: 100, max_work: 1000000, max_candidates: 1000,
  seed: null, detect_cycles: false, deadline_ms: 10000 });
export function settings(value = {}) {
  if (!value || typeof value !== 'object' || Array.isArray(value)
      || Object.keys(value).some(k => !Object.hasOwn(DEFAULTS, k))) fail(400, 'invalid_settings', 'Unknown settings');
  const s = { ...DEFAULTS, ...value };
  for (const [key, maximum] of Object.entries({ max_steps: 1000, max_work: 1000000, max_candidates: 1000, deadline_ms: 30000 })) {
    if (!Number.isSafeInteger(s[key]) || s[key] < (key === 'deadline_ms' ? 1 : 0) || s[key] > maximum)
      fail(400, 'invalid_settings', `Invalid ${key}; maximum ${maximum}`);
  }
  if (typeof s.detect_cycles !== 'boolean' || !(s.seed === null || (typeof s.seed === 'string'
      && /^(0|[1-9][0-9]{0,19})$/.test(s.seed) && BigInt(s.seed) <= 18446744073709551615n)))
    fail(400, 'invalid_settings', 'Invalid cycle flag or decimal u64 seed');
  return s;
}

export function native(binary, args, input, { signal, deadline = 10000, maximum = 16 * 1024 * 1024 } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { stdio: ['pipe', 'pipe', 'pipe'], shell: false });
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
    this.active = null; this.stopping = false;
    for (const j of store.state.jobs) settings(j.settings);
  }
  assertBinary() {
    if (hash(readFileSync(this.binary)) !== this.identity) throw Error('Native binary changed; restart after rebuilding');
  }
  async validate(source) {
    this.assertBinary();
    const r = await native(this.binary, ['check', '-'], source, { maximum: 4 * 1024 * 1024 });
    if (r.cause) fail(422, r.cause, 'Validation did not complete');
    if (r.code !== 0) fail(422, r.code === 3 ? 'resource_limit' : 'invalid_program', r.stderr.slice(0, 8192));
    return { valid: true, source_sha256: hash(source), engine_sha256: this.identity };
  }
  kick() {
    if (this.active || this.stopping) return;
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
    const update = patch => this.store.update(s => Object.assign(s.jobs.find(j => j.id === job.id), patch));
    update({ status: 'running', started_at: new Date().toISOString() });
    let patch;
    try {
      this.assertBinary();
      if (job.engine_sha256 !== this.identity) throw Error('Queued job requires a different native binary');
      const s = settings(job.settings);
      const version = this.store.state.versions.find(v => v.id === job.version_id);
      const input = this.store.read(version.source_sha256);
      const args = ['export', '-', '--max-steps', String(s.max_steps), '--max-work', String(s.max_work),
        '--max-candidates', String(s.max_candidates), '--detect-cycles', String(s.detect_cycles)];
      if (s.seed !== null) args.push('--strategy', 'seeded', '--seed', s.seed);
      const start = Date.now();
      const run = await native(this.binary, args, input, { signal, deadline: s.deadline_ms });
      if (run.cause) {
        patch = { status: ['cancelled', 'timed_out', 'interrupted'].includes(run.cause) ? run.cause : 'failed', failure: run.cause };
      } else if (![0, 3, 4, 5, 6].includes(run.code)) {
        patch = { status: 'failed', failure: run.stderr.slice(0, 8192) || 'Native execution failed' };
      } else {
        // JSON is not trusted until the engine validates replay and scheduler claims.
        const remaining = s.deadline_ms - (Date.now() - start);
        if (remaining <= 0) { patch = { status: 'timed_out', failure: 'No time left for replay verification' }; }
        else {
          const checked = await native(this.binary, ['trace-check', '-'], run.stdout, { signal, deadline: remaining });
          if (checked.cause) patch = { status: ['cancelled', 'timed_out', 'interrupted'].includes(checked.cause) ? checked.cause : 'failed', failure: checked.cause };
          else if (checked.code !== 0) throw Error('Native result failed replay verification');
          else {
            const trace = JSON.parse(run.stdout);
            const expected = { NormalForm: 0, ResourceLimit: 3, StepLimit: 3, RequestedRuleUnavailable: 4, Cancelled: 5, CycleDetected: 6 };
            if (trace.source_sha256 !== version.source_sha256 || trace.status !== 'finished'
                || expected[trace.report?.outcome] !== run.code) throw Error('Trace identity/outcome mismatch');
            if (signal.aborted) patch = { status: signal.reason === 'interrupted' ? 'interrupted' : 'cancelled', failure: 'Stopped before publication' };
            else patch = { status: 'completed', outcome: trace.report.outcome, commits: trace.events.length,
              trace_sha256: this.store.put(run.stdout), implementation: trace.implementation };
          }
        }
      }
    } catch (e) { patch = { status: 'failed', failure: e.message }; }
    update({ ...patch, finished_at: new Date().toISOString() });
  }
  cancel(id) {
    if (this.active?.id === id) {
      this.store.update(s => { s.jobs.find(j => j.id === id).cancellation_requested_at = new Date().toISOString(); });
      this.active.control.abort('cancelled');
    }
    else this.store.update(s => { const j = s.jobs.find(j => j.id === id); if (j.status === 'queued') {
      j.status = 'cancelled'; j.finished_at = new Date().toISOString(); j.failure = 'Cancelled before dispatch';
    } });
  }
  async close() {
    this.stopping = true;
    if (this.active) { this.active.control.abort('interrupted'); await this.active.promise; }
  }
}
