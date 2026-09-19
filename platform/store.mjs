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

function validate(state) {
  if (state?.schema !== 'reflective-local-store-v1' || !Array.isArray(state.projects)
      || !Array.isArray(state.versions) || !Array.isArray(state.jobs)
      || state.projects.length > 32 || state.versions.length > 128 || state.jobs.length > 256)
    throw Error('Invalid store schema or capacity');
  const ids = new Set();
  for (const entry of [...state.projects, ...state.versions, ...state.jobs]) {
    if (!uuid.test(entry.id) || ids.has(entry.id)) throw Error('Invalid or duplicate stored identity');
    ids.add(entry.id);
  }
  const seenVersions = new Set(), seenJobs = new Set();
  for (const version of state.versions) {
    if (!state.projects.some(p => p.id === version.project_id) || !digest.test(version.source_sha256))
      throw Error('Invalid stored version');
    if (version.parent_version_id != null && (!seenVersions.has(version.parent_version_id)
        || !state.versions.some(v => v.id === version.parent_version_id && v.project_id === version.project_id)))
      throw Error('Invalid stored version parent');
    seenVersions.add(version.id);
  }
  for (const job of state.jobs) {
    if (job.parent_job_id != null && (!seenJobs.has(job.parent_job_id)
        || !state.jobs.some(j => j.id === job.parent_job_id && j.project_id === job.project_id && terminal.has(j.status))))
      throw Error('Invalid stored job parent');
    seenJobs.add(job.id);
    if (!state.versions.some(v => v.id === job.version_id && v.project_id === job.project_id)
        || !['queued', 'running', ...terminal].includes(job.status)
        || !digest.test(job.request_sha256) || !digest.test(job.engine_sha256)
        || (job.trace_sha256 !== null && !digest.test(job.trace_sha256))) throw Error('Invalid stored job');
    const version = state.versions.find(v => v.id === job.version_id);
    if (job.source_sha256 !== version.source_sha256 || (job.status === 'completed' && !job.trace_sha256))
      throw Error('Invalid job input/result identity');
  }
}

export class Store {
  constructor(directory, { recover = false } = {}) {
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
        : { schema: 'reflective-local-store-v1', projects: [], versions: [], jobs: [] };
      validate(this.state);
      // Reject corrupted referenced bytes before serving any state. Orphans are not results.
      for (const v of this.state.versions) this.read(v.source_sha256);
      for (const j of this.state.jobs) if (j.trace_sha256) {
        const trace = JSON.parse(this.read(j.trace_sha256));
        if (trace.source_sha256 !== j.source_sha256 || trace.report?.outcome !== j.outcome
            || trace.events?.length !== j.commits || trace.implementation !== j.implementation)
          throw Error('Stored result metadata disagrees with trace');
      }
      this.update(next => {
        for (const job of next.jobs) if (job.status === 'running') {
          job.status = job.cancellation_requested_at ? 'cancelled' : 'interrupted'; job.finished_at = new Date().toISOString();
          job.failure = 'Server stopped before verified publication; submit a new job to retry.';
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
    if (used + Buffer.byteLength(bytes) > 64 * 1024 * 1024) fail(507, 'storage_limit', 'Local artefact capacity reached');
    atomic(file, bytes);
    return sha;
  }
  read(sha) {
    if (!digest.test(sha)) throw Error('Invalid artefact digest');
    const file = path.join(this.blobs, sha);
    if (fs.statSync(file).size > 16 * 1024 * 1024) throw Error('Artefact too large');
    const bytes = fs.readFileSync(file);
    if (hash(bytes) !== sha) throw Error('Artefact checksum mismatch');
    return bytes;
  }
  close() { if (this.lock && fs.existsSync(this.lock)) fs.unlinkSync(this.lock); this.lock = null; }
}
