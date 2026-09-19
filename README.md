# A Minimal Self-Referential Graph Formalism

MSc Computer Science dissertation project exploring graph-resident rewrite rules, guarded rule modification, semantic stability, and causal order derived from rewrite-event dependencies. The planned software artifact is a Rust rewriting engine with a concrete syntax, CLI, WebAssembly bridge, property-based tests, reproducible traces, and a browser visualiser.

## Documentation

For the browser's concepts and practical views, read [Understanding the graph workbench](docs/guides/workbench.md). It explains data, rules, execution history, stopping conditions and large-dataset limits without requiring source-code knowledge.

The [example catalogue and capability audit](examples/README.md) indexes eighteen complete programs across all seven required research categories and the practical work-item profiles. It records exact run settings, expected results, independent evidence and limitations. The browser uses the same manifest: choose an example, apply its settings, then load and run it.

New to the project? Start with [`docs/explainers/simple.md`](docs/explainers/simple.md) — a plain-language explanation of what the work is and where it stands, written for a general reader rather than a marker.

- [`docs/project-proposal.md`](docs/project-proposal.md) — approved research baseline and fixed six-sprint timeline.
- [`docs/dissertation.md`](docs/dissertation.md) — condensed, standalone submission paper covering the formalism, software and research evidence.
- [`docs/formalism.md`](docs/formalism.md) — normative mathematical definitions and open proof obligations.
- [`docs/architecture.md`](docs/architecture.md) — technical architecture implementing the formalism.
- [`docs/research-plan.md`](docs/research-plan.md) — RQ-to-evidence plan and live evidence log.
- [`docs/sprint-plan.md`](docs/sprint-plan.md) — high-level plan retaining the proposal's original sprint numbers and dates.
- [`docs/task-list.md`](docs/task-list.md) — granular execution checklist within those fixed sprints.
- [`docs/decisions.md`](docs/decisions.md) — append-only architecture decision record.

Supporting material is grouped by purpose:

- **Research:** [evaluation and scalability](docs/research/evaluation.md), [reflective-recovery methods and experiments](docs/research/reflective-recovery.md), and [claim/evidence audit](docs/research/claim-audit.md).
- **Reference:** [worked examples](docs/reference/worked-examples.md), [lambda encoding](docs/reference/lambda-encoding.md), [trace format](docs/reference/trace-format.md), [workbench command contract](docs/reference/workbench.md), and [figure descriptions](docs/reference/figures.md).
- **Guides:** [using and understanding the workbench](docs/guides/workbench.md); the local `docs/explainers/` folder retains the simple explanation, terminology and reading notes.
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

Completed implementation packages and their evidence are recorded in [research-plan.md](docs/research-plan.md); component contracts are in [architecture.md](docs/architecture.md), and remaining work is in [task-list.md](docs/task-list.md). Sprint 5 supplies source integration, controlled native measurements, complete library acceptance and rule breakpoints. Sprint 6 uses the completed audit/synthesis as a baseline for independent research cycles, scaling and a developer platform with a versioned API. The browser's Projects interface connects to a [local developer API](platform/README.md) for immutable program versions, bounded asynchronous jobs, fork/retry provenance, metadata comparison and verified graph/history inspection. Firebase remains the selected hosted direction; cloud adapters, multi-user identity and distributed execution are not delivered. Workflow, data-quality and AI applications remain separate exploration avenues. The first cycle reproduced raw-search exhaustion at one thousand items and implemented the local API/job contract; [the research plan](docs/research-plan.md#first-path-reproducible-execution-and-the-developer-api-boundary) records the subsequent investigations and their scope. Sprint 7, 2–13 October, owns final figures, verification and packaging. Proposed improvements are not implemented capabilities. The explicit schema remains an input-contract trade-off, not a globally smallest representation.

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

### Core interfaces

Sprint 3 adds repeatable rule selection, bounded graph comparison and automated tests that generate varied cases and shrink failures into smaller examples. This found a slow comparison on a nested rule package; the improved comparator and the reduced failing case are retained.

The [behavioural examples](examples/behaviour/README.md) use the same test for useful content throughout: a value link between two distinct objects. One program finishes while keeping that link, another finishes after deleting it, and a third keeps it while alternating between two graph states. The last example has a mathematical argument for continuing indefinitely; a finite computer run alone does not prove that. Native committed history, positive-resource dependencies and exact local conflict analysis are implemented. Portable trace import/export, checked replay and resumable logical runs are implemented. The browser interface provides linked views and verified history inspection; the expressiveness study remains later work.

`graph_core::ValidatedGraph` gives a read-only validated view tied to a graph borrow. `Graph::insert_batch_with_limits` resolves forward and cyclic references atomically; construction is distinct from full validation. `GraphLimits` bounds input bytes, atoms, fields, labels, package depth and validation work.

`engine::Execution` owns the committed graph. Submit an `OperationalBinding` to `prepare`, inspect the resulting `PreparedStep`, then pass it to `commit`. Ground rule/match data are checked and read-only. A candidate becomes stale after any committed step or successful `replace_graph`, even when the graph looks unchanged. External replacement starts a new execution history. `Execution::view()` decodes the current graph afresh; `live_rules()` includes inactive representations, while `active_rules()` excludes them and quoted heads. Neither iterator enumerates enabled matches.

`Execution::bindings(source)` enumerates raw matches; `enabled()` examines every active source and returns fully checked candidates plus expected rejections. `enabled_for(source)` restricts that query to one source. Select a candidate from `into_steps()` and pass it to `commit`. For cumulative queries, reuse one `Execution::search()`. Interrupted searches return errors, not partial or empty enabled sets. Only a successful empty all-source query establishes absence of an enabled step. `apply_with_binding` remains the direct reference path. Native run/check commands use this checked boundary; shared deterministic/seeded scheduling and bounded observation are provided by engine::runner and engine::observation.

The suite currently passes 311 unit/integration tests plus thirteen documentation tests, including eight compile-fail API checks. See [architecture §20](docs/architecture.md#20-checked-construction-and-execution-interfaces) for defaults, error categories and the source-level commit argument. The parser uses pinned Logos 0.16.1; engine and parser remain independent production siblings.

The shared library provides `parser::lambda::{parse, Term, Node, Limits, encode, readback, interpret}` for closed weak-head lambda evaluation through graph-resident rules. Use `cargo run -p cli -- lambda-compile examples/lambda/identity.lambda` to translate source, or open **Lambda program** in the browser, prepare its graph and load it. The result panel follows the timeline. [Examples and hand reductions](examples/lambda/README.md) and the [encoding contract](docs/reference/lambda-encoding.md) explain administrative steps and limits. Run `cargo test -p cli --test lambda --locked` for independent source/graph checks. The [controlled performance study](docs/research/evaluation.md#performance) retains native time/memory data and distinguishes 100-item completion from 1,000-item search exhaustion.

For a practical browser walkthrough, select `practical/work-items`, load it and run: ten pending items become processed. `practical/policy-enable` first changes the processing rule's activation and revision, then processes those items. The [Guide](docs/guides/workbench.md) is available inside the application and explains the timeline, linked views, themes and source/project/trace files. [Dataset recipes and tested limits](examples/practical/README.md) distinguish successful processing from a resource-limited attempt. Sprint 4 includes expanded acceptance of the local workbench. Sprint 5 retains the RQ5 expressiveness study, controlled time/memory measurements and final-library integration.
