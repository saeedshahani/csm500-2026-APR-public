# Scalable-execution study

## Protocol fixed before collection (18 September 2026)

This study extends, rather than replaces, the retained 226-cell matrix and the
AV-02/C01 diagnostic. The candidate is the exact bundled working tree, not an
unmodified commit. Every collection must preserve its protocol, source bundle,
binary digest, environment, raw outcomes and stderr. Measurements are serial:
one warm-up process and five measured processes per cell. No concurrent builds
or test suites run during measurement. Report median, minimum and maximum; five
samples do not support a population-level performance claim.

The logical-size ladder is 1,000, 10,000 and 100,000 records or relationships.
The exploratory million-record stage is permitted only if the preceding stage
completes with a linear extrapolation below 15 seconds and 256 MiB, providing
headroom beneath the 30-second and 512-MiB process-group limits. Failure of this
gate is recorded as a forecast exclusion, not an execution result. RSS is sampled
every 100 ms for cancellation and separately measured as process peak RSS by
`/usr/bin/time -l` on macOS. Sampling is not a hard OS allocation limit. A timeout,
memory cancellation, malformed output or instrumentation failure is retained,
never discarded or interpreted as convergence. SIGINT cancels the active process
group and preserves completed samples. Input generation and loading are included
in process elapsed time/RSS; named phase time excludes fixture construction.

The explicit native graph envelope is 300,000 atoms, 600,000 fields, 32 MiB labels,
64 MiB source and 100 million validation work units. Search is capped at 100,000
candidates and 10 million work units per complete query. These are experiment
settings, not raised browser or API defaults. Raw matching, enabled-set admission,
bounded execution and import-only validation are separate results. A successful
100,000-record load cannot establish 100,000-record execution throughput.

Compare reference and indexed-compact queries on identical graphs, seeds and
complete-set selection. Work units differ between profiles and are not timings.
The compact path retains bindings after full admission and prepares the selected
binding once more on its immutable borrowed baseline. It neither short-circuits
enumeration nor bypasses validation. It supports the two built-in schedulers;
custom candidate-inspecting schedulers and portable Runner trace replay remain
on the reference profile. Retaining ordinary events in the experimental probe
is not exporting a portable trace with a certified terminal search outcome.

Fixed and guarded-enable work-item policies share the same pending-to-processed
domain operation. The reflective policy first consumes one authorisation token
and installs activity with a fresh target revision; no later policy changes are
allowed. Oscillation is an adverse control, not a failed termination theorem.
Vary event count independently of inert padding. Seeded structural fixtures vary
degree/sharing, repeated labels/symmetry, rule count and competing matches. The
independent domain operation is array status replacement, with the same item
identities and per-item status result. Its reduced validation/provenance duties
must be stated: it is not an equivalent reflective runtime.

## Dataset acquisition decision (before download)

Structural input: **ca-GrQc**, released by Network Repository; creators J.
Leskovec, J. Kleinberg and C. Faloutsos. The [dataset page](https://networkrepository.com/ca-GrQc.php)
identifies an undirected collaboration network. The repository's [data policy](https://networkrepository.com/policy.php)
permits sharing and adaptation with attribution and ShareAlike. The page does
not resolve a numbered Creative Commons licence in the inspected text: retain
the policy itself and do not invent a version. Attribute also Ryan A. Rossi and
Nesreen K. Ahmed, *The Network Data Repository with Interactive Graph Analytics
and Visualization*, AAAI, 2015. Local evaluation uses numeric vertex identifiers
only; no author identification, joining or profiling. Raw/derived data are not
committed to the repository. Any future redistribution must retain the original
notices and applicable ShareAlike terms, not the software licence.

Product input: C. Sakar and Y. Kastro (2018), **Online Shoppers Purchasing
Intention Dataset**, UCI, DOI [10.24432/C5F88Q](https://doi.org/10.24432/C5F88Q).
The [primary release](https://archive.ics.uci.edu/dataset/468/online+shoppers+purchasing+intention+dataset)
expressly provides CC BY 4.0. It contains 12,330 session rows; it is not a workflow
event log. Use row positions as local session identities, retain categorical
labels and numeric values, and do not link observations to people. This study
tests import and checked processing, not purchase prediction or marketing use.
Credit the creators and identify any projection/subset as a transformation.

BPI 2017 was not selected: the authoritative 4TU page returned access denial,
so its terms were not established here. A synthetic healthcare log was also not
selected because the inspected release did not expose a licence value. Public
availability and another repository's software licence are insufficient grounds
for assuming data redistribution permission. Acquisition scripts retain hashes
and source URLs; retrieval or checksum failure aborts rather than substitutes data.

## Interpretation and transfer obligations

Index soundness follows from necessary matching constraints: exact constructor
or record-role/label/arity domains and ordered-field incidence. Every surviving
assignment still passes the reference feasibility and admission checks.
Completeness follows because an actual match satisfies each restriction; all
remaining injective assignments are explored. Repeated fields, disconnected
patterns, inactive/quoted rules and applied-rule bindings require explicit tests.
Indexes are rebuilt for every query from the borrowed graph, so there is no
cross-revision invalidation assumption. Complete candidate sorting and unchanged
ChaCha8 selection preserve built-in scheduler results when both searches finish.
Equal budgets need not yield equal resource outcomes across profiles.

Structural sharing, persistent snapshots and incremental validation are evaluated
as alternatives, not silently enabled. Query-local indexes avoid stale caches at
the cost of rebuilding; compact retention removes simultaneous candidate graphs
but not their construction or repeated full validation. Incremental validation
would additionally need dependencies for quotation, live entry/exit, mode,
activity, revisions and affected-rule closure. Until that preservation argument
and negative differential suite exist, full validation is the fallback and the
only admission path. No constant-time rewrite or general subgraph-matching
complexity improvement is inferred from this restricted experiment.
