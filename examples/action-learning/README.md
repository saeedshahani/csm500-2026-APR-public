# Acquiring and executing action rules

The example observes one door being opened and an already-open door remaining open. Power is present in both observations. The acquired action therefore requires power and sets `open` to true; the door's previous open/closed state ceases to be a precondition. Execution applies the same learned schema to two previously unseen doors. The first rewrite opens only the front door; the second opens the rear door. Both retain the power fact.

```sh
cargo run --locked -p cli -- action-learn examples/action-learning/doors.json
cargo run --locked -p cli -- action-compile examples/action-learning/doors.json
cargo run --locked -p cli -- action-compile examples/action-learning/doors.json | cargo run --locked -p cli -- export - --max-steps 8
```

`action-learn` returns the fitted model, observation receipts, projected Boolean states and a checked portable trace. `action-compile` emits ordinary graph source that can be loaded into the workbench or used by the existing run/export commands. There is no special browser learning panel or hosted training endpoint. The input schema and typed argument binding determine features; every ground fact must have an explicit Boolean value. Missing facts are errors. Nullary predicates, such as `powered`, are shared by all bindings. Failed attempts use `after: null` and are assumed to leave the world unchanged.

Inspect `model.features` alongside each model's `preconditions` and `effects`. `null` means no constraint or no observed assignment, respectively; it does not represent an unknown world value. In the carrier, `action.fact` links each coordinate to its predicate/arguments, `action.value` links it to its present Boolean value, and `action.rule` annotations identify each word position and evidence epoch. Rule bodies encode the conditions and updates. Changing or removing these rules changes execution. The fixed action word is supplied by the caller; this command does not discover a plan.

An execution that cannot match the next learned condition returns `completed: false`, even if the graph has reached normal form. That outcome does not prove the real action impossible: the learner deliberately abstains outside its justified model. A usable action needs at least one successful observation. `Learner::classify` distinguishes safe, rejected, unknown and quarantined cases; before any positive example, only an exactly witnessed failure is rejected.

Every observation pins schema, environment, expected model version, per-action epoch and sequence. Rejected input leaves the learner unchanged. A contradiction either resets the affected schema's evidence into a new epoch (`local`) or disables that schema (`quarantine`). A quarantined schema stays disabled; starting an explicitly fresh learner is the recovery boundary. Old snapshots and compiled graphs remain immutable. Version checks prevent stale requests within a session; they cannot detect an environmental change before evidence arrives or authenticate an external producer. Use a new environment identifier for an unrelated session.

The learner is Rust application code in the existing engine crate. Its fitting updates are not graph rewrites. The compiled model executes through ordinary graph admission and checked replay. This applies the conservative construction of Juba, Le and Stern (2021), *Safe Learning of Lifted Action Models*, [KR 2021](https://doi.org/10.24963/kr.2021/36), under exact stationary observations, deterministic conjunction/assignment semantics and the declared frame. It neither guarantees safe exploratory attempts nor interprets noisy or incomplete observations.

Bounds: 16 types/action schemas, 32 predicates, arity four, 128 local features, 64 objects, 4,096 ground facts/observations, sixteen requested actions, and 16 MiB CLI input/output. Ordinary graph, matching and trace limits additionally apply; inputs within the grammar can still exhaust those limits. No additional permission, budget or hosted quota is implied.
