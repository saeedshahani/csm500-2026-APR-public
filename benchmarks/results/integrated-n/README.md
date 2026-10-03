# Integrated reassessment evidence

These are functional acceptance receipts, not time/memory benchmarks. The assessed source manifest covers production code, proof sources, tests, examples, lockfiles, build/deployment configuration and the workbench's bundled guide. Narrative documents and prior experimental outputs are not part of that candidate digest; documentation regressions and the claim audit assess them separately. The baseline commit identifies the parent checkout; the source manifest identifies the edited candidate.

The accepted receipts are `core-02`, `research-02`, `platform-02`, `web-02` and `proof-02`. Their common source digest is `f1ef417b87c2c7cc8240786ae4279fb3eb9710971ca315a540dbbcf333ed8aaa`. The five groups run 38 commands. Core includes 439 passing native tests and two deliberately excluded generation tests; research includes 90 script tests and live/frozen study verification; platform includes 49 tests and six isolated emulator integrations; web includes 31 unit tests, 24 parity cases and 33 browser workflows. Proof checking uses Lean 4.19.0 for admission, minimality and workflow contracts. These counts are not coverage percentages or disjoint mathematical proofs.

`research-01` is an incomplete attempt and must remain rejected. The historical workflow verifier correctly refused today's different binary. The new explicit `--live` mode preserves that `--native` restriction and tests compatibility instead: replay the old and fresh traces, compare all fields other than validated producer implementation identity, and record both producers. Forty-eight cases retain the same results and 1,058 committed transitions. An intermediate strict comparison also exposed the changed producer fingerprint; no historical trace was edited to conceal it. Other `*-01` receipts passed their original groups before the acceptance harness was finalised; they are not the accepted candidate receipts.

Run from the repository root with the documented Rust/Node dependencies, installed browser and Lean 4.19.0 on `PATH`. Run `core` before live research checks so release/debug binaries are rebuilt. Local platform/emulator and browser groups need localhost listeners. Browser acceptance uses port 4181 by default; set `PW_TEST_PORT` to another free port if needed. A collision fails; another server is never reused.

```sh
node scripts/integrated-review.mjs collect core NEW_CORE_DIRECTORY
node scripts/integrated-review.mjs collect research NEW_RESEARCH_DIRECTORY
node scripts/integrated-review.mjs collect platform NEW_PLATFORM_DIRECTORY
node scripts/integrated-review.mjs collect web NEW_WEB_DIRECTORY
node scripts/integrated-review.mjs collect proof NEW_PROOF_DIRECTORY
node scripts/integrated-review.mjs verify core benchmarks/results/integrated-n/core-02 --current
node scripts/integrated-review.mjs verify research benchmarks/results/integrated-n/research-02 --current
node scripts/integrated-review.mjs verify platform benchmarks/results/integrated-n/platform-02 --current
node scripts/integrated-review.mjs verify web benchmarks/results/integrated-n/web-02 --current
node scripts/integrated-review.mjs verify proof benchmarks/results/integrated-n/proof-02 --current
```

Every collection destination must be new and its parent must exist. Verification without `--current` checks retained command completeness and output integrity against the recorded manifest; it does not assert that today's checkout matches. Digests do not authenticate authorship. Functional groups were permitted to overlap, so elapsed command durations are not controlled performance observations. The production bundle's chunk-size advisory and browser colour-environment warnings remain in logs. No cloud deployment, new signed-user acceptance, final-candidate benchmark or final rendered dissertation is claimed.

A separate Playwright CLI inspection of the production preview loaded `independent/shared-read`, reached NormalForm after two events, and inspected state 0 while retaining both events. The retained `browser.png` shows the timeline's explicit read-only/history distinction. This manual check is additional to, not part of, the 33 automated workflows. One pre-existing missing `favicon.ico` request returned HTTP 404; no engine or application exception was observed. The missing icon is a minor static-asset defect, not a research limitation or evidence of failed rewriting, and remains for Sprint 7's release asset check.
