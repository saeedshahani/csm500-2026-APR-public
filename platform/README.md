# Local developer API

## Large calendar-normalisation application

An explicit `large-local-v1` route processes the complete UCI Online Shoppers Purchasing Intention CSV through graph rewriting. It is a local developer deployment, not an increase to hosted Firebase quotas. Use a separate store for this route. It requires local owner-token authentication and refuses Firebase production/emulator modes or supplied remote storage.

```sh
cargo build --release --locked -p cli
GRAPH_EXECUTION_ENVELOPE=large-local-v1 GRAPH_STORE=output/calendar-platform GRAPH_API_TOKEN=replace_with_your_32_character_random_token GRAPH_BROWSER_ORIGINS=http://127.0.0.1:4173 node platform/server.mjs
```

In the browser, open Projects, choose Local API token, connect to `http://127.0.0.1:8787` with that token, and create a project. Download and extract [Sakar and Kastro's UCI release](https://doi.org/10.24432/C5F88Q), licensed CC BY 4.0. Import `online_shoppers_intention.csv` in Calendar normalisation, select the approval or fixed policy, and submit the selected version using the automatically supplied `joined-v4` settings. The source schema is deliberately restricted: all eighteen release columns, unquoted fields, known categories and valid nonnegative numerical ranges. Unknown columns, missing values and malformed rows reject before saving a version. This is not a general CSV or CSVW implementation.

Each source row becomes one constructor holding its complete ordered field values; its month relation points to a shared raw category. The graph-resident processor joins that relation with an immutable dictionary to point to the shared month-number category. It does not compute the answer in the importer. The optional approval rule first activates the processor, replaces revision tokens, lowers its own rank and disables itself. The disabled control intentionally produces no useful transformation. All other fields remain opaque preserved payload, not independently queryable graph attributes.

The route permits 50,000 atoms, 100,000 fields, 16 MiB source, one million syntax tokens, 20,000 events/candidates, ten million search-work units, 128 MiB trace artefacts, a 1 GiB local artefact store and at most 120 seconds per job including independent verification. Batches contain at most 5,000 commits; pause waits for a verified batch. Initial/endpoint checkpoints avoid repeated full snapshots but retain every event. An interrupted job resumes only from a verified checkpoint within its original remaining allowance. Cancellation publishes no unverified prefix. Storage exhaustion fails rather than publishing an incomplete result. These local logical/deadline bounds are not a portable hard memory sandbox; the retained study additionally samples native RSS. Hosted container limits are unchanged and are not suitable for this larger route.

The browser retrieves 25 before/after rows at a time, with full totals, original fields and independent acceptance status. CSV export includes all original columns plus `MonthNumber`; it refuses incomplete or incorrect results even if the graph reached NormalForm. Event pages remain bounded. Large full traces can be downloaded and checked with `GRAPH_EXECUTION_ENVELOPE=large-local-v1 target/release/cli trace-check TRACE`; they are not silently admitted into the smaller browser graph inspector. Source versions, raw CSV and traces are content-addressed. The result oracle compares against the original saved CSV, not just counts.

API additions are `POST /v1/projects/{project}/month-imports` with `{name,csv,policy}`, `GET /v1/jobs/{job}/month-results?offset=0&limit=25`, and `GET /v1/jobs/{job}/month-csv`. Health advertises `execution_envelope`; a client cannot request a larger envelope through job settings. The OpenAPI description includes these routes. Existing project access, cancellation, pause/resume and immutable version contracts apply.

Reproduction uses `scripts/collect-loop-c.mjs`, `scripts/compare-loop-c.mjs` and `scripts/accept-loop-c-recovery.mjs` with new output destinations. Place the UCI archive at `output/s6-datasets/shoppers.zip`; collection refuses bytes not matching the documented release digest. Source bundles and retained summaries support inspection without committing private local tokens or large generated stores. Native and browser acceptance are separate; conventional CSV transformation has different proof/history obligations and is expected to be much cheaper.

The developer service is a loopback HTTP API backed by the existing native Rust CLI and connected to the browser's Projects interface. It saves projects and immutable graph-source versions, queues bounded jobs, publishes replay-verified traces and recovers metadata after restart. The Node adapter implements transport and orchestration, not graph semantics. The optional Firebase Authentication emulator mode uses the pinned Admin SDK; the browser loads its Authentication SDK only when that mode is selected.

The commands in this section start **local development software**, with either one token owner or multiple emulator test accounts, not the separately accepted hosted pilot described below. Do not bind this adapter publicly or proxy it onto a network. Emulator tokens are unsigned. Production identity, IAM and hosted resource isolation use the separate hosted entry point and acceptance contract. The large local envelope is not a general-purpose job sandbox.

## Run and test

Run from the repository root with Node 24 or later and the pinned Rust toolchain:

```sh
cargo build --release --locked -p cli
export GRAPH_API_TOKEN="$(node -e 'process.stdout.write(require("node:crypto").randomBytes(32).toString("hex"))')"
export GRAPH_BROWSER_ORIGINS="http://127.0.0.1:4173"
node platform/server.mjs
```

The service listens on `http://127.0.0.1:8787`. Private files go under ignored `output/platform`. `GRAPH_STORE`, `GRAPH_ENGINE` and `GRAPH_PORT` override the local directory, native binary and port. Paths are operator configuration, never HTTP arguments. Do not commit or log the token. Local-token mode does not load the Firebase SDK and can run without installing platform packages.

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

The machine-readable contract is [openapi.json](openapi.json). Every application route requires `Authorization: Bearer TOKEN`: either the configured local owner token or an emulator ID token, never both modes together. Browser `Origin` requests are denied unless the operator explicitly allows their exact origin. Allowed preflight requests need no token but grant only GET/POST and Authorization/Content-Type headers; actual requests still require authentication. Cookies are not used. Source/trace responses expose their SHA-256 header. Lists accept `offset` and `limit` (maximum 100). Responses are not cached.

| Operation | Route |
|---|---|
| Health and engine identity | `GET /v1/health` |
| Validate source | `POST /v1/validate` |
| Create/list projects | `POST/GET /v1/projects` |
| Inspect/change membership (emulator mode, owner only) | `GET/POST /v1/projects/{project}/members` |
| Validate/save or list source versions | `POST/GET /v1/projects/{project}/versions` |
| Retrieve exact source | `GET /v1/projects/{project}/versions/{version}/source` |
| Submit/list jobs | `POST/GET /v1/projects/{project}/jobs` |
| Inspect job | `GET /v1/jobs/{job}` |
| Cancel, with empty JSON object | `POST /v1/jobs/{job}/cancel` |
| Download verified trace | `GET /v1/jobs/{job}/trace` |

A version contains the complete authored graph program, including data and rules. Named work-items-v1 datasets are stored separately as immutable authoring inputs; selecting dataset_id compiles their data and chosen policy into one ordinary program. Client-supplied file paths are not supported. Source is well-formed text capped at 256 KiB UTF-8; request bodies are capped at 1 MiB. Checked-loader errors return 422 without creating a version. Unknown fields/settings are rejected. One native slot is used: validation while another validation/job is active returns 429; jobs can queue.

Saving a version accepts optional `parent_version_id`: an existing version in the same project. The browser's **Fork version** copies verified source into an editable form and records that parent when saved. This external authoring operation is not graph-resident self-modification. Submission accepts optional `parent_job_id`: a terminal job in the same project, included in idempotent request identity. **Prepare retry / what-if** copies the old settings and version into the submission form; the user can change them and submit a new history with that parent. Older stores without these fields remain readable. Parent links must point to earlier records and are checked at startup. Metadata comparison does not establish graph equivalence.

The browser retains a submission key after an uncertain response, reusing it for the same project/version/settings/parent request until success. It never automatically repeats POST requests. After reload or disconnect, inspect the durable run list before submitting again: in-memory pending keys do not survive. Creating projects and versions is not idempotent; after an uncertain response, refresh/reconnect and inspect the list before repeating the action.

Submission requires `version_id`, `idempotency_key` and optional `settings`. Normalised defaults are `max_steps: 100`, `max_work: 1000000`, `max_candidates: 1000`, `seed: null`, `detect_cycles: false`, `deadline_ms: 10000`. Steps are capped at 1000, deadlines at 30000 ms, and search settings cannot exceed their defaults. A seed is a decimal u64 string. The observer is complete-carrier; custom observers are not yet exposed. Other native/recording limits retain the pinned CLI defaults. Trace output is capped at 16 MiB.

Admission caps are 32 projects, 128 datasets, 128 versions, 256 total jobs and 32 outstanding jobs across the store, not per user. Metadata is capped at 8 MiB and artefacts at 64 MiB; nothing is automatically deleted. An idempotency key is scoped to a project and, in emulator mode, its submitting UID. The same normalised request returns the original job, including after restart; different input returns 409. A new key creates a distinct execution. Inputs, settings and native binary digest are pinned. Stop before rebuilding/replacing the binary. Queued jobs requiring a different binary fail rather than silently changing semantics.

Errors are `{ "error": { "code": "...", "message": "..." } }`: malformed input (400), missing credentials (401), origin/host denied (403), missing resource (404), conflict/capacity (409), body too large (413), media type (415), invalid/resource-limited validation (422), busy/queue capacity (429), internal failure (500), stopped/faulted service (503), storage capacity (507).

## Datasets and result reports

`POST /v1/projects/{project}/datasets` accepts `{name, document, parent_dataset_id?}` where document is the exact JSON text for `work-items-v1`. The native compiler validates the schema, policy and unique item IDs, then checks its generated graph. Both original JSON and compiled source are content-addressed. Inputs are capped at 256 KiB and 1,000 items. `GET` on the same route lists immutable versions; `GET /v1/projects/{project}/datasets/{dataset}/document` returns the original bytes and digest header. Parent datasets must belong to the same project. Existing stores acquire an empty dataset collection without changing their programs or jobs.

Version creation accepts exactly one of `source` and `dataset_id`, alongside its name and optional program-version parent. A dataset-backed version pins the compiled source digest; the dataset remains authoring provenance, not a second executable graph. The browser exposes dataset saving, forks and program creation. The policy is part of work-items-v1; arbitrary user rule/dataset composition remains unsupported. Editing a fork externally is not graph-resident reflection.

`GET /v1/jobs/{job}/report` gives before/after atom, field, kind and role counts, total consumed/produced atoms and altered rule identities. Counts include quoted syntax and cannot establish equivalent graphs or useful convergence. `GET /v1/jobs/{job}/events?offset=0&limit=10` pages the verified trace's events, identifies its digest and reports the full event count. Each response is capped at 1 MiB; reduce the page size after 413, or download the separately bounded full trace. The local server still reads the whole capped trace; pagination is not evidence of streaming native storage or large-graph scalability. Both read routes enforce the same project authorization as trace download.

## Outcomes and recovery

`queued` and `running` are platform states. `completed` means a trace was produced and verified by native replay; inspect its separate engine `outcome`, which can be `NormalForm`, `StepLimit`, `ResourceLimit` or `CycleDetected`. A completed bounded attempt is not necessarily convergence. The matcher, limits and scheduling semantics are unchanged.

Jobs run in batches of at most ten commits. Each batch is replay-verified before its checkpoint, commit count and attempt record are published. `POST /v1/jobs/{job}/pause` requests a pause after the current batch; queued jobs pause immediately. `POST /v1/jobs/{job}/resume` continues a paused job or an interrupted job with a verified checkpoint. Both require editor access. `GET /v1/jobs/{job}/checkpoint` returns the latest yielded trace and its digest; it is separate from the finished `/trace` result. The browser exposes Pause job, Resume job, Download checkpoint and execution attempts. A batch can finish without another commit when its terminal query detects normality or a bound.

Continuation retains the original binary, source, scheduler, history and run limits. Duplicate resume delivery while a job is running or queued does not create another attempt. There are at most 256 attempts. The cumulative deadline charges execution, replay checking and checkpoint preparation across attempts; queue and deliberate pause time do not count. Final metadata persistence is outside the interruptible deadline. After a crash, elapsed wall time since the abandoned attempt started is conservatively charged, including downtime; backwards clock movement consumes the remaining allowance. Exhausted jobs cannot resume. A parent already referenced by a retry is frozen against later continuation.

`cancelled`, `timed_out`, `interrupted` and `failed` publish no finished trace or engine outcome. A previously verified checkpoint may remain downloadable, but cancellation is terminal; an interrupted job alone may recover from it. Any in-flight unverified suffix is discarded. On Linux, setting both `GRAPH_NATIVE_MEMORY_BYTES` (64–512 MiB) and `GRAPH_NATIVE_CPU_SECONDS` (1–30 seconds) applies `prlimit` to native child processes. These settings are refused on other operating systems rather than silently ignored. The container defaults to 512 MiB virtual address space and 30 CPU seconds per child; an outer container memory/CPU/PID bound covers Node and all children together. These are engineering limits, not semantic resource outcomes or a general untrusted-code sandbox.

The single-writer store uses a lock, copy-on-write metadata, atomic rename and file/directory synchronization. Hashes are checked on reads/startup. Bytes are persisted before metadata references them; crashes may leave unreferenced artefacts, never a published result without its bytes. Orphans count toward capacity and are not automatically removed. Filesystem power-loss guarantees are not established by these tests; use a local disk, not a network/synchronised directory, for recovery experiments.

Queued work survives restart. Previously running jobs become `interrupted`, or `cancelled` if a cancellation request was persisted. They are not retried automatically. Resume uses only the last verified checkpoint and remaining allowance; without a checkpoint, submit a new job. A new submission key starts a new history from the initial version. A surviving old child writes only to pipes and cannot publish into the store. Attempt-number checks reject stale publication within the service.

In file-store mode, abrupt crashes leave `owner.lock`. Once the old service is confirmed stopped, run `node platform/server.mjs --recover`. Recovery refuses if the recorded PID is alive. This is a single-operator action; simultaneous recovery commands and automatic PID-reuse resolution are unsupported. Corrupted metadata/artefacts fail startup without resetting user data. Back up a stopped store before manual repair. Hosted storage instead uses the generation-fenced lease described below.

## Firebase boundary

### Local test accounts and sharing

No Firebase login, paid project, service-account key or billing activation is needed. Install the locked tooling at the repository root, then start the emulator:

```sh
npm ci --prefix platform --ignore-scripts
npm run emulator --prefix platform
```

In a second terminal at the root, start the API with a separate store:

```sh
export GRAPH_AUTH_MODE=firebase-emulator
export GRAPH_FIREBASE_PROJECT=demo-reflective-graph
export FIREBASE_AUTH_EMULATOR_HOST=127.0.0.1:9099
export GRAPH_STORE=output/platform-emulator
export GRAPH_BROWSER_ORIGINS=http://127.0.0.1:4173
node platform/server.mjs
```

Start the frontend preview in a third terminal as above. In Projects, select **Firebase test accounts**, enter a disposable test email/password and tick **Create a test account** for the first connection. For later connections, untick it. The browser uses the fixed `demo-reflective-graph` project and `127.0.0.1:9099` emulator; the API must match. It displays your UID and refreshes ID tokens through the SDK without browser persistence. Disconnect signs out; reloading requires sign-in. Projects displays a scoped test-account warning instead of the SDK's fixed overlay.

Create a project to become its owner. Under **Project access**, paste another test account's UID and select viewer or editor. Submitting an existing UID changes its role; Remove revokes access. Owners alone administer membership; ownership transfer is not supported. Viewers can inspect/download versions and jobs. Editors can additionally save versions, submit jobs and cancel any project job. Authenticated users can create projects and use standalone validation. Membership requests contain only `uid` and `role`; `null` removes a member. There are at most 32 non-owner members. The API does not send invitations or look up accounts by email.

Project lists are filtered by current membership. An inaccessible project or its artefacts returns 404; an insufficient role within an accessible project returns 403. Job access is checked through its project, including direct trace and cancellation routes. Permissions are rechecked after awaited request bodies and native validation, before publication. Removing a member prevents subsequent access but does not retract downloaded files or cancel an already accepted job. Accepted execution belongs to the project and can be managed by its remaining editors/owner. Token revocation and disabled accounts are checked at request admission; a previously admitted request is not retroactively cancelled by account changes.

Stores record their identity namespace. An old local-token store cannot be opened as an emulator store, nor can an emulator store be reused for a different demo project. There is no automatic ownership migration. Stop the emulator with Ctrl-C to save its test accounts under ignored `output/auth-emulator`; the launcher imports them on the next start. Preserve this account state alongside the corresponding stopped project store. A crash before export can lose newly created emulator identities; recreating the same email produces a different UID and does not recover ownership. This is not a production backup mechanism.

For automated acceptance, stop the interactive emulator first, then run `npm run test:emulator --prefix platform`. It starts a fresh demo emulator, exercises real identities and native jobs, and stops it without replacing the interactive account export. Tests fail rather than skip when the emulator is unavailable. The launcher isolates CLI preferences and does not inherit cloud credential variables. Ports 9099, 4400 and 4500 must be free. The regular API suite remains `npm test --prefix platform`.

Firebase documents that [emulator ID tokens are unsigned](https://firebase.google.com/docs/emulator-suite/connect_auth), unlike production credentials. The Admin verifier's [revocation check](https://firebase.google.com/docs/auth/admin/manage-sessions) is enabled, and browser sessions use [in-memory persistence](https://firebase.google.com/docs/auth/web/auth-state-persistence). These checks exercise application access behaviour, not cryptographic authentication against a hostile local process. The emulator and API must remain loopback-only.

The pinned dependency tree has a targeted `gaxios@6.7.1` → `uuid@11.1.1` override to remove the runtime audit finding while retaining the CommonJS API used by that dependency. At verification, production platform dependencies and frontend dependencies have no reported npm advisories; five moderate findings remain in Firebase CLI development dependencies. Do not use an indiscriminate major downgrade or describe the complete tooling tree as advisory-free. Reassess before hosted delivery; these local tests do not audit unrelated CLI deployment/import functions.

### Firestore and Storage emulator persistence

The emulator launcher starts Auth, Firestore and Storage, using only `demo-reflective-graph`. To persist through those services instead of local files, also set `GRAPH_STORAGE_MODE=firebase-emulator`, `FIRESTORE_EMULATOR_HOST=127.0.0.1:8080` and `FIREBASE_STORAGE_EMULATOR_HOST=127.0.0.1:9199` in the API terminal. `GRAPH_STORE_ID` selects a simple store identifier (default `default`); it is operator configuration, not a client-supplied project selector. Keep `GRAPH_AUTH_MODE=firebase-emulator` and the demo project/Auth settings above. Stop cleanly so the launcher exports all three services. Production project IDs and non-loopback emulator hosts are refused by this adapter.

Each project, dataset, version and job occupies a separate Firestore document, capped at 800 KiB; graph/source/trace bytes live in Storage. A transactionally checked 60-second owner lease and increasing generation fence every metadata publication. Heartbeats renew the lease; a competing owner must wait for expiry or clean release. Losing the lease makes the old process unavailable. This intentionally supports one service owner, not independently scalable API and worker fleets. Application memory holds a bounded metadata snapshot, with lease checks before requests and writes. Direct client database/storage access is denied by the supplied rules; the API enforces project roles. Emulator rules do not test production IAM.

Artefact reservations count against the 64-MiB store quota before upload. Failed uploads remain reserved; retrying identical content reconciles the reservation without double charging. Uploads use generation preconditions and read-back checksums before a ready manifest is published. Live records, orphans and reservations are retained and count toward capacity; deleting history to reset usage is unsupported. Emulator tests cover restart, duplicate submissions, stale owners, failed uploads and corruption. Hosted backup retention is specified below; emulator checks alone do not establish cloud latency or availability.

### Remote container verification without local Docker

`platform/Dockerfile` builds the native CLI with the pinned Rust toolchain and packages the Node adapter without cloud credentials. The container CI workflow runs local lifecycle tests under 512-MiB memory, one CPU, 128-PID, read-only-root and temporary-storage bounds. Its image/binary identities are emitted by CI. Local Docker is optional: the build-only `platform/cloudbuild.yaml` can run equivalent tests through Cloud Build. The default container runs the loopback adapter; the approved deployment explicitly selects `platform/hosted.mjs`, with production identity and request-scoped dispatch. Cloud Run's actual limits are separately inspected; the CI PID/read-only flags are not claims about the managed service's configurable options. [Cloud Run supports remote source builds](https://docs.cloud.google.com/run/docs/deploying-source-code), including repositories with a Dockerfile.

### Hosted pilot and operator runbook

The hosted application is `https://reflective-graph-f1cd4.web.app`; the API base is `https://reflective-graph-f1cd4.web.app/api`. Email account registration requires twelve-character passwords and verification before API access. The web app is `1:463455802523:web:765223b9b6bf186ebb623e`. Public registration is enabled for a low-volume assessment/testing audience, not an unrestricted commercial-service claim. SDK configuration in the frontend is public identification, never an administrative credential.

Only project `reflective-graph-f1cd4` (number `463455802523`) is authorised. Every command must name it; no default aliases or account-wide billing changes are permitted. London (`europe-west2`) holds the database, buckets, image repository, queue, scheduler and Cloud Run service. The owner authorises paid pilot operation, reports £3 service limits and has a verified project-only £5 monthly budget. Alerts and application allowances do not prove a hard currency cap. The owner's accepted spending risk is not an outstanding deployment-approval gate.

`platform/hosted.mjs` uses production Admin verification and the managed `graph-runtime` identity. It refuses emulator or conflicting project variables and local bearer credentials. Public requests require verified password identities; `/_work` and `/_reconcile` require Google-signed OIDC for `graph-dispatch` with the exact service audience. Runtime permissions comprise database access, authentication lookup, task enqueue, bucket-object access and attachment of the dispatch identity. `graph-build` has separate logging, source-read and image-write permissions. Firebase client rules deny database/storage access; API project roles and IAM provide distinct boundaries. The underlying Cloud Run service accepts public HTTP invocation because Firebase user verification occurs inside the application; its internal routes are not unauthenticated.

The pilot acquires a sixty-second Firestore owner lease per request and releases it before responding. One native batch executes per task request; no background work depends on idle CPU. A duplicate task cannot re-execute a terminal/paused job or superseded attempt. Failed enqueue returns an uncertain response after durable submission; reuse the same idempotency key. Reconciliation every fifteen minutes rediscovers queued work. Busy requests return 503: retry reads or the same submitted operation explicitly. The browser serialises its own panel requests. A pause/cancel request competes with current work and may arrive after a short job finishes. Queue pause is an operator maintenance control; live acceptance observed delayed propagation, so do not treat its HTTP response as proof that dispatch has already ceased.

Cloud Run uses zero minimum instances, at most two instances, one request per instance, one CPU, 512 MiB and a 120-second HTTP timeout. Cloud Tasks permits one concurrent delivery, one dispatch per second and five attempts with 5–60-second backoff. The metadata lease supplies the cross-instance fence. Jobs have a cumulative 30-second maximum execution/replay allowance; queue and user pause time do not count. This serialised bounded design is suitable for the pilot, not high-throughput multi-tenancy.

Production-mode pilot admission limits each owner to three projects and each submitter to two outstanding jobs, including paused jobs. Submission reserves the full declared job deadline: at most twenty jobs and 120 seconds per user per UTC day, 600 seconds globally per UTC day, and 3,600 seconds globally per UTC month. These are admission allowances, not guarantees about provider CPU billing. Cancelled, failed and interrupted submissions retain reservations; duplicate submissions reuse the existing job without reserving twice. An interrupted job's resume checks its original submitter's outstanding allowance. Day/month rollover permits new submissions but does not erase old histories or outstanding jobs. Validation, API requests, authentication, storage, builds and logging are not covered by these execution reservations. Consequently these controls alone are insufficient for public release or a GBP 5 guarantee.

Persisted request counters additionally admit at most 10,000 authenticated requests and 300 mutations globally per UTC day, with 240 requests per minute; per-user bounds are 4,000 requests, 100 mutations and 120 requests per minute. Authentication and rejected unauthenticated traffic still incur provider work outside these counters. Store bounds remain 32 projects, 128 versions, 128 datasets, 256 jobs, 8 MiB metadata and 64 MiB reserved live artefacts, with 16 MiB per artefact. Hitting retention capacity requires deliberate operator review, not deletion of evidence to reset execution allowances.

For inspection, use `node platform/firebase-audit.mjs --project reflective-graph-f1cd4` and `node platform/cloud-admin.mjs runtime --project reflective-graph-f1cd4`. The first selects safe fields only; raw Auth configuration can contain password-hash parameters and must not be printed. The second reports runtime limits, queue/scheduler state, retention and runtime project roles. `logs` reports scoped service errors. These read-only inspections supplement, rather than replace, acceptance.

To stop new work, run `node platform/cloud-admin.mjs disable --project reflective-graph-f1cd4`. This sets the persisted pilot kill switch: authorised reads/exports remain available; new mutations and worker execution stop. It does not terminate an already admitted request. `enable` reopens admission; follow with `reconcile` to recover queued work. Never delete the queue or database as an emergency stop.

Backups are created at most once per UTC day by successful reconciliation, with hash-checked referenced artefacts and a compressed-archive digest. They expire after seven days; Storage soft deletion retains them for a further seven days. Thus nominal recovery-point age is up to one day plus scheduling delay, not fifteen minutes. The build-source bucket also expires archives after seven days; immutable live artefacts do not expire automatically. Daily backups do not export Firebase account credentials. Download important user source/traces separately, and preserve account identity when recovering project access.

With explicit permission to copy private pilot data locally, `node platform/cloud-admin.mjs backup-download --project reflective-graph-f1cd4` downloads the newest snapshot into ignored `output/hosted/`, refuses an existing output, and verifies its advertised digest. If the operator lacks object-read permission, `backup-reader` grants that logged-in operator a one-hour, backup-prefix-only reader binding; use `backup-reader-remove` immediately after download, including on failure. Both commands require the same explicit `--project reflective-graph-f1cd4` argument. These permissions do not grant access to other projects or public readers. Then run `node platform/restore-backup.mjs --archive DOWNLOADED_FILE --directory NEW_EMPTY_PATH`. The destination must not exist; the tool checks archive bounds, entity invariants, artefact hashes and native histories before creating an identity-matched local store. It never overwrites the live service. Restored queued/running jobs become interrupted. Production re-import is a separate supervised migration, not an automatic overwrite command. Do not commit a backup or account/token material.

Release procedure: commit the intended source, generate `output/hosted-source.tar.gz` with `git archive --format=tar.gz --output=output/hosted-source.tar.gz HEAD`, then use `cloud-admin.mjs build` with the explicit project. `inspect` reports progress. `deploy` refuses any successful build not matching the exact archive digest; the service uses an immutable image digest, not a mutable tag. Build the frontend with `npm run build --prefix frontend`, then run `DEBUG= FIREBASE_CLI_DISABLE_UPDATE_CHECK=true node platform/node_modules/firebase-tools/lib/bin/firebase.js deploy --only firestore:rules,storage,hosting --config firebase.hosted.json --project reflective-graph-f1cd4 --non-interactive`. Hosting pins its API rewrite to the released Run revision. Local Docker is unnecessary.

Rollback preserves data: disable new work, retain the previous tested image/source identities, and redeploy its exact archived candidate through the same build/deploy path, followed by its matching frontend. The Firebase Hosting console also supports rolling back a prior release together with its pinned API revision. Verify the pinned rewrite and task target both use compatible service code before reenabling; Hosting-only rollback does not move untagged task traffic. Do not delete immutable versions or rewrite job engine digests. Old queued jobs with a different binary fail rather than silently executing changed semantics. The native engine in the deployed pilot retains the accepted binary digest.

For an authorised frontend-only recovery check, `node platform/cloud-admin.mjs rollback-check --project reflective-graph-f1cd4` refuses differing API pins, briefly releases the previous Hosting version and restores the current version in a `finally` block. The first-loop acceptance passed this check and an isolated backup restore. That evidence does not establish compatibility between arbitrary older backend versions; a failed restoration still requires operator inspection.

`node platform/hosted-acceptance.mjs --project reflective-graph-f1cd4` is an explicit low-volume live acceptance command, not part of ordinary CI. Run during a maintenance window: it temporarily pauses/resumes the project's task queue, creates disposable synthetic identities and retains immutable test histories. It verifies real signed identities, denied internal dispatch, project/role isolation, dataset compilation, duplicate submission, multi-batch continuation, pause/cancel and native replay; accounts are removed afterwards. Generated passwords/tokens stay in memory. The test administrator verifies its synthetic accounts, so successful execution does not claim delivery to a real inbox. Check queue state after any forced interruption. Local tests remain the safe default.

Google distinguishes [alerts-only budgets](https://docs.cloud.google.com/billing/docs/how-to/budgets) from [service-specific spend caps](https://docs.cloud.google.com/billing/docs/how-to/budgets-spend-caps). Spend caps cover one eligible service in one project, can incur delayed-enforcement overages and do not stop all persistent-resource charges. Verify the actual setting and remaining service costs before deployment. No account-wide billing change is authorised by the project configuration.

Admission reservations are scoped to one metadata store, not aggregated across separately configured stores or services. Existing store-wide capacity limits still apply; UTC allowance rollover does not free retained metadata or artefact capacity. Do not delete job records to recover allowance: a future retention implementation must preserve independent reservation accounting before removing history.
