# Independent rewrites

Both programs contain one object rule replacing a pending link with a done link. The two pending link records have distinct identities. In disjoint.graph they connect different endpoint pairs; in shared-read.graph they share both endpoints. Both applications also read the same unchanged rule representation.

The hand-derived prediction is the same in each case. There are two enabled occurrences and one unordered pair. Neither occurrence consumes an endpoint, rule record or the other pending link. Both residuals survive; their final carriers and corresponding events agree under the mapping that associates each fresh done link with the pending occurrence it replaces. Numeric fresh identities need not agree between schedules.

The two executed events have no direct positive-resource edge and the strict closure is empty. Shared reads do not add edges. The local pair has no directional invalidation, commutes=true and corresponding_events=true. These facts are checked separately: absence of a dependency edge alone does not establish independence in the general reflective system.

The expected.graph files were obtained by replacing the two data-level pending labels by done, leaving the rule description unchanged. The CLI regression exports both cases, checks all predictions and evaluates both possible first choices against those expected carriers.

```sh
cargo run -p cli -- export examples/independent/shared-read.graph --max-steps 2 --conflict-state 0 --output shared-read.trace.json
cargo run -p cli -- trace-check shared-read.trace.json
cargo run -p cli -- replay shared-read.trace.json --at 1
```
