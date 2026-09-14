# A Minimal Self-Referential Graph Formalism

MSc Computer Science dissertation project exploring graph-resident rewrite rules, guarded rule modification, semantic stability, and causal order derived from rewrite-event dependencies. The planned software artifact is a Rust rewriting engine with a concrete syntax, CLI, WebAssembly bridge, property-based tests, reproducible traces, and a browser visualiser.

## Documentation

For the browser's concepts and practical views, read [Understanding the graph workbench](docs/workbench-guide.md). It explains data, rules, execution history, stopping conditions and large-dataset limits without requiring source-code knowledge.

New to the project? Start with [`docs/explainers/simple.md`](docs/explainers/simple.md) — a plain-language explanation of what the work is and where it stands, written for a general reader rather than a marker.

- [`docs/project-proposal.md`](docs/project-proposal.md) — approved research baseline and fixed six-sprint timeline.
- [`docs/dissertation.md`](docs/dissertation.md) — standalone academic account of the formalism, software and research evidence.
- [`docs/formalism.md`](docs/formalism.md) — normative mathematical definitions and open proof obligations.
- [`docs/worked-examples.md`](docs/worked-examples.md) — hand-traced constructions and counterexamples behind the formalism.
- [`docs/architecture.md`](docs/architecture.md) — technical architecture implementing the formalism.
- [`docs/research-plan.md`](docs/research-plan.md) — RQ-to-evidence plan and live evidence log.
- [`docs/sprint-plan.md`](docs/sprint-plan.md) — high-level plan retaining the proposal's original sprint numbers and dates.
- [`docs/task-list.md`](docs/task-list.md) — granular execution checklist within those fixed sprints.
- [`docs/decisions.md`](docs/decisions.md) — append-only architecture decision record.
- [`docs/literature.md`](docs/literature.md) — annotated literature map and working bibliography.

## Build and check

Install Rust through rustup; `rust-toolchain.toml` pins Rust/Cargo 1.85.0. Native commands do not require Node. The browser uses Node 25.4.0 and npm 11.7.0.

The engine-backed browser workspace supports editing, stepping/running, rule inspection and verified trace import/export. Its build and lifecycle contract is [docs/workbench.md](docs/workbench.md).

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

`demo` executes graph-resident SelfLower from rank one to zero. Test ledgers live in `examples/fixtures/`; they are separate from the authored `.graph` language. The CI workflow is configured, but remote execution is not yet confirmed.

The repository's existing ignore policy for local documentation is preserved. Builds and tests use the source fixtures directly and do not require dissertation or explainer files.

Sprint 1 and Sprint 2 local acceptance are complete, including native commands, authored examples and the integrated source-level correspondence audit. Research evidence is recorded in [`research-plan.md`](docs/research-plan.md), implementation boundaries in [`architecture.md`](docs/architecture.md), and remaining work in [`task-list.md`](docs/task-list.md). The explicit schema is a documented trade-off: a default-binding comparator uses one fewer declared tag with equal carrier costs. Full research adequacy remains pending. Sprint numbers and dates are unchanged; remote CI is reported separately from local verification.

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

Every run requires `--max-steps`, including zero. `--max-candidates` and `--max-work` bound each complete search; graph and input bounds are also configurable. The input-byte limit covers the final report too. Selection uses the shared deterministic or seeded scheduler in explicit identity-relative candidate order; neither fairness nor renaming invariance is implied. `--rule` restricts execution to one initially named head and cannot establish global normality. JSON trace export, validation and checked replay are implemented; see the [trace contract](docs/trace-format.md).

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

A trace saves the starting graph and what each rewrite did. Loading it checks the steps again instead of trusting the file. Checkpoints let the engine revisit earlier states while keeping the right identity numbers. A paused run can continue with the same random choices and memory of earlier states; simply looking at an old state does not change the original run. Existing output files are never overwritten. A saved run may have stopped at a limit, so successful file validation does not mean the program finished its work. See the [full contract](docs/trace-format.md) and [annotated example](examples/traces/README.md).

### Core interfaces

Sprint 3 adds repeatable rule selection, bounded graph comparison and automated tests that generate varied cases and shrink failures into smaller examples. This found a slow comparison on a nested rule package; the improved comparator and the reduced failing case are retained.

The [behavioural examples](examples/behaviour/README.md) use the same test for useful content throughout: a value link between two distinct objects. One program finishes while keeping that link, another finishes after deleting it, and a third keeps it while alternating between two graph states. The last example has a mathematical argument for continuing indefinitely; a finite computer run alone does not prove that. Native committed history, positive-resource dependencies and exact local conflict analysis are implemented. Portable trace import/export, checked replay and resumable logical runs are implemented. The browser interface provides linked views and verified history inspection; the expressiveness study remains later work.

`graph_core::ValidatedGraph` gives a read-only validated view tied to a graph borrow. `Graph::insert_batch_with_limits` resolves forward and cyclic references atomically; construction is distinct from full validation. `GraphLimits` bounds input bytes, atoms, fields, labels, package depth and validation work.

`engine::Execution` owns the committed graph. Submit an `OperationalBinding` to `prepare`, inspect the resulting `PreparedStep`, then pass it to `commit`. Ground rule/match data are checked and read-only. A candidate becomes stale after any committed step or successful `replace_graph`, even when the graph looks unchanged. External replacement starts a new execution history. `Execution::view()` decodes the current graph afresh; `live_rules()` includes inactive representations, while `active_rules()` excludes them and quoted heads. Neither iterator enumerates enabled matches.

`Execution::bindings(source)` enumerates raw matches; `enabled()` examines every active source and returns fully checked candidates plus expected rejections. `enabled_for(source)` restricts that query to one source. Select a candidate from `into_steps()` and pass it to `commit`. For cumulative queries, reuse one `Execution::search()`. Interrupted searches return errors, not partial or empty enabled sets. Only a successful empty all-source query establishes absence of an enabled step. `apply_with_binding` remains the direct reference path. Native run/check commands use this checked boundary; shared deterministic/seeded scheduling and bounded observation are provided by engine::runner and engine::observation.

The suite currently passes 294 unit/integration tests plus twelve documentation tests, including eight compile-fail API checks. See [architecture §20](docs/architecture.md#20-checked-construction-and-execution-interfaces) for defaults, error categories and the source-level commit argument. The parser uses pinned Logos 0.16.1; engine and parser remain independent production siblings.

For a practical browser walkthrough, select `practical/work-items`, load it and run: ten pending items become processed. `practical/policy-enable` first changes the processing rule's activation and revision, then processes those items. The [Guide](docs/workbench-guide.md) is available inside the application and explains the timeline, linked views, themes and source/project/trace files. [Dataset recipes and tested limits](examples/practical/README.md) distinguish successful processing from a resource-limited attempt. Sprint 4 includes expanded acceptance of the local workbench. Sprint 5 retains the RQ5 expressiveness study, controlled time/memory measurements and final-library integration.
