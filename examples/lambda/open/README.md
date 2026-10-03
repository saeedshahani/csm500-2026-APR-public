# Open symbolic lambda evaluation

Profile: `open-cbn-v1`. Declare unresolved symbols explicitly; they remain names, not external function calls. Bound variables in readback are zero-based indices. Execution is weak-head call-by-name: it does not enter lambdas or evaluate arguments of an unresolved head.

```sh
cargo run --locked -p cli -- lambda-compile examples/lambda/open/query.lambda --profile open-cbn-v1 --free database,select > /tmp/query.graph
cargo run --locked -p cli -- export /tmp/query.graph --max-steps 30 --output /tmp/query.trace.json
cargo run --locked -p cli -- replay /tmp/query.trace.json --at 3 | cargo run --locked -p cli -- lambda-readback -
```

The query prepares `(select database)`. It does not execute a database query. The named context and profile are stored in the graph, so trace imports preserve interpretation without an external environment. Existing closed examples retain their default encoding and behaviour.

`evidence.json` retains independently checked substitution/readback sequences for every tree of sizes one through six over two declared symbols. Regenerate explicitly with `cargo test --locked -p cli --test lambda_open generate_open_evidence -- --ignored`; ordinary tests reproduce and compare it. This bounded evidence is not a general proof.
