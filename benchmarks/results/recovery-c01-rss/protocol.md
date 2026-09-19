# Controlled reflective recovery: AV-03/C01

## 1. Question and selected scope

Can useful output become stable while graph-resident executable rules continue
to change? This experiment answers that question for a deliberately restricted
work-routing family. It does not establish convergence of arbitrary reflective
programs, recovery from arbitrary corruption, or a deployed application.

Two object-mode rank-zero processing rules assign different route labels to
pending items. Exactly one is active. Rank-one meta-rules exchange their activity
and replace both revision records. These are actual checked graph rewrites, not
external program reloads. Policy bodies stay fixed; changes to activation are
executable-description changes under the existing lifecycle semantics. There is
no learning, optimal routing or generally editable policy language in this case.

The earlier abstract model allowed lowest/highest selection and arbitrary switch
allowances. This instance instead permits any pending item, alternates two
different output policies and allows exactly one intervening switch. It is a
related restricted construction, not an implementation of that entire model.

## 2. Carrier and invariant

For a fixed finite set of n item constructors, each item has precisely one
self-link labelled pending, route-a or route-b. Its identity is preserved. A
unique ordinary control constructor owns a phase link to a separate Work or
Switch constructor, and a remaining link to a unary chain of Count constructors
ending at a fixed End constructor. The chain length equals the number p of
pending items. Count links are labelled next. Chain nodes are distinct from
payload, control and quoted syntax; the phase value is also disjoint.

The immutable construction manifest fixes every rule body, rank, mode, supervisor
activity/revision, item identity, policy marker, control and End atom. Only the
two processor activities/revisions, item statuses, phase and countdown change.
There is exactly one active processor. No unaccounted atoms are allowed.
Core configuration validity is required before this extra membership check.
Fresh dynamic identities are allowed; the manifest does not require the original
countdown nodes to survive a later, explicitly declared disturbance.

The finite variant contains two switch supervisors; the modulating variant adds
two idle supervisors. Each supervisor recognises its two targets through their
object modes and LHS membership of distinct quoted policy markers. The ordinary
data markers satisfy the corresponding processor patterns. All ordinary link
endpoints are Constructors: rule heads are accessed by typed metadata instead.

The rules have these effects:

| Precondition | Event | Effect |
|---|---|---|
| Work, p>0 | Active processor chooses a pending item | Change its status to that policy's route; consume the top Count and next link; advance remaining; change phase to Switch. |
| Switch | Corresponding switch supervisor | Exchange active processor; replace both revisions; change phase to Work. |
| Work, p=0, modulating variant | Corresponding idle supervisor | Exchange active processor and revisions; recreate Work phase without changing any item status. |

All consuming records and their private endpoints are matched, so there is no
dangling deletion in this invariant. Meta-target post-ranks remain zero below
the source's pre-rank one. Both changed targets receive fresh revisions. No
exception to typing, GC, quotation, atomic commit or freshness is introduced.

## 3. Progress, closure and observations

Let I be the invariant above, L the states with p=0, and c=1 in Switch and 0 in
Work. The common potential is V=2p+c. A processing event decreases p by one and
changes c from zero to one, decreasing V by one. A switch outside L changes c
from one to zero, also decreasing V by one. There is a successor at every
outside-L state: a processor can choose a pending item in Work; the appropriate
supervisor applies in Switch. Thus every maximal mathematical execution reaches
L. No fairness premise is necessary because *every* allowed outside-L step
decreases V. Runtime timeouts and finite bounds can stop a prefix before L.

Each case preserves I: status replacement preserves the item set, processing
removes exactly one Count pair, switching preserves the countdown, and activity
exchange preserves exactly-one-active with fresh revisions. All immutable atoms
are preserved. Within L neither processor can match; supervisors preserve all
item identities and status labels. Consequently the induced item/status graph
is constant after entry, including route labels. Complete carriers continue to
change through revisions and activity. This is observational stability, not
normality or complete-state identity.

For a fresh Work initial state with n>0, L is reached after exactly 2n−1 events.
The finite variant performs the last switch and reaches normal form at 2n.
For n=0 the initial state is already legitimate and the finite variant is normal.
The modulating variant has an indefinitely extendible mathematical execution
inside L, assuming unlimited fresh identities. Bounded tests show finite prefixes,
not physical infinite executions. Carrier size is 190+2n+2p in the finite
variant and 310+2n+2p in the modulating variant: processing removes two atoms,
switching has constant size. There is no accumulating rule-edit garbage.

