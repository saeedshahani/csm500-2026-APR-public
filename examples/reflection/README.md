# Version-aware execution

`versioned.graph` is compiled from `versioned.json` by `cli version-compile`. It contains seven ordinary object-mode rules, two implementation descriptions, a registry, one captured call and one request to start another call. The payload is the constructor `payload:sample-item`; its identity is preserved throughout evaluation.

The previous implementation, `DelayReturn`, passes through a `Pending` state before returning its argument. The replacement, `Return`, returns the same argument in one evaluation rewrite. A publication budget of one permits the replacement. This budget counts evaluation rewrites of a newly selected implementation; it excludes publication, spawning, old captured calls and host processing costs.

The gallery profile uses seed 0 and reaches normal form after five events. One call retains the previous version and the other captures the replacement. With a different schedule, spawning may precede publication: both calls then use the previous version, and completion takes six events. The output identities agree in either case. Neither scheduling choice retroactively changes a captured call.

In the browser, select `reflection/versioned`, load the program and step through the timeline. The Internal view shows `Registry` linked by `active` to the installed `Version`; each version's `body` link selects an operative description. Each `Call`, `Pending` or `Done` has its own `version` and `arg` links. The Rules view shows the fixed interpreter rules. No native rule representation is modified by guest publication.

For native use:

```sh
cargo run --locked -p cli -- version-compile examples/reflection/versioned.json
cargo run --locked -p cli -- export examples/reflection/versioned.graph --max-steps 8 --strategy seeded --seed 0 --output /tmp/versioned-trace.json
cargo run --locked -p cli -- trace-check /tmp/versioned-trace.json
cargo run --locked -p cli -- replay /tmp/versioned-trace.json | cargo run --locked -p cli -- version-readback -
```

Choose an unused trace filename: export does not overwrite existing files. Graph source replay can rename identities; identities are meaningful within the resulting graph or a verified trace, not across unrelated source loads.

Readback includes the installed implementation, captured versions and payload identities. It rejects missing, ambiguous and unsupported descriptions; it is not provenance authentication or proof that arbitrary edited rules obey the compiler's contract. A `delay_return` replacement with budget one remains pending because no publication rule matches it. Remaining calls may still finish, so normal form alone does not imply successful publication.

This finite interpreter demonstrates operative descriptions and continuity under a checked version change. It supplies no arbitrary code-update checker, numerical optimizer, diagnostic learner or new convergence theorem. The native tests enumerate the closed fixture's schedules, compare every admitted transition with reference admission and test replay, resource failure and installed-body mutation. The abstract proof is in `proofs/versioned/Publication.lean`; it is not a verification of the Rust implementation.
