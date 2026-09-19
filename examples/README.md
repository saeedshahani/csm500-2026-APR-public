# Example catalogue and capability audit

The catalogue contains eighteen complete programs covering the seven required research categories: arithmetic, reflective modification, stability, oscillation, collapse, independent rewrites and lambda reduction. Two work-item profiles provide an additional practical interpretation; guarded processing also belongs to the reflective category. Every executable `.graph` includes its rules and initial data. No external rule store or built-in arithmetic operation is required.

[manifest.json](manifest.json) is the common CLI-test and browser-gallery index. Each entry records the authored input, executable graph, research questions, purpose, expected bounded outcome, observer, scheduler seed, limits and explanation. Lambda graphs are generated from the adjacent named source terms by the shared adapter. Expected carriers and fixture ledgers are supporting evidence, not additional selectable programs. The other `syntax/` and `fixtures/` inputs exercise language and admission boundaries rather than the seven-category demonstration library.

## Reproducing a catalogue entry

From the repository root, substitute an executable path and step bound from the table:

```sh
cargo run --locked -p cli -- export examples/programs/arithmetic.graph --max-steps 6 --strategy seeded --seed 42 --observer examples/observers/all.observer --detect-cycles false --output arithmetic.trace.json
cargo run --locked -p cli -- trace-check arithmetic.trace.json
cargo run --locked -p cli -- replay arithmetic.trace.json --at 1
```

Publication refuses to overwrite an existing destination. `NormalForm` exits 0; `StepLimit` exits 3; `CycleDetected` exits 6. These are distinct outcomes, not interchangeable successes. A finite prefix is not a proof of divergence. All reference profiles use seed 42, 1,000 candidates and 1,000,000 matching-work units per query. They use the complete-carrier observer; recurrence is disabled except for the oscillation profile. Other resource bounds remain the shared CLI/bridge defaults recorded in the exported trace. No requested-rule override is used in the catalogue runs.

In the browser, select the same example, press **Apply example settings**, then **Load program** and **Run**. Selecting source or applying settings does not modify an already loaded history. The timeline inspects that history. Lambda entries are also available through the named-source editor; the graph gallery displays the complete generated program, including its eleven machine rules.

