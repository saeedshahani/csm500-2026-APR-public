# Loop C evidence

This collection records complete calendar-dimension integration of all 12,330 rows of Sakar and Kastro's (2018) [Online Shoppers Purchasing Intention Dataset](https://doi.org/10.24432/C5F88Q), licensed CC BY 4.0. Original fields and duplicate rows are preserved; the output appends a month number. The approval intervention is authored, not observed in the source data. No source rows, private platform stores, credentials or cloud backups are published here.

`results.json` identifies the archive, CSV, source bundle, binary, hardware, protocol and five full local-platform jobs: one fixed warm-up, three fixed measurements and one approval-controlled observation. The complete independent CSV oracle accepts every job. Native RSS is sampled every 500 ms; it is not a hard cap or total API/browser memory. Job elapsed time includes repeated native verification and publication, excluding import and subsequent result retrieval. Those retrieval timings are separately recorded.

`sources.json.gz` contains the exact measured source snapshot as a JSON map of paths to base64 contents. It is evidence, not an instruction to overwrite a working tree. `alternatives.json` records nine smaller, single-observation comparisons; its two ten-second timeouts remain failures. Recurrence is disabled uniformly in those comparisons, unlike the complete-job collection. `components.json` separates one-process export and independent replay of each full policy; it does not isolate rewrite-only time. Both resulting trace digests equal their corresponding complete-platform digests.

`recovery.json` records a verified 5,000-event pause, service restart, exact final-trace equality after resumption, cancellation with no finished trace and a resource-limited result denied complete CSV publication. `browser.json` records the final bounded-table walkthrough, three page interactions, response sizes, viewport/theme checks, compiled asset identities and the downloaded CSV digest. Response-body timing includes server reconstruction and transport; the subsequent frame interval is not pure renderer CPU time. Automated checks are not a human usability study.

`failed-screening.json` and its source bundle retain the earlier coarse-signature comparison failure after approval, alongside its successful fixed-policy observations. The final collection uses the separately justified rule-structure screen. `failed-pilot.json` retains an earlier deadline failure collected with a different sampling interval and concurrent diagnostic work; no complete source bundle was captured for that pilot, so it is not reproducible timing evidence or the final acceptance collection. Its cause is not uniquely established.

Later diagnostic wording, response-size enforcement and result-table presentation refinements are covered by final functional checks, not silently attributed to the frozen platform timings. The measured native binary is unchanged by those adapter refinements. The maintained evaluation document explains the proofs, alternatives and remaining boundaries.

## Reproduce

From the repository root, build the release CLI and obtain the pinned UCI archive as `output/s6-datasets/shoppers.zip`; `collect-loop-c.mjs` checks its SHA-256 before using it. Choose fresh output paths. Never reuse these collection directories as a live developer store.

```sh
cargo build --release --locked -p cli
node scripts/collect-loop-c.mjs output/loop-c-recollection
node scripts/compare-loop-c.mjs output/loop-c-recollection/alternatives.json
node scripts/accept-loop-c-recovery.mjs output/loop-c-recovery-recollection
node scripts/measure-loop-c-components.mjs output/loop-c-components-recollection.json
node scripts/verify-loop-c.mjs
```

The collector needs loopback networking and POSIX process-RSS inspection. Run serially without concurrent test/build workloads. Recollection under another build or machine requires its own identities and cannot overwrite this record. The verifier checks internal consistency and retained source identities; it does not rerun or independently authenticate measurements.

For browser acceptance, start the large local service and production frontend, connect in Projects, import the full CSV, submit a graph-approved job and inspect its complete result. After a fresh Playwright CLI snapshot confirms the Calendar results controls on the first page, run `node scripts/accept-loop-c-browser.mjs PATH_TO_PLAYWRIGHT_CLI_JS SESSION NEW_RESULT.json`. Screenshots and CSV downloads go to ignored `output/playwright/`. Inspect the screenshots as well as the returned measurements.
