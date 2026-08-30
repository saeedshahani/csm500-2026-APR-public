# A Minimal Self-Referential Graph Formalism

MSc Computer Science dissertation project exploring graph-resident rewrite rules, guarded rule modification, semantic stability, and causal order derived from rewrite-event dependencies. The planned software artifact is a Rust rewriting engine with a concrete syntax, CLI, WebAssembly bridge, property-based tests, reproducible traces, and a browser visualiser.

The repository is currently documentation-only: no Rust workspace or executable engine exists. The pre-engine design was re-baselined on 30 August 2026 around an ordinary atom/field carrier graph, structural quotation and fresh-rule packages, variable instantiation followed by a ground rewrite, explicit rule lifecycle, and revision-aware mutable-rank guardedness. The completed constrained comparative/Pareto audit selects a five-kind generic-record schema. It has a smaller declared profile than the superseded seven-kind schema, is fixed-schema irredundant under the recorded single-symbol ablations, and is Pareto-undominated among the studied eligible candidates on the published numeric vector; subset and global minimality are not claimed.

## Documentation

New to the project? Start with [`docs/simple.md`](docs/simple.md) — a plain-language explanation of what the work is and where it stands, written for a general reader rather than a marker.

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

Build and run instructions will be added when the Rust workspace is scaffolded.

The pre-engine minimality gate is complete in [`docs/worked-examples.md`](docs/worked-examples.md) §§8–9 and ADR-051. The selected generic-record encoding must now reproduce all operational fixtures—fresh staging, exact self-rewriting, rejected mutual revision-depth regress, admitted restoration, conservative over-rejection, and object divergence—before broader engine runs count as evidence.
