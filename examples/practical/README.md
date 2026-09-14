# Work-item processing profile

The domain contains named work items with exactly one status, pending or processed. A constructor labelled `WorkItem:<id>` represents an item. One unquoted `ObjectLink` record labelled `pending` or `processed` points to that constructor in both field positions. The self-link denotes a unary status using the existing binary record kind; it is not a new semantic primitive. Identity, domain ID and authored alias are distinct.

## Input and deterministic generation

Dataset JSON has exactly `schema`, `policy` and `items` fields. The schema is `work-items-v1`; policy is `fixed` or `guarded_enable`. Each item has exactly `id` and `status`; status is `pending` or `processed`. IDs are unique, 1–64 ASCII letters, digits, hyphens or underscores. Empty datasets are valid. Unknown fields, statuses, policies, duplicate IDs and more than 1,000 items are rejected. Dataset requests have a 1 MiB input allowance. Checked source construction still enforces the normal parser and carrier limits.

```json
{"schema":"work-items-v1","policy":"fixed","items":[{"id":"invoice-01","status":"pending"}]}
```

The generator uses IDs `work-0000` through `work-(N-1)`, all pending. It permutes their authored order with Fisher–Yates using the unsigned 32-bit recurrence `x = 1664525*x + 1013904223` modulo 2^32, selecting each index by remainder. This is a reproducible fixture generator, not a statistical sampling or cryptographic claim. Retain count, seed and policy; the dataset seed is distinct from the execution scheduler seed. The committed ten-item sources use seed 42.

From the repository root:

```sh
cargo run --locked -p wasm-bridge --example dataset -- 10 42 fixed
cargo run --locked -p wasm-bridge --example dataset -- 100 42 fixed json
cargo run --locked -p cli -- export examples/practical/work-items.graph --max-steps 20
cargo run --locked -p cli -- export examples/practical/policy-enable.graph --max-steps 20
```

The optional final `json` argument emits the restricted dataset instead of compiled graph source. Generated graph source retains count, seed and policy in an inert recipe comment, also preserved by source/project export. Dataset JSON retains the explicit item order rather than generator provenance; retain the generating command alongside JSON when the seed is needed. The browser's Work-item datasets control invokes the same encoding through the shared protocol. Preview reports input counts, carrier size, pending items and the expected number of commits before explicit loading. It does not replace the session.

## Independent reference outcomes

For fixed processing, every pending status is replaced by a fresh processed status, preserving the item. Already processed items are unchanged. With P pending items, sufficient budgets, and the generated rules without edits, exactly P commits lead to normal form. No rule recreates a pending status. A whole-carrier reference transformation preserves all rule atoms and field incidence, changes only the domain status labels and accounts for P fresh identities; it is independent of matching and transaction code. Tests normalise only these intentionally replaced identities and compare every remaining atom and field, also checking the allocator increment. These tests are finite evidence, not a new general convergence theorem.

In the guarded policy example, `process` is initially inactive at rank zero. An active rank-one meta-rule consumes the unquoted `EnableProcessing` permission and the processing rule's old Revision record. It creates a fresh Revision and an Active record, preserving the rule head, mode, rank and pattern. The source pre-rank is one and target post-rank is zero, satisfying the guard. The processing body is unchanged: this is an activation-policy modification, not synthesis of a new algorithm. Subsequent data events use the fresh revision. The expected count is P+1, allocator increment P+2; the whole-carrier reference also removes the permission and replaces the revision. Quoted permission syntax remains part of the meta-rule definition. A later raw match against that syntax fails dangling admission; the CLI may print `Dangling: dangling reference` as a rejected candidate while correctly reporting NormalForm. No loose permission copies are generated.

The fixture contains no external work service, side effects or business transaction. Normal form means its specified local processing is finished, not that invoices or jobs were processed outside the graph.

## Declared scale profile and limits

Attempted sizes are 10, 100 and 1,000; dataset seed 42, fixed policy, complete-carrier observer, recurrence detection off, deterministic native scheduling and browser scheduler seed 42. The browser uses a 20-step absolute bound, 1,000,000 matching-work units per query, 1,000 candidates and single-commit batches for stepping. Ten-item guarded cases use the same allowances. Source selection and loaded settings must be retained when reproducing a run.

Local checks on 2026-09-14 with Rust 1.85.0, Node 25.4.0 and Playwright Chromium 1243 establish:

| Attempt | Observation | Not established |
|---|---|---|
| 10 fixed / 10 guarded | NormalForm after 10 / 11 commits; whole-carrier oracle; exact CLI/native/WASM trace agreement; browser counts and historical navigation | Arbitrary scheduling or modified-source correctness |
| 100 fixed | Load, pagination, one committed step, 99 pending / 1 processed, state-zero and endpoint navigation | Full hundred-item completion or throughput |
| 1,000 fixed | Load and complete-identity search; focused rendering; stop/recovery; attempted step reaches ResourceLimit with zero events | Successful processing or arbitrary scale |

The first two datasets have 33 and 213 initial atoms; the thousand-item fixed dataset has 2,013. Large input admission is distinct from feasible complete matching. The matching algorithm enumerates candidates before selection, while replay and publication reconstruct and copy graph/history data. The canvas limits layout to 250 atoms and 500 connections, the item table to 25 rows and atom selection to 100 entries per page. All admitted identities remain searchable. The bridge's separate inspect query returns at most 200 matching records; full state responses and verified exports remain bounded but are not streaming.

These are functional boundary observations, not time or memory benchmarks. S5.D should use these exact recipes alongside controlled graph-size, branching, rule-count and trace-length variations, separately recording matching, replay/publication, layout and storage costs. A search limit or unacknowledged interrupted batch must not be counted as completed work.
