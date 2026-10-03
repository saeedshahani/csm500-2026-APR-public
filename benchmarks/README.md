# Native performance evidence

`results/interventions-03/` is the Loop G bounded-provenance collection: eighteen finite cells, three process repetitions, two target queries and every selected prefix subset. Reproduce with `cargo build --release --locked -p engine --example interventions`, `cargo build --release --locked -p cli`, then `node scripts/intervention-study.mjs collect NEW_DIRECTORY` on macOS with permission for `time -l` counters. `verify DIRECTORY` checks hashes and independent Boolean expectations; `--native` repeats trace replay with the exact recorded binaries, while `--live` regenerates all cells using the current build without cross-platform timing claims. The [protocol](../docs/research/intervention-protocol.md) and [interpretation](../docs/research/reflective-recovery.md#condition-sensitive-interventions-loop-g) separate subset completeness from unrestricted causality. `interventions-01` retains a sandbox measurement failure; `interventions-02` is excluded because a concurrently rebuilt replay binary invalidated the collection's binary-identity premise. Neither is overwritten.

`results/s5-native/` is the reported collection. `results/s5-loop-check/` retains the preliminary collection excluded from reported results: inspection found that its owned-input Criterion commit loop also destroyed the temporary execution object inside timing. The final harness borrows that object and drops it outside timing. The separate process probe already excluded that destruction; the full matrix was repeated to retain one consistent final collection. This is an instrumentation correction, not a production-engine optimisation or a selection based on favourable timings.

Each directory retains the exact protocol, environment/binary/source digests, matrix, warm-up and measured process records, stderr from the RSS instrument, and machine-readable summaries. The final Criterion directory retains `new` samples and estimates only, not automatic comparisons against a preceding run. No statistical improvement over the preliminary collection is claimed. The final source/test fingerprint in the dissertation is broader than the collection's code digest and includes subsequent documentation and test evidence.

Build first, close browser/test processes, then run sequentially on macOS:

```sh
cargo build --release --locked -p engine --example measure
cargo bench --locked -p engine --bench scaling -- --noplot
node scripts/benchmark.mjs /private/tmp/reflective-study-new
```

The output directory must not exist. Use `--smoke` after the directory argument for a five-cell functional check without platform RSS measurement. That mode is suitable for CI, not performance reporting. The collector refuses an existing evidence directory. For Linux, only smoke mode is currently supported; a platform-specific memory protocol would be required for a new measurement campaign.

The study is specified and interpreted in [evaluation.md](../docs/research/evaluation.md#performance). Graph constructors, fixture loading and semantic checks lie outside region timings but inside process RSS. Keep ResourceLimit, inapplicability, StepLimit, normal completion and process censoring distinct. The `run-times.svg` plot marks bounded and failed-search outcomes explicitly.
