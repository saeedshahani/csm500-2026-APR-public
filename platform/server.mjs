import http from 'node:http';
import { randomUUID, timingSafeEqual } from 'node:crypto';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { Store, ApiError, fail, hash, terminal } from './store.mjs';
import { Worker, settings } from './worker.mjs';

const MAX_BODY = 1024 * 1024;
function object(value, allowed, required = allowed) {
  if (!value || typeof value !== 'object' || Array.isArray(value)
      || Object.keys(value).some(k => !allowed.includes(k)) || required.some(k => !Object.hasOwn(value, k)))
    fail(400, 'invalid_request', 'Invalid or unknown request fields');
}
function name(value) {
  if (typeof value !== 'string' || !value.trim() || value.length > 100) fail(400, 'invalid_name', 'Name must contain 1–100 characters');
  return value.trim();
}
function source(value) {
  if (typeof value !== 'string' || !value.length || Buffer.byteLength(value) > 256 * 1024 || !value.isWellFormed())
    fail(400, 'invalid_source', 'Source must be well-formed text, 1–262144 UTF-8 bytes');
  return value;
}
async function body(req) {
  if (req.headers['content-type']?.split(';')[0].trim() !== 'application/json') fail(415, 'content_type', 'Expected application/json');
  const chunks = []; let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > MAX_BODY) fail(413, 'body_limit', 'Request exceeds 1 MiB');
    chunks.push(chunk);
  }
  try { return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks))); }
  catch { fail(400, 'invalid_json', 'Expected valid UTF-8 JSON'); }
}
function page(items, url) {
  if ([...url.searchParams.keys()].some(k => !['offset', 'limit'].includes(k))) fail(400, 'invalid_query', 'Unknown query parameter');
  const offset = Number(url.searchParams.get('offset') ?? 0), limit = Number(url.searchParams.get('limit') ?? 25);
  if (!Number.isSafeInteger(offset) || offset < 0 || !Number.isSafeInteger(limit) || limit < 1 || limit > 100)
    fail(400, 'invalid_query', 'Invalid pagination');
  return { items: items.slice(offset, offset + limit), total: items.length, next_offset: offset + limit < items.length ? offset + limit : null };
}

