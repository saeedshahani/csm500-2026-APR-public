# Local developer API

The developer service is a loopback HTTP API backed by the existing native Rust CLI and connected to the browser's Projects interface. It saves projects and immutable graph-source versions, queues bounded jobs, publishes replay-verified traces and recovers metadata after restart. The Node adapter implements transport and orchestration, not graph semantics. It has no third-party Node dependencies.

This is **local, single-owner software**, not a Firebase deployment, multi-user service or general-purpose job sandbox. Do not bind it publicly, proxy it onto a network or share its store directory. Hosted identity, project-user permissions, Firebase adapters and production resource isolation remain separate work.

## Run and test

Run from the repository root with Node 24 or later and the pinned Rust toolchain:

```sh
cargo build --release --locked -p cli
export GRAPH_API_TOKEN="$(node -e 'process.stdout.write(require("node:crypto").randomBytes(32).toString("hex"))')"
export GRAPH_BROWSER_ORIGINS="http://127.0.0.1:4173"
node platform/server.mjs
```

The service listens on `http://127.0.0.1:8787`. Private files go under ignored `output/platform`. `GRAPH_STORE`, `GRAPH_ENGINE` and `GRAPH_PORT` override the local directory, native binary and port. Paths are operator configuration, never HTTP arguments. Do not commit or log the token. No Node packages need installing.

To use the browser, build and start the frontend in another terminal (`cd frontend`, `npm ci`, `npm run build`, `npm run preview`). Open `http://127.0.0.1:4173/#projects`, enter the API address and transfer the token privately into the password field. On macOS, `printf %s "$GRAPH_API_TOKEN" | pbcopy` copies the token without printing it; use this in the terminal where the variable is set before starting the server. Clear the clipboard afterwards. The browser retains the token only in memory; reconnect after a page reload. If using the Vite development server instead, allow its actual origin, normally `http://127.0.0.1:5173`, and restart the API. `GRAPH_BROWSER_ORIGINS` accepts comma-separated exact loopback HTTP origins, not wildcards or arbitrary websites.

Create a project, use **Save a new version → Use workspace draft**, name and save it, then submit a bounded job. The selected saved version, not the unsaved editor, is executed. **Inspect graph and history** checks the downloaded hash and then imports through the existing WASM verifier. Opening source changes only the editor after confirmation; importing a run changes only the loaded execution after confirmation. Neither silently replaces both. Projects and jobs survive leaving the page; the browser does not need to remain open for native execution.

In another terminal at the root, set `GRAPH_API_TOKEN` to the same value (transfer it privately), then run:

```sh
node platform/client-example.mjs examples/programs/arithmetic.graph output/api-arithmetic.json
target/release/cli trace-check output/api-arithmetic.json
node --test platform/tests/*.test.mjs
```

The example refuses to overwrite an existing output; each invocation creates a project. Tests use temporary stores and real native processes, including server-kill/restart. They need loopback permission but no cloud connection.

## API contract

The machine-readable contract is [openapi.json](openapi.json). Every application route requires `Authorization: Bearer TOKEN`. Browser `Origin` requests are denied unless the operator explicitly allows their exact origin. Allowed preflight requests need no token but grant only GET/POST and Authorization/Content-Type headers; actual requests still require the token. Cookies are not used. Source/trace responses expose their SHA-256 header. This token represents one owner of the whole store, not separate project tenants. Lists accept `offset` and `limit` (maximum 100). Responses are not cached.

| Operation | Route |
|---|---|
| Health and engine identity | `GET /v1/health` |
| Validate source | `POST /v1/validate` |
| Create/list projects | `POST/GET /v1/projects` |
| Validate/save or list source versions | `POST/GET /v1/projects/{project}/versions` |
| Retrieve exact source | `GET /v1/projects/{project}/versions/{version}/source` |
| Submit/list jobs | `POST/GET /v1/projects/{project}/jobs` |
| Inspect job | `GET /v1/jobs/{job}` |
| Cancel, with empty JSON object | `POST /v1/jobs/{job}/cancel` |
| Download verified trace | `GET /v1/jobs/{job}/trace` |

A version contains the complete authored graph program, including data and rules. Separate dataset uploads and client-supplied file paths are not supported. Source is well-formed text capped at 256 KiB UTF-8; request bodies are capped at 1 MiB. Checked-loader errors return 422 without creating a version. Unknown fields/settings are rejected. One native slot is used: validation while another validation/job is active returns 429; jobs can queue.

