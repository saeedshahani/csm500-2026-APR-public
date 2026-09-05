# A Minimal Self-Referential Graph Formalism

MSc Computer Science dissertation project exploring graph-resident rewrite rules, guarded rule modification, semantic stability, and causal order derived from rewrite-event dependencies. The planned software artifact is a Rust rewriting engine with a concrete syntax, CLI, WebAssembly bridge, property-based tests, reproducible traces, and a browser visualiser.

## Documentation

New to the project? Start with [`docs/explainers/simple.md`](docs/explainers/simple.md) — a plain-language explanation of what the work is and where it stands, written for a general reader rather than a marker.

- [`docs/project-proposal.md`](docs/project-proposal.md) — approved research baseline and fixed six-sprint timeline.
- [`docs/dissertation.md`](docs/dissertation.md) — living dissertation draft, written up sprint by sprint rather than at the end.
- [`docs/formalism.md`](docs/formalism.md) — normative mathematical definitions and open proof obligations.
- [`docs/worked-examples.md`](docs/worked-examples.md) — hand-traced constructions and counterexamples behind the formalism.
- [`docs/architecture.md`](docs/architecture.md) — technical architecture implementing the formalism.
- [`docs/research-plan.md`](docs/research-plan.md) — RQ-to-evidence plan and live evidence log.
- [`docs/sprint-plan.md`](docs/sprint-plan.md) — high-level plan retaining the proposal's original sprint numbers and dates.
- [`docs/task-list.md`](docs/task-list.md) — granular execution checklist within those fixed sprints.
- [`docs/decisions.md`](docs/decisions.md) — append-only architecture decision record.
- [`docs/literature.md`](docs/literature.md) — annotated literature map and working bibliography.

## Build and check

Install Rust through rustup; `rust-toolchain.toml` pins Rust/Cargo 1.85.0. No Node tooling is required for the current native workspace.

```sh
cargo test --workspace --locked
cargo run --locked -p cli -- demo
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
```

`demo` executes graph-resident SelfLower from rank one to zero. Test ledgers live in `examples/fixtures/`; they are not the planned surface language. The CI workflow is configured, but remote execution is not yet confirmed.

The repository's existing ignore policy for local documentation is preserved. Builds and tests use the source fixtures directly and do not require dissertation or explainer files.

Sprint 1 local acceptance is complete. Research evidence is recorded in [`research-plan.md`](docs/research-plan.md), implementation boundaries in [`architecture.md`](docs/architecture.md), and remaining work in [`task-list.md`](docs/task-list.md). The explicit schema is a documented trade-off: a default-binding comparator uses one fewer declared tag with equal carrier costs. Full research adequacy remains pending. Sprint numbers and dates are unchanged; committing, pushing and remote CI confirmation remain with the user.
