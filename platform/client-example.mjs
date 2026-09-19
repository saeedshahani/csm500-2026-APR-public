// Run from the repository root after starting the local service.
import { readFileSync, writeFileSync } from 'node:fs';
import { setTimeout as delay } from 'node:timers/promises';

const url = process.env.GRAPH_API_URL ?? 'http://127.0.0.1:8787';
if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(url)) throw Error('This example accepts loopback only');
const token = process.env.GRAPH_API_TOKEN;
if (!token) throw Error('Set GRAPH_API_TOKEN to the same value used by the server');
const [input, output] = process.argv.slice(2);
if (!input || !output) throw Error('Usage: node platform/client-example.mjs PROGRAM.graph NEW_TRACE.json');
async function api(route, value) {
  const r = await fetch(url + route, { method: value ? 'POST' : 'GET',
    headers: { Authorization: `Bearer ${token}`, ...(value ? { 'Content-Type': 'application/json' } : {}) },
    ...(value ? { body: JSON.stringify(value) } : {}) });
  if (!r.ok) throw Error(`${r.status}: ${await r.text()}`);
  return r.json();
}
const project = await api('/v1/projects', { name: 'Client example' });
const version = await api(`/v1/projects/${project.id}/versions`, { name: 'Imported source', source: readFileSync(input, 'utf8') });
let job = await api(`/v1/projects/${project.id}/jobs`, { version_id: version.id, idempotency_key: 'example-run', settings: { max_steps: 100 } });
while (['queued', 'running'].includes(job.status)) { await delay(100); job = await api(`/v1/jobs/${job.id}`); }
console.log(JSON.stringify(job, null, 2));
if (job.status !== 'completed') throw Error(`No verified result: ${job.status}`);
const response = await fetch(url + `/v1/jobs/${job.id}/trace`, { headers: { Authorization: `Bearer ${token}` } });
if (!response.ok) throw Error(await response.text());
writeFileSync(output, Buffer.from(await response.arrayBuffer()), { flag: 'wx', mode: 0o600 });
console.log(`Saved verified trace; engine outcome: ${job.outcome}. Verify independently with cli trace-check.`);
