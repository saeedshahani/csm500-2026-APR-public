# Behavioural evidence

These are three predeclared experiments, not a general stability classifier. Every program stores its rules in the carrier. The rules in these particular examples are fixed object rules; reflective revision is tested separately.

Use `examples/observers/all.observer` for the complete carrier and `examples/observers/value.observer` for the same restricted observation in all cases. The latter selects data links labelled `value`, then their endpoints. The measure m counts distinct endpoints of non-self selected value links; the threshold is two. Labels have no built-in meaning.

| Case | Initial carrier (atoms / fields) | Hand-derived result | O_all | O_value and m |
|---|---:|---|---|---|
| stable | 20 / 25 | Replace the one pending link by done; preserve every other atom | One step reaches a normal form, 20 / 25 | The same three-atom observation throughout; 2 → 2 |
| collapse | 17 / 19 | Delete only the value link; preserve endpoints and complete program | One step reaches a normal form, 16 / 17 | Three atoms become the empty observation; 2 → 0 |
| oscillating | 36 / 46 | Replace pending by done on odd steps, and done by pending on even steps | Two distinct classes alternate; size remains 36 / 46 | The same three-atom observation throughout; m = 2 |

Expected files were constructed from those deletion/preservation/fresh-creation descriptions, not saved from engine output. Fresh phase identities need not equal the names in expected files. Collapse does not erase the complete carrier. No loose atom is silently discarded.

## Reproduction profile

Rust 1.85.0, Cargo.lock, the initial authored files, observer declarations, and the implementation fingerprint in dissertation Appendix B identify the experiment. Use seeded-v1 with seed 42 and the source-template-host-lexicographic-v1 order. Each non-normal state has exactly one enabled binding, so these examples do not depend on fairness or random choice. Default per-query bounds are 1,000 candidates and 1,000,000 search units; graph defaults are specified in architecture §20.4. Projection, comparison and history bounds are each 1,000,000 units. All finite experiment commands use six as the maximum committed-step count, unless testing zero/exact limits.

```sh
cargo run --locked -p cli -- run examples/behaviour/stable.graph --max-steps 6 --strategy seeded --seed 42 --observer examples/observers/value.observer --detect-cycles true
cargo run --locked -p cli -- run examples/behaviour/collapse.graph --max-steps 6 --strategy seeded --seed 42 --observer examples/observers/value.observer --detect-cycles true
cargo run --locked -p cli -- run examples/behaviour/oscillating.graph --max-steps 6 --strategy seeded --seed 42 --observer examples/observers/all.observer --detect-cycles true
cargo run --locked -p cli -- run examples/behaviour/oscillating.graph --max-steps 6 --strategy seeded --seed 42 --observer examples/observers/value.observer --detect-cycles true
cargo test --locked -p cli --test behaviour
```

The first two commands return NormalForm after one step (exit 0). Oscillation under O_all returns CycleDetected at indices 0 and 2 (exit 6); under O_value it returns at 0 and 1, with complete-state-equal=false. With cycle detection disabled it reaches StepLimit after six steps (exit 3). Zero steps yields StepLimit for all three; one step establishes normality for stable and collapse. Normality is checked before recurrence. Consequently the normal-form reports leave the runtime payload field absent; the test independently projects the published final graph to establish m = 2 or 0 rather than reusing an earlier state's measure.

## Mathematical interpretation

Stable is the one-pending-link instance of LN (formalism N1–N4). Its maximal execution has one step, with a unique normal form modulo full isomorphism. Its normal-form class is strongly stable because no member has a successor; this is termination, not an invented infinite sequence of idle events. Its payload is invariant in the fixed-program family, not throughout all possible reflective configurations.

Collapse has exactly one matching value link. Its deletion satisfies dangling exclusion because nothing points to that link. No value link remains and the sole rule cannot apply. Hence the measure crosses the same threshold despite the surviving constructors and rule representation.

Oscillating has exactly one phase link and two opposite relabelling rules. Exactly one is enabled at every step, its endpoints are preserved, and one fresh opposite-phase link is created. The program and value link never change, so GC is vacuous. Induction gives every finite alternating prefix and a unique infinite maximal execution in the mathematical infinite-identity model. Pending/done label counts distinguish the two full-carrier classes. This infinite execution oscillates under O_all but is constantly observed under O_value. No scheduler fairness assumption is needed beyond continuing to execute the sole enabled step; a user stop or resource limit instead gives a finite prefix. The bounded implementation does not have an infinite identity allocator, and CycleDetected alone is not the proof.

These native state/result records are inputs to S4.E's golden-trace conversion (S4-116); they are not causal traces, checkpoints or history-preserving replay files.