export async function start({ directory, binary, token, port = 0, recover = false, origins = [] }) {
  if (typeof token !== 'string' || !/^[A-Za-z0-9_-]{32,128}$/.test(token)) throw Error('A 32–128 character local bearer token is required');
  if (!Array.isArray(origins) || origins.some(origin => {
    try { const url = new URL(origin); return url.origin !== origin || url.protocol !== 'http:' || !['127.0.0.1', 'localhost'].includes(url.hostname); }
    catch { return true; }
  })) throw Error('Browser origins must be exact http://127.0.0.1 or http://localhost origins with an optional port');
  const store = new Store(directory, { recover });
  let worker;
  try { worker = new Worker(store, resolve(binary)); } catch (e) { store.close(); throw e; }
  let validating = false, closing = false;
  const validate = async text => {
    if (validating || worker.active) fail(429, 'busy', 'Native slot busy; retry validation later');
    validating = true;
    try { return await worker.validate(text); } finally { validating = false; worker.kick(); }
  };
  const project = id => store.state.projects.find(p => p.id === id) ?? fail(404, 'not_found', 'Project not found');
  const job = id => store.state.jobs.find(j => j.id === id) ?? fail(404, 'not_found', 'Job not found');
  const server = http.createServer(async (req, res) => {
    res.setHeader('Cache-Control', 'no-store'); res.setHeader('X-Content-Type-Options', 'nosniff');
    const send = (status, data) => { res.writeHead(status, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(data)); };
    try {
      if (closing || worker.stopping || store.fault) fail(503, 'unavailable', 'Service stopping or persistence/worker unavailable');
      if (!/^127\.0\.0\.1:\d+$/.test(req.headers.host ?? '')) fail(403, 'host_denied', 'Expected loopback Host');
      const origin = req.headers.origin;
      if (origin) {
        if (!origins.includes(origin)) fail(403, 'origin_denied', 'Browser origin is not allowed');
        res.setHeader('Access-Control-Allow-Origin', origin);
        res.setHeader('Vary', 'Origin');
        res.setHeader('Access-Control-Expose-Headers', 'X-Content-SHA256');
      }
      if (req.method === 'OPTIONS') {
        const headers = (req.headers['access-control-request-headers'] ?? '').toLowerCase().split(',').map(s => s.trim()).filter(Boolean);
        if (!origin || !['GET', 'POST'].includes(req.headers['access-control-request-method']) || headers.some(h => !['authorization', 'content-type'].includes(h)))
          fail(403, 'preflight_denied', 'Unsupported browser preflight');
        res.writeHead(204, { 'Access-Control-Allow-Methods': 'GET, POST', 'Access-Control-Allow-Headers': 'Authorization, Content-Type' });
        res.end(); return;
      }
      const supplied = Buffer.from(req.headers.authorization ?? ''), expected = Buffer.from(`Bearer ${token}`);
      if (supplied.length !== expected.length || !timingSafeEqual(supplied, expected)) fail(401, 'unauthorized', 'Local bearer token required');
      const url = new URL(req.url, 'http://127.0.0.1');
      const parts = url.pathname.split('/').filter(Boolean);
      if (parts[0] !== 'v1') fail(404, 'not_found', 'Unknown API version or route');
      const method = req.method;
      if (method === 'GET' && url.pathname === '/v1/health') return send(200, { api: 'v1', mode: 'local-single-owner', engine_sha256: worker.identity });
      if (method === 'POST' && url.pathname === '/v1/validate') {
        const b = await body(req); object(b, ['source']); return send(200, await validate(source(b.source)));
      }
      if (parts[1] === 'projects' && parts.length === 2) {
        if (method === 'GET') return send(200, page(store.state.projects, url));
        if (method === 'POST') {
          const b = await body(req); object(b, ['name']);
          const p = { id: randomUUID(), name: name(b.name), created_at: new Date().toISOString() };
          store.update(s => { if (s.projects.length >= 32) fail(409, 'capacity', 'Project capacity reached'); s.projects.push(p); });
          return send(201, p);
        }
      }
      if (parts[1] === 'projects' && parts.length === 4) {
        const p = project(parts[2]);
        if (parts[3] === 'versions') {
          if (method === 'GET') return send(200, page(store.state.versions.filter(v => v.project_id === p.id), url));
          if (method === 'POST') {
            const b = await body(req); object(b, ['name', 'source', 'parent_version_id'], ['name', 'source']); const label = name(b.name), text = source(b.source);
            if (b.parent_version_id != null && !store.state.versions.some(v => v.id === b.parent_version_id && v.project_id === p.id))
              fail(404, 'not_found', 'Parent version not found in project');
            if (store.state.versions.length >= 128) fail(409, 'capacity', 'Version capacity reached');
            const validation = await validate(text);
            const v = { id: randomUUID(), project_id: p.id, name: label, source_sha256: store.put(text),
              validated_engine_sha256: validation.engine_sha256, parent_version_id: b.parent_version_id ?? null, created_at: new Date().toISOString() };
            store.update(s => s.versions.push(v)); return send(201, v);
          }
        }
        if (parts[3] === 'jobs') {
          if (method === 'GET') return send(200, page(store.state.jobs.filter(j => j.project_id === p.id), url));
          if (method === 'POST') {
            const b = await body(req); object(b, ['version_id', 'settings', 'idempotency_key', 'parent_job_id'], ['version_id', 'idempotency_key']);
            if (typeof b.idempotency_key !== 'string' || !/^[A-Za-z0-9_-]{1,100}$/.test(b.idempotency_key)) fail(400, 'invalid_key', 'Invalid idempotency key');
            const v = store.state.versions.find(v => v.id === b.version_id && v.project_id === p.id);
            if (!v) fail(404, 'not_found', 'Version not found in project');
            if (b.parent_job_id != null && !store.state.jobs.some(j => j.id === b.parent_job_id && j.project_id === p.id && terminal.has(j.status)))
              fail(409, 'invalid_parent', 'Parent job must be a finished job in this project');
            const config = settings(b.settings), request = hash(JSON.stringify({ version: v.id, settings: config,
              ...(b.parent_job_id != null ? { parent_job_id: b.parent_job_id } : {}) }));
            const existing = store.state.jobs.find(j => j.project_id === p.id && j.idempotency_key === b.idempotency_key);
            if (existing) {
              if (existing.request_sha256 !== request) fail(409, 'idempotency_conflict', 'Key already used with another request');
              return send(200, existing);
            }
            worker.assertBinary();
            const j = { id: randomUUID(), project_id: p.id, version_id: v.id, source_sha256: v.source_sha256,
              idempotency_key: b.idempotency_key, request_sha256: request, engine_sha256: worker.identity,
              settings: config, parent_job_id: b.parent_job_id ?? null, status: 'queued', outcome: null, commits: null, trace_sha256: null,
              failure: null, created_at: new Date().toISOString(), started_at: null, finished_at: null };
            store.update(s => {
              if (s.jobs.length >= 256 || s.jobs.filter(j => !terminal.has(j.status)).length >= 32) fail(429, 'capacity', 'Job capacity reached');
              s.jobs.push(j);
            });
            send(202, j); if (!validating) worker.kick(); return;
          }
        }
      }
      if (method === 'GET' && parts[1] === 'projects' && parts.length === 6 && parts[3] === 'versions' && parts[5] === 'source') {
        project(parts[2]);
        const v = store.state.versions.find(v => v.id === parts[4] && v.project_id === parts[2]);
        if (!v) fail(404, 'not_found', 'Version not found');
        const bytes = store.read(v.source_sha256);
        res.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8', 'X-Content-SHA256': v.source_sha256 });
        res.end(bytes); return;
      }
      if (parts[1] === 'jobs' && [3, 4].includes(parts.length)) {
        const j = job(parts[2]);
        if (method === 'GET' && parts.length === 3) return send(200, j);
        if (method === 'POST' && parts[3] === 'cancel') {
          object(await body(req), []);
          if (!terminal.has(j.status)) worker.cancel(j.id);
          return send(202, job(j.id));
        }
        if (method === 'GET' && parts[3] === 'trace') {
          if (!j.trace_sha256) fail(409, 'no_trace', 'No verified trace published');
          const bytes = store.read(j.trace_sha256);
          res.writeHead(200, { 'Content-Type': 'application/json', 'Content-Length': bytes.length,
            'X-Content-SHA256': j.trace_sha256 }); res.end(bytes); return;
        }
      }
      fail(404, 'not_found', 'Unknown route or method');
    } catch (e) {
      if (!(e instanceof ApiError)) console.error('API error:', e.message);
      if (!res.headersSent && !res.destroyed) send(e.status ?? 500, { error: { code: e.code ?? 'internal_error',
        message: e instanceof ApiError ? e.message : 'Internal failure; inspect local server logs' } });
      else res.destroy();
    }
  });
  server.requestTimeout = 15000; server.headersTimeout = 10000;
  server.maxConnections = 32;
  try { await new Promise((yes, no) => { server.once('error', no); server.listen(port, '127.0.0.1', yes); }); }
  catch (e) { store.close(); throw e; }
  worker.kick();
  return { url: `http://127.0.0.1:${server.address().port}`, store, worker,
    async close() {
      closing = true;
      const stopped = new Promise(resolve => server.close(resolve));
      try { await worker.close(); } finally { await stopped; store.close(); }
    } };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  if (args.some(a => a !== '--recover')) throw Error('Only --recover is accepted; configuration uses GRAPH_* environment variables');
  const port = Number(process.env.GRAPH_PORT ?? 8787);
  if (!Number.isSafeInteger(port) || port < 0 || port > 65535) throw Error('Invalid GRAPH_PORT');
  const app = await start({ directory: process.env.GRAPH_STORE ?? 'output/platform', binary: process.env.GRAPH_ENGINE ?? 'target/release/cli',
    token: process.env.GRAPH_API_TOKEN, port, recover: args.includes('--recover'),
    origins: process.env.GRAPH_BROWSER_ORIGINS?.split(',').map(s => s.trim()).filter(Boolean) ?? [] });
  console.log(JSON.stringify({ url: app.url, mode: 'local-single-owner' }));
  let stopping = false;
  for (const event of ['SIGINT', 'SIGTERM']) process.on(event, async () => {
    if (stopping) return; stopping = true; await app.close();
  });
}
