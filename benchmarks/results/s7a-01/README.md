# Consolidation acceptance and candidate remeasurement

The `*-final-verified` directories retain the first local acceptance candidate, now superseded by the trace-memory correction. Their shared source-inventory SHA-256 is `8aa4d04d48586603665d9d7e50b6a8d4eb6276d75300c912106cc0253e2eb18a`, based on the working candidate after baseline commit `9285edb8bbe194d6c6c18fda875e28d76985b863`. The preceding receipts cover inventory `e310ec917ee93d33070f39133b4864c21a79f46b050b8c27e09cdbd5aca39033`, before the Guide-fragment repair and anchor validation. A source inventory is not a commit or a signature. These receipts do not establish GitHub CI, hosted deployment acceptance, publication permission or a substantive freeze.

Current local acceptance consists of 49 successful commands in `../s7a-04-core/`, `../s7a-04-research/`, `../s7a-04-proof/`, `../s7a-04-web/` and `../s7a-04-platform/`, all on source inventory `89227fafa4b13aaff80a35f4029f199439b0b619cfe773f6928fbfa96e524de6`. The research inventory additionally verifies the failed initial measurement and successful corrected measurement. Earlier successful `s7a-02-*` and `s7a-03-*` receipts precede the archive-check portability and dependency-notice corrections. The unsuccessful `../s7a-02-proof/` invocation lacked the required Mathlib environment; its separately retained rerun supplies that environment and passes both commands.

| Group | Commands | Scope |
|---|---:|---|
| core | 9 | Formatting, strict Clippy, 511 Rust tests, documentation, locked native builds and 23 catalogue profiles |
| research | 31 | 117 script tests, historical/live comparisons, all adopted applications and a distinct current minimality collection |
| proof | 2 | Executable admission proof campaign and 82 abstract statements across ten pinned proof sources |
| web | 5 | Build/type checking, lint, 48 units, 25 parity tests and 33 Chromium tests |
| platform | 2 | 49 local service tests and six isolated Firebase emulator tests |

Two ignored Rust routines generate fixtures; ordinary tests verify the retained fixtures instead. The proof scope is abstract and does not establish native refinement. Node 25.4.0/npm 11.7.0 were supplied by a checksum-verified isolated runtime because the machine's package-manager installation was unusable. The platform also supports the separately tested Node 24 runtime. The emulator uses only the `demo-reflective-graph` namespace.

From the repository root, verify the current complete receipts:

```sh
for group in core research web platform; do
  node scripts/integrated-review.mjs verify "$group" "benchmarks/results/s7a-04-$group" --current
done
node scripts/integrated-review.mjs verify proof benchmarks/results/s7a-04-proof --current
node benchmarks/results/s7a-01/verify-measurements.mjs
node scripts/verify-loop-b2.mjs benchmarks/results/s7a-02-scale
```

The public [evaluation protocol](../../PROTOCOL.md) documents collection and pinned proof setup. Earlier receipt directories remain as failed or superseded attempts during the JSON-corpus repair and subsequent corrections. They retain trace-producer comparison and immutable-reference-import failures, local emulator port collisions, and runs invalidated by source edits during acceptance. A receipt with `complete:false` is never a pass.

## Representation reproduction

The original minimality collection remains unchanged: 69 snapshots, 477 native cases and 1,431 translated queries. The new collection in `research-final-verified/studies/minimality-current/` has 73 snapshots, 513 native cases and 1,539 translated queries. It includes the authored versioned JSON request. An earlier collection of these same candidate semantics in `research-accepted/studies/minimality-current/` was extracted into an empty directory: all 244 source files built with locked dependencies, and its fresh executable reproduced every native snapshot and case. Neither procedure imports the personal documentation archive.

## Archive-free builds and browser rehearsal

An uncommitted candidate archive, SHA-256 `9f0a7b1ab204aa385e2ce5f5b1cd5821bc5df5784bcbbb43c885bfa861bb1071`, was extracted into a fresh directory without the personal archive, dependency directories or build products. Locked debug and release native builds succeeded; 114 applicable script tests passed, two personal-archive checks were explicitly skipped, and all 23 catalogue profiles passed validation and replay. This candidate predates the final Guide-fragment correction; it is not the release package. An earlier clean extraction also completed dependency installation and production WebAssembly/frontend compilation.

Local production-browser rehearsal completed the eight-stage order walkthrough, version-aware execution in five rewrites and lambda erasure in six rewrites with identity-function readback. The Guide's stated controls match those interactions. This does not establish hosted sign-in, durable-job acceptance or completion of the full examiner journey.

