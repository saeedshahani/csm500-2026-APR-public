# Work-item processing profile

## Bounded reflective administration

`credit-spawn.graph` contains a meta-rule that spends one visible credit to construct an inactive rank-zero empty rule package. Two credits permit two editing transactions. `delete-rule.graph` retires one inactive package, demonstrating that deletion also counts as an edit. Run either with `cargo run --release --locked -p cli -- meta-check PATH finite-edits none`; the exact initial bounds are two and one respectively. These are standalone analysis examples, not additions to the eighteen-program browser catalogue. A credit guard is only safe while other rules cannot remove it, replenish credits or expose quoted credit patterns. See [the meta-activity study](../../docs/research/reflective-recovery.md#meta-activity-contracts-loop-f) for the proof, adversarial examples and distinct productive-modulation contract.

## Vocabulary-join example

`month-normalisation.graph` is an independently authored small example of the `joined-v4` functional-join class. It replaces a session-to-raw-month relation by a session-to-canonical-month relation using a preserved dictionary edge. Run `cargo run --release -p cli -- export examples/practical/month-normalisation.graph --max-steps 2 --execution-profile joined-v4`. The same pattern is used by the full public-data Calendar normalisation application in Projects; its importer preserves all eighteen UCI source fields and its result export checks each original row. See `platform/README.md` for the explicit larger local route. This small example contains no approval rule and is not itself a public-data benchmark.

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

The historical results below use the reference method and remain attributed to that configuration. An explicit `framed-v3` runtime now recognises the fixed one-shot rule shape, maintains its complete remaining occurrence set and retains exact screened recurrence. Select it in browser Run settings before loading, or pass `--execution-profile framed-v3` to CLI run/export. It is not the default and does not raise input or hosted job limits. The separately bounded native complete-job study uses `cargo run --release --locked -p engine --example scale_probe -- fixed 1000 pipeline-framed 1000 42`, including recording, recurrence, per-item payload/status checks and verified replay. See the [profile-specific study](../../docs/research/evaluation.md#b2-complete-job-collection); large synthetic native acceptance is distinct from the 1,000-item browser dataset input contract.

Attempted sizes are 10, 100 and 1,000; dataset seed 42, fixed policy, complete-carrier observer, recurrence detection off, deterministic native scheduling and browser scheduler seed 42. The browser uses a 20-step absolute bound, 1,000,000 matching-work units per query, 1,000 candidates and single-commit batches for stepping. Ten-item guarded cases use the same allowances. Source selection and loaded settings must be retained when reproducing a run.

Local acceptance through 2026-09-16 uses Rust 1.85.0 and Playwright Chromium 1243. The initial checks used Node 25.4.0; the integrated rerun also passes with local Node 24.21.0, while CI remains pinned to 25.4.0. The hundred-item completion profile overrides the initial 20-step browser bound with 101, uses a blank execution seed (deterministic), disables recurrence and advances ten commits per batch. These checks establish:

| Attempt | Observation | Not established |
|---|---|---|
| 10 fixed / 10 guarded | NormalForm after 10 / 11 commits; whole-carrier oracle; exact CLI/native/WASM trace agreement; browser counts and historical navigation | Arbitrary scheduling or modified-source correctness |
| 100 fixed | NormalForm after 100 commits; CLI validation of exported history; pending/processed counts 100/0, 50/50 and 0/100 at states 0/50/100; pagination and unchanged exports after navigation | Controlled browser throughput or memory cost |
| 1,000 fixed | Load and complete-identity search; focused rendering; stop/recovery; attempted step reaches ResourceLimit with zero events | Successful processing or arbitrary scale |

The first two datasets have 33 and 213 initial atoms; the thousand-item fixed dataset has 2,013. Large input admission is distinct from feasible complete matching. The matching algorithm enumerates candidates before selection, while replay and publication reconstruct and copy graph/history data. The canvas limits layout to 250 atoms and 500 connections, the item table to 25 rows and atom selection to 100 entries per page. All admitted identities remain searchable. The bridge's separate inspect query returns at most 200 matching records; full state responses and verified exports remain bounded but are not streaming.

These browser observations are functional boundaries, not time or memory benchmarks. The separate [native performance study](../../docs/research/evaluation.md#performance) measures these recipes alongside controlled graph-size, branching, rule-count and trace-length variations. Its hundred-item run completes; its thousand-item run reaches ResourceLimit without a commit. Browser layout and storage costs are not measured there. A search limit or unacknowledged interrupted batch must not be counted as completed work. The shared [catalogue](../README.md) retains the ten-item fixed and guarded reference profiles.
