# Authored execution cases

These UTF-8 programs contain both data and rules. They use explicit atom declarations so that every metadata record and quoted item can be inspected. Rule/package shorthand is illustrated separately in `../syntax/`. No example relies on built-in arithmetic or an external rule store.

From the repository root:

```sh
cargo run --locked -p cli -- check examples/programs/arithmetic.graph
cargo run --locked -p cli -- run examples/programs/arithmetic.graph --max-steps 1
cargo run --locked -p cli -- run examples/programs/self-modify.graph --max-steps 3
cargo run --locked -p cli -- run examples/programs/mutual-regress.graph --max-steps 1 --rule p
cargo test --locked -p cli --test programs
```

The arithmetic command exits 0 with `NormalForm`. The bounded self-modification command exits 3 with `StepLimit`; this is not an error in its graph or proof of divergence. The requested mutual-regress command exits 4 with `RequestedRuleUnavailable` and a guardedness diagnostic. Without `--rule p`, a complete query reports `NormalForm`: raw matches exist, but none is admissible. `check` reports only structural validity and performs no rewrite.

The command-line policy chooses the first enabled candidate in identity-based search order. It is reproducible for the same input but is not fair or invariant under renaming. In particular, repeated installation can starve the installed rule, and repeated P/supervisor execution can starve Q. The integration tests select the indicated candidate from a **complete automatically computed enabled set**. They do not supply a binding to the transaction or bypass any admission check. The self-modification test selects the current output membership explicitly; the CLI's first-enabled policy need not choose that same membership when several become eligible. These are witness paths, not claims about all schedules. General scheduling is Sprint 3 work.

## Independent expectations

Expected files were derived by symbolic delete–preserve–create accounting before executing these programs, not by saving interpreter output. Arithmetic, SelfLower and installation are transcriptions/extensions of A2, B2 and A4's hand ledgers. C1 is the rejected rank exchange. Restoration transcribes C2's two note-replacing lowerers and supervisor. Fresh names in expected files are explanatory and do not predict allocation keys. Every atom omitted from a stated deletion survives, including unattached syntax and old output atoms; no garbage collection is assumed.

| Case and selected steps | Expected change | Rank and revision expectation | Preserved expectation |
|---|---|---|---|
| `arithmetic.graph`: AddZero once | Remove `h_a`, `h_z`, `h_e0`, `h_e1`, `h_e2`; add `expr(h_ev,h_y)` | Rule remains rank 0 with the same revision | `arithmetic.expected.graph`: 27 atoms, 35 fields; non-empty normal form |
| `self-modify.graph`: self, supervisor, self | First self-step removes rank-top, rank, revision and A-output membership; creates rank, revision, visible A, quoted B and its membership (4 consumed/5 produced). Supervisor replaces rank/revision and adds a successor (2/3). Second self-step repeats 4/5 on the selected B membership, producing visible B | Self ranks 1→0→1→0; each change receives a fresh revision. Supervisor remains rank 2 and unchanged | `self-modify.first.expected.graph`, `.promoted.expected.graph`, and `.expected.graph`; all old unattached constructors retained |
| `mutual-regress.graph`: request P | Exchange would give the other rule rank 1 while source pre-rank is 1; GC rejects equality | Both ranks and revisions unchanged; no event committed | The input file itself is the complete expected state |
| `staging.graph`: installer, then entered rule on Payload | Jointly copy 13 quoted atoms, then add Wrapped and its child link (0/13, then 0/2) | Installer stays rank 1; entered rule is rank 0 with a fresh private revision; its later object step leaves that revision unchanged | `staging.installed.expected.graph` and `staging.expected.graph`; original package stays quoted and the installed variable stays private |
| `restoration.graph`: P, supervisor, Q, supervisor | Each lowerer consumes rank-top, rank, revision, note and its two memberships (6/5); each restoration replaces rank/revision and adds successor (2/3) | Target rank 1→0→1; each alteration has a fresh revision. Supervisor rank 2 unchanged | `restoration.expected.graph`: after each complete round, identical complete-carrier structure up to renaming, including both notes; atom/field counts unchanged |

`crates/cli/tests/programs.rs` compares each stated final carrier with the stored expectation using exhaustive kind-, attribute- and positional-field-preserving isomorphism. It also reverses expected allocation order to ensure the comparison is not identity equality. The oracle is test-only, limited to 256 atoms and 1,000,000 candidate assignments per comparison; unfinished search is an error, not inequality. The test runs four complete restoration rounds and checks revision freshness and step deltas separately, since isomorphism alone cannot establish historical identity change. It is finite regression evidence, not an infinite-execution proof.

## Research interpretation

Arithmetic supplies a small RQ2 normal-form witness and an RQ1 carrier/execution case. Consistent renaming of Add/Zero labels leaves its behaviour unchanged; removing activation stops it. It is an AddZero structural reduction, not a general arithmetic implementation, convergence theorem or RQ5 simulation.

Self-modification and staging supply RQ1 witnesses of graph-resident operational reflection. C1 tests rejection of a particular forbidden rank exchange; it does not establish that every infinite meta-run is forbidden. C2 supplies RQ1's distinction between finite exact-revision chains and continuing supervised meta-activity. Its complete-carrier recurrence also provides material for RQ3, but no scheduler-independent stability or observation-sensitive collapse result follows here. Output accumulation in self-modification remains part of the complete state.

These expected carriers are retained as independent regression inputs. Exported event traces and their golden comparisons belong to S4.E; no trace, causal-poset or replay claim is made by a printed state.
