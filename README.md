# A Minimal Self-Referential Graph Formalism

MSc Computer Science dissertation project exploring graph-resident rewrite rules, guarded rule modification, semantic stability, and causal order derived from rewrite-event dependencies. The software includes a Rust engine, concrete syntax, CLI, WebAssembly workspace and a hosted developer platform with durable projects, native jobs and verified traces.

Open the [hosted application](https://reflective-graph-f1cd4.web.app/#projects), create an email account and verify your address before signing in. The [user guide](docs/guides/workbench.md) explains both hosted and local use; the [platform guide](platform/README.md) documents the API, pilot limits and recovery.

## Documentation

For version-aware execution, select `reflection/versioned` in the browser or use `cli version-compile examples/reflection/versioned.json`. The graph publishes a replacement while existing calls retain their captured versions. `cli version-readback` reports actual installed descriptions and returned items. [The example contract](examples/reflection/README.md) explains its two supported implementations, local step budget, scheduling and replay.

The optional [local calendar learner](platform/README.md#local-learned-calendar-proposals) learns a bounded cleaning policy from examples, exposes it for review and executes explicitly promoted graph rules with checked traces. The [context interpreter](platform/README.md#contextual-interpretation) additionally distinguishes calendar, fiscal and not-applicable meanings using tables inside the graph. Neither uses a model provider. The [evaluation](docs/research/learning.md) separates operational interpretation from intrinsic meaning and records the lack of advantage over correct fixed policies.

Optional [adaptive workflow contracts](platform/README.md#adaptive-workflows-with-stability-contracts) check a restricted class of graph-resident routing rules and run persistent tasks across body revisions, including bounded task branching. The native CLI/API and browser Projects panel report completion separately from normal form, resource limits and unsupported inputs. The workflow panel is available in hosted Projects and through the local API; its results are ephemeral rather than saved jobs.

The Rust [organization application](docs/research/learning.md#acquired-representations-and-renewable-organization) combines acquired predictions, renewable checking, recognition revision and inheritance. Run `cargo run --locked -p cli -- organization-run examples/organization/renewal.json --output /tmp/organization-report.json` with a new output path. The checked request runs a parent and child, then executes the child's action table through admitted graph rewrites. The report contains complete states, measurements and a portable native trace. No Python runtime or research directory is required. Population transitions remain a distinct application law; they are not relabelled as graph-admission events.

For the browser's concepts and practical views, read [Understanding the graph workbench](docs/guides/workbench.md). It explains data, rules, execution history, stopping conditions and large-dataset limits without requiring source-code knowledge.

The [example catalogue and capability audit](examples/README.md) indexes nineteen complete programs across all seven required research categories, the practical work-item profiles and version-aware interpretation. It records exact run settings, expected results, independent evidence and limitations. The browser uses the same manifest: choose an example, then load and run it. Its settings load automatically unless the user has overridden them.

The [Rust numerical application](docs/reference/numerical-components.md) computes workflow success probabilities with checked error bounds and replayable native publication. `cli numerical-run examples/numerical/workflow.json` uses the existing engine and recorder. It distinguishes certified, uncertified and stale answers while tasks and inputs change. Arithmetic is host-side Rust computation; there is no Python runtime, browser panel or hosted endpoint.

New to the project? Start with [`docs/explainers/simple.md`](docs/explainers/simple.md) — a plain-language explanation of what the work is and where it stands, written for a general reader rather than a marker.

- [`docs/project-proposal.md`](docs/project-proposal.md) — immutable historical research baseline, restored byte-for-byte from initial commit `04cfd35`. Do not edit its text, references, links or formatting; subsequent findings and corrections belong in maintained documentation.
- [`docs/dissertation.md`](docs/dissertation.md) — condensed, standalone submission paper covering the formalism, software and research evidence.
- [`docs/formalism.md`](docs/formalism.md) — normative mathematical definitions and open proof obligations.
- [`docs/architecture.md`](docs/architecture.md) — technical architecture implementing the formalism.
- [`docs/research-plan.md`](docs/research-plan.md) — RQ-to-evidence plan and live evidence log.
- [`docs/research/loop-plans.md`](docs/research/loop-plans.md) — Loops B–N: forward research questions, alternatives, experiments and acceptance criteria; task status remains in the task list.
- [`docs/sprint-plan.md`](docs/sprint-plan.md) — high-level plan retaining the proposal's original sprint numbers and dates.
- [`docs/task-list.md`](docs/task-list.md) — granular execution checklist within those fixed sprints.
- [`docs/decisions.md`](docs/decisions.md) — append-only architecture decision record.

Supporting material is grouped by purpose:

- **Research:** [evaluation and scalability](docs/research/evaluation.md), [reflective-recovery methods and experiments](docs/research/reflective-recovery.md), and [claim/evidence audit](docs/research/claim-audit.md).
- **Reference:** [worked examples](docs/reference/worked-examples.md), [lambda encoding](docs/reference/lambda-encoding.md), [trace format](docs/reference/trace-format.md), [workbench command contract](docs/reference/workbench.md), and [figure descriptions](docs/reference/figures.md).
- **Guides:** [using and understanding the workbench](docs/guides/workbench.md) and the maintained [plain-language explanation](docs/explainers/simple.md). Other material under `docs/explainers/`, including personal reading notes and terminology, remains local.
- **Archive:** the maintained [full-length dissertation](docs/archive/dissertation-full.md), its [maintenance policy](docs/archive/README.md), and the [annotated literature map](docs/archive/literature.md).

Keep the eight core documents at `docs/` level. Extend the relevant research collection for subsequent cycles instead of adding a separate study document each time. Contracts belong under `reference/`, user instructions under `guides/`, and operational progress in the research/sprint/task plans. Frozen experiment protocols and source bundles remain with their original results; consolidation does not rewrite historical evidence.

## Build and check

Install Rust through rustup; `rust-toolchain.toml` pins Rust/Cargo 1.85.0. Native commands do not require Node. The browser uses Node 25.4.0 and npm 11.7.0.

The engine-backed browser workspace supports editing, stepping/running, rule inspection and verified trace import/export. Its build and lifecycle contract is [docs/reference/workbench.md](docs/reference/workbench.md).

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.85.0
cd frontend
npm ci
npm run dev
```

Use npm run build and npm run preview for the production bundle. Native checks below run from the repository root.

```sh
cargo test --workspace --locked
cargo run --locked -p cli -- demo
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
```

`demo` executes graph-resident SelfLower from rank one to zero. Test ledgers live in `examples/fixtures/`; they are separate from the authored `.graph` language. CI executes the documented checks on pushes; acceptance must refer to the delivery commit rather than an earlier passing build.

The repository's existing ignore policy for local documentation is preserved. Builds and tests use the source fixtures directly and do not require dissertation or explainer files.

Completed work and evidence are recorded in [research-plan.md](docs/research-plan.md), component contracts in [architecture.md](docs/architecture.md), and task status in [task-list.md](docs/task-list.md). The first Sprint 6 loop, now called Loop A, delivers the bounded Firebase developer platform, restricted reflective recovery, condition-sensitive provenance and further scalability/reproducibility experiments. It does not establish unrestricted convergence, global minimality or million-item execution. [Loops B–N](docs/research/loop-plans.md) specify the subsequent scaling, application and research investigations, starting with complete execution and profile compatibility. Sprint 7, 2–13 October, owns final figures, verification and packaging; closing one improvement loop does not complete those tasks or later research cycles.

## Run an authored program

```sh
cargo run --locked -p cli -- check examples/programs/arithmetic.graph
cargo run --locked -p cli -- run examples/programs/arithmetic.graph --max-steps 1
cargo run --locked -p cli -- run examples/programs/self-modify.graph --max-steps 3
cargo run --locked -p cli -- run examples/programs/mutual-regress.graph --max-steps 1 --rule p
cargo run --locked -p cli -- --help
```

`check` means “this graph is valid”, not “nothing can run”. Arithmetic reaches a non-empty `NormalForm` in one step (exit 0). The self-modification run reaches `StepLimit` (exit 3). Requested P is rejected by guardedness (exit 4); without `--rule`, C1 has no enabled step and reports `NormalForm`. A limit never proves divergence.

Use `-` instead of a filename to read standard input. Output is loadable graph text with outcome, committed-step, selection-policy and limit comments; diagnostics go to stderr. `--output new.graph` publishes a complete new file and refuses to overwrite an existing destination. Input/output failure codes are documented in [architecture §21](docs/architecture.md#21-native-command-interface). Check the exit code before consuming stdout: an I/O failure may interrupt a write. Loading or final-report size failure publishes no graph.

Every run requires `--max-steps`, including zero. `--max-candidates` and `--max-work` bound each complete search; graph and input bounds are also configurable. The input-byte limit covers the final report too. Selection uses the shared deterministic or seeded scheduler in explicit identity-relative candidate order; neither fairness nor renaming invariance is implied. `--rule` restricts execution to one initially named head and cannot establish global normality. JSON trace export, validation and checked replay are implemented; see the [trace contract](docs/reference/trace-format.md).

See the [example guide](examples/programs/README.md) for independently derived expected states, explicit witness selections and research limits. Printing/reloading preserves graph structure, not source names, allocation history or an execution trace.

## Concrete syntax

`parser::parse(source)` reads a complete `program { ... }` into a source-located syntax tree. It supports all atom kinds, explicit variable bindings, named rule metadata, L/R sides and nested RHS packages. It reports multiple syntax errors and bounds input, tokens, diagnostics and nesting. It checks syntax only. Use `parser::load(source)` or `load_with_limits` to resolve names, expand headers and packages, and construct a fully validated `LoadedProgram`. The loaded graph and its source origins are read-only; `into_graph` transfers the buffer to an engine consumer. `parser::print(graph)` produces explicit, readable atom declarations that can be loaded again without losing carrier structure; it does not retain comments, authored names or execution history.

See [the grammar](docs/architecture.md#9-concrete-syntax-and-parsing) and [the installer syntax specimen](examples/syntax/install-wrap.graph). `parser::parse_with_limits` accepts explicit settings. The original `.atoms` ledger format remains a separate test-input path.

## Checked execution

### Native history and local conflicts

Enable `Runner::enable_recording(RecordingLimits::default())` at state zero to keep a committed history. For explicit checked selections, use `Recording::new(&execution, limits)` and `recording.commit(&mut execution, prepared)`. Read `recording.events()` and `recording.atom(id)` to inspect changes, including deleted atoms. `DependencyOrder::extract(events, AnalysisLimits::default())` supplies resource witnesses and bounded strict reachability; `LocalSnapshot::analyse(&execution, limits)` checks alternatives in both orders without changing the execution. The recording module includes a tested API example.

In simple terms, the system now keeps a record of what each rewrite used and changed, explains which recorded steps depend on earlier ones, and checks whether two available rewrites interfere. Shared reading alone does not force an order. This is not a complete explanation of every enabling condition: making hidden rule data visible is a tested exception. No dependency arrow does **not** mean “safe to reorder”; local residual checking is separate. Files can now be exported, checked and replayed; browser history controls inspect that same verified record.

### Save, check and revisit a run

```sh
cargo run -p cli -- export examples/behaviour/stable.graph --max-steps 1 --output stable.trace.json
cargo run -p cli -- trace-check stable.trace.json
cargo run -p cli -- replay stable.trace.json --at 1
```

A trace saves the starting graph and what each rewrite did. Loading it checks the steps again instead of trusting the file. Checkpoints let the engine revisit earlier states while keeping the right identity numbers. A paused run can continue with the same random choices and memory of earlier states; simply looking at an old state does not change the original run. Existing output files are never overwritten. A saved run may have stopped at a limit, so successful file validation does not mean the program finished its work. See the [full contract](docs/reference/trace-format.md) and [annotated example](examples/traces/README.md).

### Explain a short recorded history

```sh
cargo run -p cli -- trace-explain stable.trace.json --event 0 --query exact
cargo run -p cli -- trace-explain stable.trace.json --event 0 --query source-revision
```

Use the zero-based index of an event present in your trace. `exact` asks whether that particular occurrence remains admitted after selected earlier events are omitted; `source-revision` asks whether that rule version has any admitted match. Every retained prefix step is replayed through native admission. Reports distinguish impossible prefixes, unavailable targets and rejected targets, and retain alternative minimal successful selections. The default prefix limit is eight; `--max-prefix` can raise it to twelve. Cost grows exponentially. This is a separate offline explanation, not general causality or a browser feature; the [research account](docs/research/reflective-recovery.md#condition-sensitive-interventions-loop-g) states its precise limits.

### Reproducing research results

Run `node scripts/verify-reproduction.mjs --check benchmarks/results/reproduction-m/report.json` to check the retained cross-environment evidence without compiling or executing archived source. For a fresh reconstruction of the reviewed experiment, use `node scripts/reproduce-frozen.mjs benchmarks/results/reproduction-m-macos-02 NEW_OUTPUT` after the [documented tool provisioning](benchmarks/results/reproduction-m/README.md). The study distinguishes exact execution/proposal agreement, controlled same-target binary identity, cross-host WASM differences and measured timing/RSS. A seed alone does not establish all these properties, and a hash does not authenticate an unknown source bundle.

### Core interfaces

The [assurance investigation](docs/research/assurance.md) includes Lean-checked owned-field mutation/reference-search properties and an independent native admission audit. Reproduce the finite native campaign with `cargo build --release --locked -p engine --example admission_audit` and `node scripts/admission-study.mjs verify benchmarks/results/admission-01 --live`; add `--proof` with Lean 4.19.0 installed to check proofs and both campaigns. This is scoped assurance, not verification of the entire Rust engine.

Sprint 3 adds repeatable rule selection, bounded graph comparison and automated tests that generate varied cases and shrink failures into smaller examples. This found a slow comparison on a nested rule package; the improved comparator and the reduced failing case are retained.

The [behavioural examples](examples/behaviour/README.md) use the same test for useful content throughout: a value link between two distinct objects. One program finishes while keeping that link, another finishes after deleting it, and a third keeps it while alternating between two graph states. The last example has a mathematical argument for continuing indefinitely; a finite computer run alone does not prove that. Native committed history, positive-resource dependencies and exact local conflict analysis are implemented. Portable trace import/export, checked replay and resumable logical runs are implemented. The browser interface provides linked views and verified history inspection; closed and explicit-context open weak-head evaluation provide the scoped expressiveness result.

`graph_core::ValidatedGraph` gives a read-only validated view tied to a graph borrow. `Graph::insert_batch_with_limits` resolves forward and cyclic references atomically; construction is distinct from full validation. `GraphLimits` bounds input bytes, atoms, fields, labels, package depth and validation work.

`engine::Execution` owns the committed graph. Submit an `OperationalBinding` to `prepare`, inspect the resulting `PreparedStep`, then pass it to `commit`. Ground rule/match data are checked and read-only. A candidate becomes stale after any committed step or successful `replace_graph`, even when the graph looks unchanged. External replacement starts a new execution history. `Execution::view()` decodes the current graph afresh; `live_rules()` includes inactive representations, while `active_rules()` excludes them and quoted heads. Neither iterator enumerates enabled matches.

`Execution::bindings(source)` enumerates raw matches; `enabled()` examines every active source and returns fully checked candidates plus expected rejections. `enabled_for(source)` restricts that query to one source. Select a candidate from `into_steps()` and pass it to `commit`. For cumulative queries, reuse one `Execution::search()`. Interrupted searches return errors, not partial or empty enabled sets. Only a successful empty all-source query establishes absence of an enabled step. `apply_with_binding` remains the direct reference path. Native run/check commands use this checked boundary; shared deterministic/seeded scheduling and bounded observation are provided by engine::runner and engine::observation.

The workspace suite covers unit/integration tests, documentation examples and compile-fail API checks. See [architecture §20](docs/architecture.md#20-checked-construction-and-execution-interfaces) for defaults, error categories and the source-level commit argument. The parser uses pinned Logos 0.16.1; engine and parser remain independent production siblings.

For optional recovery analysis, run `cargo run --release --locked -p cli -- stability-check examples/practical/revise-guard.graph examples/practical/revise-guard.contract.json` from the repository root. This checks every reachable transition up to exact full-carrier isomorphism, including graph-resident body changes. Exit codes are 0 for certification, 7 for a counterexample, 3 for resource-inconclusive analysis and 2 for invalid input. It does not change ordinary execution or expose an unbounded hosted service. [Contracts, methods and limitations](docs/research/reflective-recovery.md#body-changing-recovery-loop-d) include reproduction commands and retained failures.

For changing rules that agree on a final result, use `stability-check examples/practical/repair-policy.graph examples/practical/repair-policy.contract.json`. A graph rule converts the alternative audited result into done. Its v2 contract checks every reachable schedule, not just a selected run. The separate `repair-statuses INPUT CONTRACT pending` command restores missing/duplicate tracked statuses to pending and emits a verified reconstruction receipt and new baseline; it does not mark work done or repair arbitrary corrupt logic. [Agreement and fault-domain evidence](docs/research/reflective-recovery.md#agreement-and-disturbances-loop-e) explains the guarantees and includes all reproduction commands.

To check whether rule changes eventually cease, run `cargo run --release --locked -p cli -- meta-check examples/practical/credit-spawn.graph finite-edits none`. It certifies at most two editing transactions for that program, without imposing termination on ordinary work. Other modes are `edit-burst`, `meta-invocations`, `meta-burst` and `productive`; the last requires a recovery-contract file in place of `none` and can permit continuing rule changes after output stabilises. Reports include exact finite-quotient potentials or cycle witnesses. Exit codes are 0 certified, 7 refuted, 3 resource-inconclusive and 2 invalid input/domain. These are optional native analyses, not new runtime guards. [Meta-activity research and reproduction](docs/research/reflective-recovery.md#meta-activity-contracts-loop-f) explain the distinction and limits.

For unbounded inaccessible logging, try `cargo run --release --locked -p cli -- meta-abstract examples/practical/inert-log.graph finite-edits none Log`. It proves the same two-edit bound while allowing log growth. Analysis privately ignores only isolated `Log` constructors after checking every reachable active pattern and any declared payload contract. Referenced logs and all events remain; unsupported readers exit 2, not refutation. Reports are explicitly versioned `meta-activity-analysis-v2`. Runtime data and memory usage are unchanged; this does not handle arbitrary growing graphs.

The shared library provides `parser::lambda::{parse, Term, Node, Limits, encode, readback, interpret}` for closed weak-head lambda evaluation through graph-resident rules. The additive `OpenTerm`, `parse_open`, `encode_open` and `readback_open` APIs support symbolic inputs; `compile_profile` dispatches profiles. Use `--profile open-cbn-v1 --free database,select` with the [symbolic query](examples/lambda/open/README.md), or select Symbolic inputs in the browser. Use `cargo run -p cli -- lambda-compile examples/lambda/identity.lambda` to translate source, or open **Lambda program** in the browser, prepare its graph and load it. The result panel follows the timeline. [Examples and hand reductions](examples/lambda/README.md) and the [encoding contract](docs/reference/lambda-encoding.md) explain administrative steps and limits. Run `cargo test -p cli --test lambda --locked` for independent source/graph checks. The [controlled performance study](docs/research/evaluation.md#performance) retains profile-specific native time/memory evidence, including reference failures and separately measured framed complete jobs.

For a practical browser walkthrough, select `practical/work-items`, load it and run: ten pending items become processed. `practical/policy-enable` first changes the processing rule's activation and revision, then processes those items. The [Guide](docs/guides/workbench.md) is available inside the application and explains the timeline, linked views, themes and source/project/trace files. [Dataset recipes and tested limits](examples/practical/README.md) distinguish successful processing from a resource-limited attempt. Select an execution profile in Run settings before loading: `reference-v1` remains the default; `indexed-v2` and `framed-v3` are explicit checked alternatives. Their search/history accounting differs. CLI runs accept `--execution-profile framed-v3`; portable histories retain that choice. The native complete-job acceptance command is `cargo run --release --locked -p engine --example scale_probe -- fixed 1000 pipeline-framed 1000 42`. It includes recording, recurrence and verified replay under a declared native envelope, not the hosted service's limits.
