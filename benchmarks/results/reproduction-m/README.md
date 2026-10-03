# Cross-environment reproduction evidence

The [protocol and interpretation](../../../docs/research/evaluation.md#loop-m--protocol-before-collection) distinguish semantic, proposal, binary and performance agreement. `report.json` is recomputed by `scripts/verify-reproduction.mjs`; the sibling directories contain the actual source bundles, histories, build metadata, measurements and diagnostic binaries. Failed evidence is retained, not overwritten.

```sh
node scripts/verify-reproduction.mjs --check benchmarks/results/reproduction-m/report.json
node scripts/reproduce-frozen.mjs benchmarks/results/reproduction-m-macos-02 NEW_OUTPUT
```

The first command checks retained evidence without executing archived source or building native binaries. The second rebuilds a reviewed source bundle and requires Rust 1.85.0, downloaded Cargo.lock dependencies, the wasm32-unknown-unknown target, Node 24.21.0, wasm-bindgen-cli 0.2.100 at `target/tools/bin/wasm-bindgen`, and permission for process-RSS measurement. Do not execute untrusted source bundles merely because their hashes agree.

| Collection | Role |
|---|---|
| reproduction-m-macos-01 | Incomplete sandbox-denied memory-measurement pilot |
| reproduction-m-macos-02 | Primary Apple M4 Pro collection |
| reproduction-m-linux-one / two | Independently provisioned Ubuntu jobs, no restored compilation caches |
| reproduction-m-frozen-01 | Single-command archived-source reconstruction on the same Mac |
| reproduction-m-historical-01 | Four reconstructions of the original 162-file source and three exact historical traces |
| reproduction-m-wasm-macos / linux | Raw/processed binary-section diagnosis and unsuccessful added-sysroot-remapping alternative |

Primary Linux collection: [37153895525](https://github.com/saeedshahani/csm500-2026-APR/actions/runs/37153895525), source checkpoint `6751446aa9f681740f71e4131a8599236e4e3498`. WASM diagnostic: [37154502350](https://github.com/saeedshahani/csm500-2026-APR/actions/runs/37154502350), diagnostic script checkpoint `93057ce9073ea580c251ff6d64e889999f9aeb9c`. The Mac diagnostic uses that same script; its actual build inputs come from the frozen primary bundle. The earlier workflow-validation failure [37153669653](https://github.com/saeedshahani/csm500-2026-APR/actions/runs/37153669653) collected no Linux data and is not a failed semantic experiment.

`browser.json` identifies the production-browser foreign-history roundtrip, source scope and built-asset digests; `browser.png` is the inspected viewport. Browser application acceptance additionally passed in [37153867814](https://github.com/saeedshahani/csm500-2026-APR/actions/runs/37153867814). The favicon 404 is recorded, not hidden as an error-free console. No private user data, credentials, Firebase export or cloud deployment is part of this collection.