Different item orders need not agree on route labels: with two items, either
can receive route-a first. All completed runs agree only on the coarser property
that the same items are processed. No confluence or unique final route assignment
is claimed. Stability within one run and agreement across runs are different.

A supported disturbance reopens completed items and extends the countdown by the
same number of fresh Count pairs, preserving I. It creates a declared new
execution baseline, not a concealed event in an existing trace. After the last
such disturbance the same proof applies; continuing arrivals and arbitrary
program/counter corruption are outside the guarantee. Merely relabelling a
completed item pending without updating the countdown is rejected, not repaired.

### Exhaustiveness of the family driver

The research helper first checks I, then lists every pending-item processor
assignment in Work, or the single applicable supervisor assignment in Switch or
idle Work. Object matching cannot see quoted syntax. Meta matching could inspect
quoted patterns, but its RuleHead/ModeObject/membership requirements identify the
two actual object heads: quoted head variables are not RuleHeads. Their unique
policy memberships fix the target order. A quoted control phase cannot be
consumed because its outside LHS/RHS memberships would dangle. Therefore no
additional admitted assignment exists in I. The complete indexed query is the
differential reference on small execution states; each family-selected assignment
still receives full preparation and commit. This is a manual case argument plus
finite regression evidence, not a mechanised matcher-completeness proof.

`checked-family-v1` is an experimental example driver, not an installed Runner
profile, a general optimised matcher or a portable scheduler continuation.
Recorded events remain replayable through the existing checked trace format.

## 4. Counterexamples and limits

The first construction attempted ordinary links to Nat countdowns/rule heads.
It failed endpoint typing. Replacing those with Constructor countdowns and typed
metadata access retained the original language rather than weakening validation.

A second candidate membership check allowed a phase link to a quoted Work atom.
That configuration can be core-valid yet deadlock outside L: object visibility
and meta dangling checks prevent progress. The check now requires phase disjointness;
the regression retains the core-valid deadlock as a rejected counterexample.

GC alone permits an extra active empty object rule to fire forever without
processing pending items. The family check rejects that extension. Conversely,
it also rejects a harmless unrelated constructor. It is therefore a conservative
recogniser of one fixed family, not a complete convergence decision procedure.
The manifest is trusted output from this generator, not an untrusted certificate
that can assert arbitrary code safe. There is no security authorization claim.

Earlier independent specification tests retain individually terminating policies
whose union cycles, unlimited zero-progress switches, and repair policies that
undo each other. This construction prevents those cases through its fixed
grammar and common potential, not through a stronger interpretation of NR_v.
Its contribution is a carrier-specific worked instance and executable checks of
an established ranking method, not a claim of priority for self-stabilisation.

## 5. Pre-collection diagnostic protocol

Protocol declared before collection. Build the locked release example. Test
finite and modulating variants at n=0,1,3,10,100,1000, with a 2n+4 step bound.
Collect one process per cell, no warm-up. This is a 12-cell diagnostic ladder,
not an inferential benchmark. Retain failed/censored attempts. Impose a 30-second
process-group deadline and 1-MiB output cap; no hard memory ceiling is imposed.
Record macOS process peak RSS where available, wall time, probe-region time,
separate initial core-validation/membership times, family enumeration including
membership, and full admission/commit time. Membership itself includes core
validation; those two initial timings are overlapping operations, not additive
subcomponents. Setup/build is excluded from probe time but included in process
wall/RSS. Post-state checking and other overhead remain in the total.

The independently predicted completed outcome is finite: 2n commits, n processed,
n meta-events, zero pending; modulating: 2n+4 commits, n processed, n+4 meta-events,
zero pending. First legitimacy is zero for n=0, otherwise 2n−1. Initial/final
atom counts follow §3. Check every successful row against these formulae.
Retain raw output, source bundle, binary hash, environment and frozen protocol.
Do not compare these timings with default complete matching as equivalent work:
the restricted driver exploits a certified application family and does not
retain a trace in this measurement. General-query and replay checks are separate.

Reproduction from the repository root:

```sh
cargo test --release --locked -p engine --test recovery
cargo build --release --locked -p engine --example recovery_probe
node scripts/recovery-study.mjs benchmarks/results/NEW-recovery-collection
node scripts/verify-recovery-study.mjs benchmarks/results/NEW-recovery-collection
cargo run --release --locked -p engine --example recovery_probe -- source 2 0
cargo run --release --locked -p engine --example recovery_probe -- modulating 10 24
```

`source` prints a loadable research program. It is not a new bundled workbench
catalogue entry; using the general browser/native Runner retains its usual
search, observation and stopping limits. A recurrence stop does not refute the
mathematical existence of a longer modulating execution.
