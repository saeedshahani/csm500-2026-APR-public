# One recorded rewrite

stable.trace.json is a one-event recorded-binding prefix from behaviour/stable.graph. It is deliberately not a scheduler-reproduction claim: settings.plan and report are null, status is recorded_prefix. Its source hash identifies the authored input bytes; the initial hash identifies the exact atom descriptions and allocator.

The initial carrier has twenty atoms. Event 0 replaces the pending data link at identity 19 with the done link at fresh identity 20; the next allocation is 21. The original value link and both endpoints remain. Its binding identifies the original rule's endpoint variables and pending-link template. The read set additionally retains the applied rule representation; the fresh record supplies the new label and both targets.

There are no direct dependencies because the trace contains only one event. No local conflict query is requested. The empty checkpoints list requests reconstruction, not permission to trust an arbitrary snapshot. Import derives state zero and the endpoint and checks the recorded transition through the engine. This small file is illustrative; the CLI tests independently generate full scheduled traces with both behavioural observers and compare their final carriers with hand-derived expectations.

```sh
cargo run -p cli -- trace-check examples/traces/stable.trace.json
cargo run -p cli -- replay examples/traces/stable.trace.json --at 0
cargo run -p cli -- replay examples/traces/stable.trace.json --at 1
```

To obtain a new scheduled trace, run the export command documented in docs/reference/trace-format.md. A source-build mismatch rejects scheduler reproduction; this explicitly selected prefix remains subject to checked binding replay under the supported semantic profile. Editing its event, revision, fresh identities or dependency claims is not a valid way to change the result.