Saving a version accepts optional `parent_version_id`: an existing version in the same project. The browser's **Fork version** copies verified source into an editable form and records that parent when saved. This external authoring operation is not graph-resident self-modification. Submission accepts optional `parent_job_id`: a terminal job in the same project, included in idempotent request identity. **Prepare retry / what-if** copies the old settings and version into the submission form; the user can change them and submit a new history with that parent. Older stores without these fields remain readable. Parent links must point to earlier records and are checked at startup. Metadata comparison does not establish graph equivalence.

The browser retains a submission key after an uncertain response, reusing it for the same project/version/settings/parent request until success. It never automatically repeats POST requests. After reload or disconnect, inspect the durable run list before submitting again: in-memory pending keys do not survive. Creating projects and versions is not idempotent; after an uncertain response, refresh/reconnect and inspect the list before repeating the action.

Submission requires `version_id`, `idempotency_key` and optional `settings`. Normalised defaults are `max_steps: 100`, `max_work: 1000000`, `max_candidates: 1000`, `seed: null`, `detect_cycles: false`, `deadline_ms: 10000`. Steps are capped at 1000, deadlines at 30000 ms, and search settings cannot exceed their defaults. A seed is a decimal u64 string. The observer is complete-carrier; custom observers are not yet exposed. Other native/recording limits retain the pinned CLI defaults. Trace output is capped at 16 MiB.

Admission caps are 32 projects, 128 versions, 256 total jobs and 32 outstanding jobs. Metadata is capped at 8 MiB and artefacts at 64 MiB; nothing is automatically deleted. A project-scoped idempotency key with the same normalised request returns the original job, including after restart; different input returns 409. A new key creates a distinct execution. Inputs, settings and native binary digest are pinned. Stop before rebuilding/replacing the binary. Queued jobs requiring a different binary fail rather than silently changing semantics.

Errors are `{ "error": { "code": "...", "message": "..." } }`: malformed input (400), missing credentials (401), origin/host denied (403), missing resource (404), conflict/capacity (409), body too large (413), media type (415), invalid/resource-limited validation (422), busy/queue capacity (429), internal failure (500), stopped/faulted service (503), storage capacity (507).

## Outcomes and recovery

`queued` and `running` are platform states. `completed` means a trace was produced and verified by native replay; inspect its separate engine `outcome`, which can be `NormalForm`, `StepLimit`, `ResourceLimit` or `CycleDetected`. A completed bounded attempt is not necessarily convergence. The matcher, limits and scheduling semantics are unchanged.

`cancelled`, `timed_out`, `interrupted` and `failed` publish no trace or engine outcome. Cancellation/deadline kills the child; this is **not** graceful engine-boundary cancellation or resumable history. The deadline covers execution and verification, not queue time or final synchronous disk publication. There is no hard OS memory limit: semantic/output caps bound work, but process/container memory enforcement remains required before remote execution. Progress reports lifecycle state, not live per-step counts.

The single-writer store uses a lock, copy-on-write metadata, atomic rename and file/directory synchronization. Hashes are checked on reads/startup. Bytes are persisted before metadata references them; crashes may leave unreferenced artefacts, never a published result without its bytes. Orphans count toward capacity and are not automatically removed. Filesystem power-loss guarantees are not established by these tests; use a local disk, not a network/synchronised directory, for recovery experiments.

Queued work survives restart. Previously running jobs become `interrupted`, or `cancelled` if a cancellation request was persisted. They are not retried automatically. A new key retries from the initial version as a **new history**, not continuation. A surviving old child writes only to pipes and cannot publish into the store.

Abrupt crashes leave `owner.lock`. Once the old service is confirmed stopped, run `node platform/server.mjs --recover`. Recovery refuses if the recorded PID is alive. This is a single-operator action; simultaneous recovery commands and automatic PID-reuse resolution are unsupported. Corrupted metadata/artefacts fail startup without resetting user data. Back up a stopped store before manual repair. No distributed leases or multi-worker fencing are claimed.

## Firebase boundary

The API/job contract and browser workflow support the selected Firebase direction, but this file-store adapter is not a cloud database. Hosted authentication, Firestore/Storage adapters, distributed dispatch/fencing, resource quotas and deployment tests remain open. Never reuse the local token as hosted authentication. Graph interpretation remains in the native engine. Before hosted setup, agree on the Firebase project, region, authentication providers, project roles, intended audience and cost limits. Service credentials must not be placed in the frontend or shared in chat. Emulator-backed implementation precedes any separately authorised deployment or billing activation.