For a debugger walkthrough, open Rule breakpoints after loading practical/policy-enable and select process. The activation event runs first; the browser then pauses before processing an item with the replacement revision. Step commits that occurrence, while Run continues until another selected breakpoint or execution boundary. Debugger pauses do not add events or change the reference result. Reset/load/import clear the selections; worker recovery restores acknowledged controls separately from the trace. See [the workbench guide](../docs/guides/workbench.md#stopping-before-a-particular-rule).

| Executable input | Step bound | Reference outcome | Interpretation and supporting evidence |
|---|---:|---|---|
| programs/arithmetic.graph | 6 | NormalForm, 1 event | Graph-defined AddZero removes five atoms and creates one link; not a general calculator. [Authored cases](programs/README.md). |
| programs/self-modify.graph | 6 | StepLimit, 6 events | Seeded reflective prefix. The separately selected self/supervisor/self path changes later output from A to B; arbitrary selection need not follow it. [Authored cases](programs/README.md). |
| programs/mutual-regress.graph | 6 | NormalForm, 0 events | Raw rank exchange is rejected by GC; no admitted successor. [Rejected comparison](programs/README.md). |
| programs/staging.graph | 6 | StepLimit, 6 events | Rules can install rules. Installer-then-use is a separately selected witness, not a fairness claim. [Authored cases](programs/README.md). |
| programs/restoration.graph | 6 | StepLimit, 6 events | Revisions remain fresh even when a selected four-step round restores graph shape. [Authored cases](programs/README.md). |
| behaviour/stable.graph | 6 | NormalForm, 1 event | A non-empty normal form with payload two. [Behavioural proof and measure](behaviour/README.md). |
| behaviour/oscillating.graph | 6 | CycleDetected, 2 events | Alternating full-carrier classes; an induction, not the finite recurrence report, establishes mathematical infinite behaviour. [Behavioural proof](behaviour/README.md). |
| behaviour/collapse.graph | 6 | NormalForm, 1 event | Value-observation payload falls from two to zero, although rules and endpoints remain. [Predeclared measure](behaviour/README.md). |
| independent/disjoint.graph | 6 | NormalForm, 2 events | Both local residual orders agree; no positive dependency edge. [Independence witnesses](independent/README.md). |
| independent/shared-read.graph | 6 | NormalForm, 2 events | Shared unchanged reads do not impose order; commutation is checked separately. [Independence witnesses](independent/README.md). |
| practical/work-items.graph | 20 | NormalForm, 10 events | Ten local status changes; no external jobs are executed. [Practical interpretation](practical/README.md). |
| practical/policy-enable.graph | 20 | NormalForm, 11 events | One graph-resident activation precedes ten processing steps using the new revision. [Practical interpretation](practical/README.md). |
| lambda/identity.graph | 20 | NormalForm, 3 events | Identity application; two administrative steps and one beta step. [Source derivations](lambda/README.md). |
| lambda/erasure.graph | 20 | NormalForm, 6 events | An unused divergent argument is not evaluated. [Source derivations](lambda/README.md). |
| lambda/repetition.graph | 20 | NormalForm, 7 events | Repeated use of a bound argument. [Source derivations](lambda/README.md). |
| lambda/shadowing.graph | 20 | NormalForm, 2 events | The nearest binder determines the variable. [Source derivations](lambda/README.md). |
| lambda/context.graph | 20 | NormalForm, 6 events | Reduction in function position before the outer application. [Source derivations](lambda/README.md). |
| lambda/omega.graph | 20 | StepLimit, 20 events | Self-application continues in the source semantics; finite graph execution records five beta and fifteen administrative steps. [Source derivations](lambda/README.md). |

The behavioural regression additionally runs stable, collapse and oscillating with `examples/observers/value.observer`. Oscillation then reports recurrence after one event while the full carrier differs. A fourth variant disables recurrence for the full-carrier oscillating run and records six events with StepLimit. These variants retain the same declared payload measure and threshold; they do not redefine collapse after observing the outcome.

The explicitly rejected comparison is also reproducible with:

```sh
cargo run --locked -p cli -- run examples/programs/mutual-regress.graph --max-steps 1 --rule p
```

It exits 4 with RequestedRuleUnavailable and a guardedness diagnostic. NormalForm without this override means no *admitted* step, not that no raw match exists.

## Verification and golden records

```sh
cargo build --locked -p cli
cargo build --locked -p wasm-bridge --example protocol
node scripts/verify-examples.mjs
cargo test --locked -p graph-core --test encoding_audit
cargo test --locked -p cli --test programs --test behaviour --test trace --test lambda
# From frontend/: npm run test:parity
```

The catalogue verifier checks twenty-two execution profiles. It compares CLI export with the native workbench protocol, validates each trace against the public `reflective-trace-v1` decoder and checked replay, reconstructs every historical state, and exercises CLI source replay at initial, middle and final positions. The same verifier is invoked with exact native/WASM comparison by the parity suite. Source replay is loadable text, not an identity-preserving checkpoint.

[catalogue-golden.json](catalogue-golden.json) stores complete-trace and per-state SHA-256 regressions. Object keys are sorted, array order and identities are retained, and only the build implementation fingerprint is omitted from the golden digest. The original, unmodified live trace is first verified with its actual implementation identity; hashes are never used to bypass import checks. Changes to source bytes, allocation, settings, reports, revisions, footprints, checkpoints or dependencies therefore change the regression. Explicit regeneration uses `node scripts/verify-examples.mjs --regenerate` only after reviewing the semantic change. The verifier still checks declared outcomes before regeneration.

These generated regressions detect drift; they are not independent correctness proofs. Hand-derived carriers in `programs/`, `behaviour/` and `independent/`, the source substitution oracle in `crates/cli/tests/lambda.rs`, and the whole-carrier practical oracle in `crates/wasm-bridge/tests/datasets.rs` provide separate expectations. `crates/cli/tests/trace.rs` checks produce/read and consume dependency witnesses, selected reflective histories, both residual orders and conflict assertions. `crates/engine/tests/` supplies negative admission, conflict, shared-read and replay-tampering tests. Absence of a positive-order edge alone is never treated as proof of independence.

## Capability conclusions

The eight capabilities are those fixed in [formalism §12.1](../docs/formalism.md), not easier substitutes chosen after implementation.

| Capability | Retained evidence | Conclusion and boundary |
|---|---|---|
| C1: ordinary object graphs | Graph-core construction/validation and graph isomorphism tests; parallel shared-read links; cyclic fixtures | Vertices, labelled directed links, sharing, parallel links and finite cycles are supported. No infinite in-memory graph is claimed. |
| C2: graph-resident execution | Decoder/matcher/transaction tests; complete authored programs; arithmetic activation and label-renaming controls | L/R and preserved intersection, mode and activation are decoded from the graph. An external interpreter still defines their meaning. |
| C3: reflective modification | Guarded practical activation, promotion and restoration; revision-aware trace oracles | Rules can change other rules, including activity, mode or body under validation and GC. Arbitrary unsafe edits are rejected. |
| C4: literal self-modification | B2; `self_modification_changes_a_to_b_after_graph_resident_promotion`; hand-derived complete carriers | AppliedRule binds the exact source head and an admitted rewrite changes it. Later output changes along the selected witness; universal scheduling fairness is not claimed. |
| C5: staged creation | A4; installer/package decoding and nested-variable tests; staging installed/used carriers | A complete rule is installed with fresh private syntax and can execute later. Incomplete packages and capture remain rejected. |
| C6: operational adequacy | Ground DPO result D1; variable matcher, preserved/deleted/fresh and dangling tests; bounded checked construction | Finite checks are decidable and resource exhaustion is explicit. DPO correspondence is restricted to the stated ground attributed-record setting, not all reflective transactions. |
| C7: version-depth non-regress | GC and G5′/G6 arguments; rejected rank exchange; exact-revision histories | No infinite exact-revision ancestry chain. This is a manual proof under stated assumptions, not mechanical verification or termination of all meta-activity. |
| C8: retained computation | Oscillation induction; restoration C2; selected restoration rounds; Omega correspondence | Object non-termination and unbounded supervised meta-activity remain mathematically expressible. The implementation executes bounded prefixes. |

No C1–C8 capability is substituted or found missing within its declared scope. This does not establish every proposal ambition: complete application acceptance, remaining editor/debug controls and final evaluation reconciliation are separate tasks. RQ2–RQ3 conclusions remain confined to the stated stability families and observers; RQ4 exposes positive-resource order plus local residual evidence, not complete enabling causality; RQ5 covers closed weak-head call-by-name terms, not full normalisation or an unrestricted programming platform.

The counting regression reproduces `G` carrier atoms/fields as 31/39, 33/47, 23/31 and 81/117, with pattern sizes 8/13/7/28. Applying the declared `U` formulas yields 89/189, 99/220, 73/155 and 217/522, with pattern lower bounds 16/28/14/54. The default-free round trip and provisional mode/activity omission counts are also retained. These are bounded representation checks, not implemented alternative engines. `G_F` retains the same fixture costs with seventeen declared items against `G`'s eighteen; explicit declarations remain an engineering choice despite that dominance. `E` remains outside the historically selected ordinary-carrier class and receives no Pareto rank. Neither global minimality nor an exhaustive lower bound follows.
