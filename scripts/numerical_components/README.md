# Checked numerical components

This optional local application computes success probabilities for a retry/delegation workflow and publishes a checked numerical receipt through the native graph CLI. It supports one finite module with up to four states, sixteen policies and one scalar boundary probability. It does not require a Python dependency installation.

From the repository root:

```sh
cargo build --locked -p cli
python3 -B scripts/numerical_components/adapter.py \
  scripts/numerical_components/workflow-request.json \
  --cli target/debug/cli --out /tmp/numerical-workflow-demo
python3 -B -m unittest discover -s scripts/numerical_components -p 'test_*.py' -v
```

The output directory must not exist. It contains the task/value/radius packet, ordinary graph source, portable trace, checked replay and command receipts. `certified` means the computed radius is at most the requested tolerance at the recorded logical snapshot. `uncertified` means insufficient accuracy evidence, not a wrong answer. `stale` means an expired, invalidated or contract-failed snapshot. Failures raise an error and never return a successful receipt; partial diagnostic output may remain. There is no hosted endpoint or browser UI for this component.

Each action declares retry weights Q, immediate success probability r and delegation probability s. All are nonnegative; sum(Q)<1 and sum(Q)+r+s≤1. For parent success probability p∈[0,1], solve F_i(x,p)=max_a(Q_ia·x+r_ia+s_ia p). The supplied example has two retry states and two choices per state. Change `mode` to `raw`, `compiled` or `direct` for the supported alternatives. Raw mode uses the declared bounded iteration count. Compiled mode checks an affine-envelope program over all p∈[0,1]. Direct mode enumerates policies and solves their linear systems. Direct solving is a strong baseline and is cheaper on the retained one-off task sample.

The checker recomputes the residual of the actual rounded output y. With q=max row sum, the error bound is min(1,‖F(y,p)−y‖∞/(1−q)). Exact rational arithmetic includes output rounding in that residual. Solver operation counts exclude residual/publication costs; native export and verification incur additional work. Input rationals are capped at 128 bits, output precision at 4–64 bits, raw iterations at 4,096, retained receipts at 64, packet size at 8,192 bytes and source size at 32,768 bytes. The default one-million native search-work and portable trace policies remain in force. Each CLI process has a 30-second timeout; checked returned trace/replay text is capped at 2 MB. These are bounded application safeguards, not a strict OS memory quota.

For continuing input, import `Component` from this directory in a trusted local driver. `compute` issues a receipt; `observe(boundary, sequence, tick, declared_movement=None)` checks a complete ordered sample stream and computes its variation internally. `assess(receipt, tolerance, now, max_age=32)` transports the radius using the task's boundary sensitivity. `publish` assesses and invokes the CLI under the same host lock. `revise` changes the task version; `depart` invalidates current service; `rejoin` starts a new incarnation. Old receipts become stale, while unknown/tampered receipts are rejected. A monitor violation latches until explicit reconfiguration. A full 64-receipt registry requires a fresh component session and computation; it does not silently evict evidence.

The task, host checker, session registry, serialized calls, logical time and completeness of sampled input are trusted. Hashes are content identifiers, not cryptographic signatures. `verify_packet` recomputes mathematical consistency from exported data; it cannot authenticate the host or physical measurements. Receipt origin is checked against the protected registry before publication. A restart does not import old receipts as authority. Native rewriting carries the complete packet as an opaque value and changes one status link. Trace checking verifies graph transport, not Bellman arithmetic. Publication is valid for `checked_at`; a historical certified file does not automatically stay current after later task/input changes.

The 18 unit tests cover the theorem's constant-map counterexample, finite-precision floors, tampering, stale receipts, task changes, departure/rejoin and input/clock monitoring. [Retained acceptance](../../benchmarks/results/numerical-components-01/campaign/results.json) adds 144 numerical cases, exact and interval comparison methods, 26 native campaign cases and a complete CLI example, plus the corrected Lean convergence proof. Infinite-horizon theory does not make this finite adapter an unbounded distributed executor or an arbitrary graph-repair system.

The corrected convergence proof is maintained at [proofs/numerical/LocalCycles.lean](../../proofs/numerical/LocalCycles.lean). The retained collection is an unchanged historical acceptance run, including exact source bytes and command/tool identities; it is not a new performance measurement of the current engine. Its 238 artifacts can be checked without any exploratory directory:

```sh
python3 benchmarks/results/numerical-components-01/source/review-response/reproduce.py --verify benchmarks/results/numerical-components-01
```

The same frozen script supports a fresh run with `--out NEW_DIRECTORY --cli PATH --mathlib PATH --lean-bin PATH`, using the bundled component sources. Lean/mathlib versions and hashes are recorded in the collection's environment and source metadata. The full theoretical domain and limitations remain in the formalism and dissertations.