The corrected source archive has SHA-256 `a56a9c5861fe49aaf5f7d7e833178b686b1e133b8dc70d87446c57ea2c0f6634` and contains 2,944 files without Git metadata, the personal archive or build products. Its fresh extraction rebuilds locked debug/release native targets and production WebAssembly/frontend assets, including dependency notices. All 115 applicable script tests pass, with only two private-document checks skipped; all 23 catalogue profiles pass, and all 82 abstract proof statements elaborate using the separately pinned dependencies. Native cross-feature, organization, numerical, action-learning, snapshot-cost and selected-provenance checks also pass. `../s7a-04-clean/receipt.json` records the scope and log hashes. Later evidence/planning annotations do not alter that archive identity; it is a tested candidate package, not an approved release.

A local Projects rehearsal creates an immutable version, completes its two-event job at NormalForm, downloads a trace independently accepted by the native verifier, imports it into the WASM workspace, inspects state zero and returns to the persisted project. Disposable data stay outside the distributed evidence. This confirms the local fallback, not the real hosted candidate.

## Controlled measurements

Measurements ran serially after builds and functional campaigns had finished. Hardware and resource profiles are retained in each collection. Existing user applications were not terminated; system-level conditions are not claimed to be identical to the historical experiments.

`scale/` retains 100 processes across 25 cells, including warm-ups. Ninety-six processes finish, while all four 100,000-item complete-pipeline processes cross the sampled 6-GiB ceiling and are killed. The 10,000-item measured median is 2.043 seconds with 1,079.09 MiB peak RSS. The historical 100,000-item gate therefore does **not** reproduce in this environment. `verify-measurements.mjs` checks the retained censored outcomes explicitly; the ordinary all-gates-passed verifier must reject this collection.

`scale-diagnostic/` retains one bounded process for each of the reconstructed historical and current probes. Both cross the same ceiling. This is a diagnostic, not a repeated performance experiment: it prevents attribution solely to current code changes but does not establish a unique cause. The script records its temporary executable paths; reconstruct the historical probe from `../s6-loop-b2/sources.json.gz` before repeating it, and retain any repetition under a new identity.

`public-data/` retains a full 12,330-row transformation through the isolated large-local service: one warm-up, three fixed-policy measurements and one graph-approved control. All five finish, preserve the eighteen input fields, pass the independent complete CSV oracle and export checked history. Fixed-policy times are 10.517, 10.600 and 10.574 seconds; the guarded control takes 12.011 seconds. The raw local job stores remain outside the distribution; the source bundle, protocol, digests and checked output metadata are retained here. Input is the UCI Online Shoppers Purchasing Intention Dataset (Sakar and Kastro, 2018), CC BY 4.0, archive SHA-256 `2972e6184d3ad7beaaa831d9fc2b059dc3ee29df69d1ec593c466a5cd8485d14`. The [UCI dataset page](https://archive.ics.uci.edu/dataset/468/online+shoppers+purchasing+intention+dataset) supplies provenance and the download.

`../s7a-02-scale/` repeats all 25 cells and 100 processes after replacing allocation-heavy trace validation with streaming structural checks, borrowed canonical comparison and bounded continuation retention. All processes pass under the unchanged 600-second/6-GiB protocol. At 100,000 items the measured median is 22.780 seconds and 5,857.97 MiB RSS; observed ranges are 22.726–23.020 seconds and 5,849.53–5,859.64 MiB. All 100,000 events and observations remain retained and replay-verified. The engine source fingerprint is `392a941cc7e3c566bddb4b2d5b3dc0fc7e8bf21b4772a772cc21341a1367a348`.

`../s7a-02-public-data/` repeats the five complete public-data jobs with that implementation. Fixed-policy measurements are 10.848, 10.928 and 10.843 seconds, with sampled native peaks 671.23, 696.44 and 677.23 MiB. The guarded control takes 12.121 seconds. Every job completes with the expected commit count and complete independent CSV oracle under the unchanged 120-second/2-GiB limits. Source-bundle SHA-256 is `dc564fd8ab90f9dbc514446befa6923f058c6e16a733e923dad730458dbef774`; the native executable is `189a771cf7cf774fe20a8c0c10ed270f0a6b2e5053f1a83f02f43d782bd42b2b`. Raw disposable job stores are excluded from distribution. This recollection does not repeat the historical browser latency or recovery campaign.

Reproduction requires the declared toolchains, input archive and explicit native/local allowances. These measurements are not hosted-capacity claims, isolated optimisation speedups or evidence of universal scalability. The manuscripts distinguish the unsuccessful initial large-history replication from the successful corrected implementation.
