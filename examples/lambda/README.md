# Closed lambda examples

These original fixtures retain the default `closed-cbn-v1` encoding. The separate [open symbolic examples](open/README.md) use the opt-in `open-cbn-v1` profile and do not alter this catalogue's reference histories.

All inputs use the closed, pure, untyped lambda fragment. Application is binary and fully parenthesised; abstractions use ASCII `\`. Bound names may be renamed. Free names, constants and effects are excluded. Evaluation is call-by-name to weak head form, not reduction under lambdas. The compiler creates the eleven graph-resident rules documented in [the encoding](../../docs/reference/lambda-encoding.md).

Write `I = (\x. x)`, `K = (\x. (\y. x))`, `D = (\x. (x x))`, `Ω = (D D)`. Independent hand calculations are:

- **identity:** `I I → I`; one beta reduction.
- **erasure:** `K I Ω → (\y. I) Ω → I`; two beta reductions; Ω is never evaluated.
- **repetition:** `D I → I I → I`; two beta reductions, repeated use through closures rather than destructive shared-variable binding.
- **shadowing:** `(\x. (\x. x)) I → I`; one beta reduction; the inner binder shadows the outer one.
- **context:** `(I I) I → I I → I`; two beta reductions along the function spine.
- **omega:** `Ω → Ω → …`; retain a bounded graph prefix, not a termination result. Syntax and environments can share; no cyclic source term is supplied.

`golden.json` contains actual graph transition names and readbacks at every step, generated and checked against the independent source evaluator in `crates/cli/tests/lambda.rs`. `*.trace.json` are verified portable engine histories; `*.graph` are compiler outputs, not separately maintained rules. The generated names/identities are not the observation used in correspondence tests. Finite test agreement supports, but does not replace, the proof. Earlier rejected encoding candidates remain in `lambda_candidates.rs`.

The retained histories are `recorded_prefix` documents: they certify event bindings without promising reproduction by an exact scheduler build. Outcomes and beta counts are checked by fresh execution and retained in `golden.json`. Imported prefixes are inspectable, not resumable runs. Regenerate only these compiler-owned files with `cargo test -p cli --test lambda regenerate_lambda_fixtures -- --ignored`.

```sh
cargo run -p cli -- lambda-compile examples/lambda/identity.lambda
cargo run -p cli -- run examples/lambda/identity.graph --max-steps 20
cargo run -p cli -- replay examples/lambda/identity.trace.json --at 2 | cargo run -p cli -- lambda-readback -
cargo test -p cli --test lambda
```

The browser’s **Lambda program** section prepares the same graph and automatically adopts the selected fixture's reference run settings. Custom closed and symbolic terms use the bounded lambda profile. Explicitly edited run settings take precedence until restored; **Load program** applies the displayed effective values. The interpretation follows the history slider. Administrative rewrites may leave it unchanged, even after it already displays `I`. A source result alone does not establish engine normal form. These examples use the portable default 1,000,000 search-work cap; larger terms can exhaust it. The broader native correspondence tests use a separate 50,000,000 cap and do not establish browser capacity.
