// Single-writer local store. Never expose this directory directly through HTTP.
import fs from 'node:fs';
import path from 'node:path';
import { createHash, randomUUID } from 'node:crypto';

export const hash = data => createHash('sha256').update(data).digest('hex');
export class ApiError extends Error {
  constructor(status, code, message) { super(message); this.status = status; this.code = code; }
}
export const fail = (status, code, message) => { throw new ApiError(status, code, message); };
export const terminal = new Set(['completed', 'cancelled', 'timed_out', 'failed', 'interrupted']);
const uuid = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/;
const digest = /^[0-9a-f]{64}$/;
const uid = value => typeof value === 'string' && value.length > 0 && value.length <= 128 && !/[\x00-\x1f\x7f]/.test(value);

function syncDirectory(dir) {
  const fd = fs.openSync(dir, 'r');
  try { fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
}
function atomic(file, value) {
  const temporary = `${file}.${randomUUID()}.tmp`;
  const fd = fs.openSync(temporary, 'wx', 0o600);
  try { fs.writeFileSync(fd, value); fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
  try { fs.renameSync(temporary, file); syncDirectory(path.dirname(file)); }
  finally { if (fs.existsSync(temporary)) fs.unlinkSync(temporary); }
}

export function validate(state) {
  if (state?.schema !== 'reflective-local-store-v1' || !Array.isArray(state.projects)
      || !Array.isArray(state.versions) || !Array.isArray(state.jobs)
      || !Array.isArray(state.datasets) || state.datasets.length > 128
      || state.projects.length > 32 || state.versions.length > 128 || state.jobs.length > 256)
    throw Error('Invalid store schema or capacity');
  const ids = new Set();
  const shared = state.identity_namespace?.startsWith('firebase-emulator:')
    || state.identity_namespace === 'firebase-production:reflective-graph-f1cd4';
  if (state.identity_namespace != null && state.identity_namespace !== 'local-single-owner'
      && state.identity_namespace !== 'firebase-production:reflective-graph-f1cd4'
      && !/^firebase-emulator:demo-[a-z0-9-]{1,24}$/.test(state.identity_namespace)) throw Error('Invalid store identity namespace');
  for (const project of state.projects) {
    if (shared) {
      if (!uid(project.owner_uid) || !Array.isArray(project.members) || project.members.length > 32
          || new Set(project.members.map(m => m.uid)).size !== project.members.length
          || project.members.some(m => !uid(m.uid) || m.uid === project.owner_uid || !['viewer', 'editor'].includes(m.role)))
        throw Error('Invalid stored project access');
    } else if (project.owner_uid != null || project.members != null) throw Error('Shared project in local identity store');
  }
  for (const entry of [...state.projects, ...state.datasets, ...state.versions, ...state.jobs]) {
    if (!uuid.test(entry.id) || ids.has(entry.id)) throw Error('Invalid or duplicate stored identity');
    ids.add(entry.id);
  }
  const seenVersions = new Set(), seenJobs = new Set();
  const seenDatasets = new Set();
  for (const data of state.datasets) {
    if (!state.projects.some(p => p.id === data.project_id) || !digest.test(data.document_sha256)
        || !digest.test(data.source_sha256) || !digest.test(data.validated_engine_sha256)
        || data.schema !== 'work-items-v1' || !Number.isSafeInteger(data.items) || data.items < 0 || data.items > 1000)
      throw Error('Invalid stored dataset');
    if (data.parent_dataset_id != null && (!seenDatasets.has(data.parent_dataset_id)
        || !state.datasets.some(d => d.id === data.parent_dataset_id && d.project_id === data.project_id)))
      throw Error('Invalid stored dataset parent');
    seenDatasets.add(data.id);
  }
  for (const version of state.versions) {
    if (!state.projects.some(p => p.id === version.project_id) || !digest.test(version.source_sha256))
      throw Error('Invalid stored version');
    if(version.application && (version.application.schema!=='shopping-months-v1'||!Number.isSafeInteger(version.application.rows)||version.application.rows<1||version.application.rows>15000||!['fixed','guarded','disabled'].includes(version.application.policy)||!digest.test(version.application.input_sha256)))throw Error('Invalid stored application import');
    if (version.dataset_id != null && !state.datasets.some(d => d.id === version.dataset_id
        && d.project_id === version.project_id && d.source_sha256 === version.source_sha256))
      throw Error('Invalid version dataset identity');
    if (version.parent_version_id != null && (!seenVersions.has(version.parent_version_id)
        || !state.versions.some(v => v.id === version.parent_version_id && v.project_id === version.project_id)))
      throw Error('Invalid stored version parent');
    seenVersions.add(version.id);
  }
  for (const job of state.jobs) {
    if (shared && !uid(job.submitted_by)) throw Error('Invalid stored job submitter');
    if (job.parent_job_id != null && (!seenJobs.has(job.parent_job_id)
        || !state.jobs.some(j => j.id === job.parent_job_id && j.project_id === job.project_id && terminal.has(j.status))))
      throw Error('Invalid stored job parent');
    seenJobs.add(job.id);
    if (!state.versions.some(v => v.id === job.version_id && v.project_id === job.project_id)
        || !['queued', 'running', 'paused', ...terminal].includes(job.status)
        || !digest.test(job.request_sha256) || !digest.test(job.engine_sha256)
        || (job.trace_sha256 !== null && !digest.test(job.trace_sha256))) throw Error('Invalid stored job');
    const version = state.versions.find(v => v.id === job.version_id);
    if (job.source_sha256 !== version.source_sha256 || (job.status === 'completed' && !job.trace_sha256))
      throw Error('Invalid job input/result identity');
    if (!Array.isArray(job.attempts) || job.attempts.length > 256
        || !Number.isSafeInteger(job.elapsed_ms) || job.elapsed_ms < 0
        || (job.checkpoint_sha256 !== null && !digest.test(job.checkpoint_sha256)))
      throw Error('Invalid job continuation metadata');
    for (const [index, attempt] of job.attempts.entries()) {
      if (attempt.number !== index + 1 || !Number.isFinite(Date.parse(attempt.started_at))
          || !['running', 'yielded', 'completed', 'cancelled', 'timed_out', 'failed', 'interrupted'].includes(attempt.status)
          || (attempt.finished_at !== null && !Number.isFinite(Date.parse(attempt.finished_at))))
        throw Error('Invalid job attempt');
    }
  }
}

export class Store {
  constructor(directory, { recover = false, namespace = 'local-single-owner', large = false } = {}) {
    this.large=large;
    this.directory = path.resolve(directory);
    fs.mkdirSync(this.directory, { recursive: true, mode: 0o700 });
    this.lock = path.join(this.directory, 'owner.lock');
    if (recover && fs.existsSync(this.lock)) {
      // Explicit operator action only. Concurrent recovery attempts are unsupported.
      const owner = JSON.parse(fs.readFileSync(this.lock, 'utf8'));
      if (!Number.isSafeInteger(owner.pid) || owner.pid <= 0) throw Error('Unrecognised lock; inspect manually');
      let dead = false;
      try { process.kill(owner.pid, 0); } catch (e) { if (e.code === 'ESRCH') dead = true; else throw e; }
      if (!dead) throw Error('Store owner is still alive; recovery refused');
      fs.unlinkSync(this.lock);
    }
    const fd = fs.openSync(this.lock, 'wx', 0o600);
    try { fs.writeFileSync(fd, JSON.stringify({ pid: process.pid })); fs.fsyncSync(fd); }
    finally { fs.closeSync(fd); }
    this.file = path.join(this.directory, 'metadata.json');
    this.blobs = path.join(this.directory, 'blobs');
    try {
      fs.mkdirSync(this.blobs, { recursive: true, mode: 0o700 });
      if (fs.existsSync(this.file) && fs.statSync(this.file).size > 8 * 1024 * 1024) throw Error('Store too large');
      this.state = fs.existsSync(this.file) ? JSON.parse(fs.readFileSync(this.file, 'utf8'))
        : { schema: 'reflective-local-store-v1', identity_namespace: namespace, projects: [], versions: [], jobs: [] };
      if ((this.state.identity_namespace ?? 'local-single-owner') !== namespace) throw Error('Store identity namespace mismatch; use a separate store directory');
      this.state.identity_namespace = namespace;
      this.state.datasets ??= []; // Additive migration; existing sources and jobs stay immutable.
      for (const job of this.state.jobs) {
        job.attempts ??= []; job.elapsed_ms ??= 0; job.checkpoint_sha256 ??= null;
      }
      validate(this.state);
      // Reject corrupted referenced bytes before serving any state. Orphans are not results.
      for (const v of this.state.versions) {this.read(v.source_sha256);if(v.application)this.read(v.application.input_sha256);}
      for (const d of this.state.datasets) { this.read(d.document_sha256); this.read(d.source_sha256); }
      for (const j of this.state.jobs) if (j.trace_sha256) {
        const trace = JSON.parse(this.read(j.trace_sha256));
        if (trace.source_sha256 !== j.source_sha256 || trace.report?.outcome !== j.outcome
            || trace.events?.length !== j.commits || trace.implementation !== j.implementation)
          throw Error('Stored result metadata disagrees with trace');
      }
      for (const j of this.state.jobs) if (j.checkpoint_sha256) {
        const trace = JSON.parse(this.read(j.checkpoint_sha256));
        if (trace.status !== 'yielded' || trace.source_sha256 !== j.source_sha256
            || trace.events?.length !== j.checkpoint_commits || trace.implementation !== j.implementation)
          throw Error('Stored checkpoint metadata disagrees with trace');
      }
      this.update(next => {
        for (const job of next.jobs) if (job.status === 'running') {
          const attempt = job.attempts.at(-1);
          if (attempt?.status === 'running') {
            // A crash loses the monotonic clock. Charge downtime conservatively; a
            // backwards wall clock consumes the entire remaining execution allowance.
            const delta = Date.now() - Date.parse(attempt.started_at);
            job.elapsed_ms += Math.min(Math.max(0, job.settings.deadline_ms - job.elapsed_ms), delta < 0 ? job.settings.deadline_ms : Math.ceil(delta));
            attempt.status = 'interrupted'; attempt.finished_at = new Date().toISOString();
          }
          job.status = job.cancellation_requested_at ? 'cancelled' : 'interrupted'; job.finished_at = new Date().toISOString();
          job.failure = 'Server stopped before verified publication. Only the last verified checkpoint can be recovered.';
        }
      });
    } catch (e) { this.close(); throw e; }
  }
  update(change) {
    if (this.fault) throw Error('Store requires restart after persistence failure');
    const next = structuredClone(this.state);
    const result = change(next);
    validate(next);
    const bytes = JSON.stringify(next);
    if (Buffer.byteLength(bytes) > 8 * 1024 * 1024) fail(507, 'storage_limit', 'Metadata capacity reached');
    try { atomic(this.file, bytes); } catch (error) { this.fault = true; throw error; }
    this.state = next;
    return result;
  }
  put(bytes) {
    const sha = hash(bytes), file = path.join(this.blobs, sha);
    if (fs.existsSync(file)) { this.read(sha); return sha; }
    const used = fs.readdirSync(this.blobs).reduce((n, name) => n + fs.statSync(path.join(this.blobs, name)).size, 0);
    if (Buffer.byteLength(bytes) > (this.large?128:16)*1024*1024 || used + Buffer.byteLength(bytes) > (this.large?1024:64) * 1024 * 1024) fail(507, 'storage_limit', 'Local artefact capacity reached');
    atomic(file, bytes);
    return sha;
  }
  read(sha) {
    if (!digest.test(sha)) throw Error('Invalid artefact digest');
    const file = path.join(this.blobs, sha);
    if (fs.statSync(file).size > (this.large?128:16) * 1024 * 1024) throw Error('Artefact too large');
    const bytes = fs.readFileSync(file);
    if (hash(bytes) !== sha) throw Error('Artefact checksum mismatch');
    return bytes;
  }
  close() { if (this.lock && fs.existsSync(this.lock)) fs.unlinkSync(this.lock); this.lock = null; }
}
