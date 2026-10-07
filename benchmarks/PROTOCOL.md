# Reproducible evaluation

This file supplies the public protocol for new experiment collections. Historical collections retain their original protocol, source bundle, settings and hashes under `results/`; reproducing those results does not require the personal documentation archive. Mathematical definitions and claim boundaries are stated in the standalone dissertation and formal specification. Executable experiment matrices are defined by the collector scripts and recorded in each collection.

## Collection and provenance

Build the required native binaries from the intended commit before collecting results. Use the locked Rust dependencies and recorded toolchain. Give each collection a new output directory; collectors must refuse to overwrite retained evidence. Record the source commit, relevant source files and their digests, executable identity, environment, complete inputs, settings, output, exit status and instrument failures. Keep failed, censored and inconclusive runs distinct from successful runs. Do not silently replace recorded results after a software or protocol change.

Collectors using this protocol include it in their source inventory and, where applicable, retain a separate `protocol.md` and its digest. Source bundles preserve the paths used at collection time. Historical path allowances in verification scripts apply to the files inside those immutable bundles, not to missing files in a current checkout.

## Timing and memory

Performance collections run sequentially without concurrent builds, browser tests or other substantial workloads. Record hardware, operating system, runtime/toolchain versions, scheduling seed and execution profile. Keep construction, matching, admission, commit, recording and verification phases distinguishable; a region timing and whole-process resident memory measure different costs.

`scripts/benchmark.mjs` uses the native `measure` probe, the collector's declared matrix, five measured processes per cell and one warm-up. The macOS memory instrument is `/usr/bin/time -l`; retain its raw output and maximum resident bytes. The default process timeout is 30 seconds, and the sampled process-group ceiling is 512 MiB. Smoke mode uses one process per selected cell without full memory instrumentation and provides functional evidence only. Criterion measurements have a separate harness and exclude destruction of the retained execution object from the intended commit timing.

`scripts/scale-study.mjs` records its full size/family/phase matrix, five repetitions and one warm-up, with the same declared timeout and sampled memory ceiling. `scripts/recovery-study.mjs` records finite and modulating profiles for sizes 0, 1, 3, 10, 100 and 1000, with one diagnostic process per cell. Its structural and output checks do not turn a bounded modulating prefix into a convergence proof. Later large-job collections retain their own complete-job protocols, resource profiles and time/RSS results in their manifests.

Do not interpret timeout, memory censoring, instrumentation failure, search exhaustion or a step bound as measured normal completion. Report the denominator, exclusions, samples and variability. A functional rerun cannot replace a controlled performance measurement. Historical timings apply to their recorded sources and hardware.

## Semantic and application studies

Recovery, agreement and meta-activity collectors record their complete declared cases, native outputs and transition relations. Independent verifiers check ranks, bounds, counterexamples and table consistency under the stated finite domains. Complete analysis, unsupported input and resource-inconclusive analysis remain separate outcomes. The native matcher and validator are not independently proved by agreement with a transition-table oracle.

Intervention collections distinguish exact-occurrence from source-revision queries, replay retained events through native admission, and record every order-preserving subset within the declared prefix bound. The controlled campaign uses eighteen cells and three process repetitions. Alternative support within that bounded universe does not establish complete causality. Resource exhaustion cannot be classified as impossibility.

Minimality collections retain the selected carrier snapshots, native queries, translated queries, codecs and exhaustive finite gadget checks. Frozen-corpus reproduction and acceptance of an expanding example catalogue are separate obligations. Keep the original comparison domain, allocator/identity treatment and cost units; snapshot storage alone excludes decoder, evaluator and execution costs.

Live learning and interpretation comparisons first replay both recorded and current histories. Semantic comparison excludes only the well-formed top-level implementation fingerprint at explicitly named trace positions. The original histories retain that fingerprint; source hashes are not reassigned. Settings, identities, allocator, events, checkpoints, outcomes and every application field still require equality. This permits testing the same behaviour on a later executable without asserting byte-identical production provenance.

Learning and contextual-interpretation studies retain supplied examples, held-out cases, baselines, interventions, native traces and independent expected answers. Fit or select a policy before evaluating held-out cases. Correctness under a supplied convention does not establish intrinsic meaning or superiority to a complete hand-written baseline. Numerical, organisation, versioned execution, snapshot-cost and provenance studies likewise retain their source identities and explicit assurance boundaries in machine-readable results.

Cross-environment reproduction separates semantic agreement, executable-byte identity, trace identity and performance. Declare the equivalence being compared and all permitted normalisation. Do not remove an unexplained difference from the comparison. Tests of finite cases and abstract proof elaboration remain distinct from complete implementation verification.

## Verification and distribution

Use the verifier belonging to each result family; its default directory identifies the retained collection. Verification checks hashes, schemas, expected matrices, outcomes and independent oracles as applicable. `--live` repeats the supported native checks with the current build; it does not imply repeated timing. Proof checks require their pinned Lean/Mathlib dependencies. Refer to each script's usage and the adjacent result README for exact commands and supported modes.

Required collectors, verifiers, proof sources, fixtures and retained result bundles are part of the software distribution. The private `docs/archive/` tree is not a build, test or evaluation dependency. The submitted dissertation must state its definitions, methods and results directly, without requiring another project document to supply its argument.

### Complete candidate acceptance

Use the repository's pinned Rust toolchain and frontend Node/npm versions. Build the release CLI and examples before the research group. `node scripts/integrated-review.mjs collect GROUP NEW_DIRECTORY` records the complete `core`, `research`, `platform`, `web` or `proof` group, command logs and a source inventory. Verify each receipt with `node scripts/integrated-review.mjs verify GROUP DIRECTORY --current`. A failed attempt remains a failed receipt; repeat into a new directory after correction. These durations are functional acceptance evidence, not performance measurements. Platform and web groups require local listening ports and the documented emulator/browser dependencies.

For the proof group, install Lean 4.19.0 and set `LEAN_BIN` to its executable. Check out Mathlib commit `c44e0c8ee63ca166450922a373c7409c5d26b00b`, fetch its matching cache with `lake exe cache get`, and set `NUMERICAL_MATHLIB` to that checkout and `NUMERICAL_LEAN_BIN` to the Lean binary directory. `node scripts/check-proofs.mjs` elaborates all ten adopted proof sources and audits each theorem's axioms. The admission campaign separately checks executable abstract cases. These checks establish abstract results; they do not prove refinement of the Rust implementation.
