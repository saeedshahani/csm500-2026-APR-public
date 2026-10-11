# Understanding the graph workbench

## Start with the walkthrough

Choose Walkthrough in the navigation bar. One annotated program follows Order 1042 from pending to packed to dispatched. Numbered callouts identify its data, relationship, stored rules and results. The two rewrites execute in a dedicated instance of the workspace's WebAssembly engine. Each stage includes a research explanation below the graph, connecting the observed structure or change to the relevant question.

Use Next to inspect the program, Apply pack rule and Apply dispatch rule to execute it, then Check for more work to establish its stopping point. Back revisits recorded states without adding events. Restart walkthrough starts the sample again. Exit walkthrough returns to your workspace without replacing its draft or history. If the workspace was already running, opening the walkthrough does not pause it; use Stop first if you want to pause that run.

The journey connects persistent entity identities and addressable relationships to RQ1's representation question. The rule and event stages explain structural admission, application interpretation and inspectable execution. RQ2–RQ3 appear in a remaining-work argument: the scores 2, 1 and 0 decrease as the preserved order reaches dispatched. The final history supplies RQ4's concrete dependency witness, because dispatch consumes the packed record produced by the preceding event. The closing discussion places these observations alongside guarded self-modification, conditional recovery and RQ5's separate lambda evaluation result.

The sample's two object-mode rules remain fixed. Dispatch denotes a modelled status; physical fulfilment requires a service integration. Application labels receive their interpretation from the example's design. These boundaries delimit the demonstration's evidence. Annotated sample source exposes its complete program. Guided callouts are confined to this one program.

## Version-aware execution example

The `reflection/versioned` example publishes a one-step Return implementation while an existing call retains its two-step DelayReturn version. With the recommended seed 0 it completes in five rewrites. In Internal, follow the Registry's active relationship and each Call's version and arg relationships; Pending and Done show evaluation progress. Both calls retain their argument identities. Different scheduling may capture the old version twice and require six rewrites. The Rules view contains the fixed interpreter rules; publication changes the selected guest version. Its budget measures replacement evaluation steps, excluding publication and existing calls.

## Load and execute a program

In Workspace, choose an Example, then select Load program. Selection automatically supplies its reference settings: step bound, seed, recurrence detection, observer, search-work and candidate limits, and execution profile. Example notes lists its purpose and expected outcome. Selection updates the draft and pending settings; the loaded graph and history remain available until a successful load replaces the execution.

Each field edited in Run settings becomes an explicit override and is retained across subsequent example selections. This includes a blank deterministic seed and recurrence switched off. Untouched fields follow the new example. Apply example settings restores the selected example's complete profile; Restore recommended settings clears overrides for the current base profile. Custom source edits retain the current settings. Imported project settings are treated as explicit choices; trace import retains its recorded execution settings independently of the pending controls. You can validate an edited source before loading; a failed load retains the previous graph.

Step commits one rewrite. Run continues until the engine stops or you press Stop. Reset starts the loaded program again. The editor remains a draft: editing it does not modify a running graph. An Editor differs from loaded source notice identifies that difference.

Run settings apply to a newly loaded run. A blank seed selects deterministic order; a number selects repeatable pseudo-random scheduling. The absolute step bound limits commits, while search work and candidate limits bound the search for a next step. Commits per batch and execution pacing control how frequently the browser receives updates. Loaded run settings shows the configuration actually in use.

## Read the graph

An atom is one stored item. Its number identifies it; the number is not its value, importance or execution order. The graph contains data, relationships and rule descriptions together.

| View | Contents |
|---|---|
| Practical | Data items and eligible relationships, with rule syntax hidden |
| Internal | Stored atoms and their ordered fields, including rule descriptions |
| Rules | Each rule's matching, resulting and preserved patterns, rank and revision |
| History | Recorded events, resource dependencies and optional analysis |

Practical and Internal are two presentations of the same state. A practical line corresponds to a relationship record. In Internal, that record appears as its own atom, with fields pointing to its endpoints. Arrow labels 0 and 1 are field positions, not time steps. Only unquoted, non-rule ObjectLink records between constructor data items become practical lines; other data remains inspectable internally.

Dashed circles indicate quoted rule syntax. The rule head connects to its representation. Active means the rule is switched on; it still needs a valid match before it can execute. Check applicability and local conflicts performs that check. In Rules, L means the matching pattern, R the resulting pattern and K the preserved part. A revision identifies an exact executable version of a rule.

Select a circle or relationship to open its Atom inspector entry. The inspector shows fields, quotation and membership records at the selected state. An item removed by a rewrite can be inspected from its event or by returning to the preceding state.

## Navigate a dense graph

Fit frames the displayed drawing. Drag the background to pan, use + and − to zoom, or pinch on a trackpad. Ctrl+scroll zooms the graph; ordinary scrolling moves the page. With the graph background focused, arrow keys pan and 0 fits the drawing.

Disconnected parts of the displayed subset are spaced apart. Click a rule-group heading, or choose Rule group, to inspect that rule. This grouping is a drawing aid: filters and display limits can hide connections, so separation does not prove independence in the complete graph.

Labels: Auto reveals text as you zoom. Selecting or hovering over an atom highlights its immediate connections. All displays every label; Selected keeps local detail. Pin layout preserves existing displayed coordinates. These settings change presentation, never the program.

The canvas displays at most 250 atoms and 500 connections. Counts disclose truncation. Find an atom in the inspector, then use Focus selected to reach it even if it is outside the initial picture. Focus clears conflicting graph filters. Expand neighbourhood adds another hop; Show all leaves neighbourhood focus. The inspector searches all loaded atoms and pages results in groups of 100. A smaller picture does not mean the stored graph has lost data.

## Inspect the timeline

State 0 is the initial graph. Event 0 produces state 1; event 1 produces state 2. The slider, numbered state field and navigation buttons inspect those recorded states. Play history replays existing history; it does not execute rules. Return to Latest to continue an unfinished run. Execution from an earlier state is not an implicit branch.

Recorded rewrites lists events in pages of 50. Inspect before and Inspect after show both sides of a change. Produced and consumed identify created and removed atoms; preserved identifies retained ones. Visual movement is a layout change, not a graph rewrite.

History's arrows express recorded positive-resource dependencies. Shared reads alone do not impose an order, and a missing arrow does not prove independence. Ancestor and descendant queries inspect that relation. Alternative-match analysis is hypothetical, not additional executed events. If an analysis exhausts its allowance, no complete conclusion is available.

## Understand why execution stops

| Outcome | Meaning |
|---|---|
| NormalForm | No admitted rewrite remains at the endpoint |
| CycleDetected | The chosen observation repeated; hidden state may still differ |
| StepLimit | The configured commit bound was reached |
| ResourceLimit / ResourceExhaustion | An allowance prevented further execution or a complete answer |
| Breakpoint | Execution paused before a selected rule occurrence |
| Yielded / Paused | The run can continue from its endpoint |

A normal form is not automatically a correct application result. Check the expected output or the application's independent result report. A limit is not evidence of convergence. Small examples often need only one or two rewrites; others can run indefinitely. To study a longer cycle, disable recurrence detection and load a new bounded run.

Rule breakpoints pauses before a selected rule commits. Step executes that occurrence once; Run continues until another stop. Breakpoints follow rule-head identities through revision changes. Reset, load and import clear them; exported files do not save debugger selections. Recover engine reconstructs the last acknowledged state and debugger continuation, not unfinished work.

## Save and restore work

Export source saves the editor draft. Import source replaces only that draft. Export trace saves checked identities, events and execution settings; importing a trace replaces the execution, not the draft. Source names may be absent from imported histories.

Export project keeps the draft, loaded source when available, settings, verified history, inspected state, theme and view. Import project checks the source/history relationship before replacing the session. Layout coordinates, selected atoms and every open panel are not part of the project file.

Save draft locally uses browser storage. It is not a server backup and can be cleared or unavailable. Restore saved draft is explicit. Export files you need to keep. Light, Dark and System themes affect presentation only.

## Projects and background jobs

Projects saves immutable source versions and runs native background jobs. Choose Email account, create an account with a password of at least twelve characters, verify the email, and sign in. Reset password and Resend verification handle account recovery. Reloading requires reconnecting; saved projects and submitted jobs remain on the server.

Create a project, choose Use workspace draft in Save a new version, name it and save. Select the saved version and submit a job with suitable limits. Editing a workspace draft does not change a saved version. Fork version creates an editable successor while retaining its parent.

Runs shows job status and checked commit counts. Completed means the final trace is available; read its outcome to distinguish a normal form from a limit. Pause waits for a batch boundary. Resume continues the same history and remaining allowance. Cancel is permanent. Closing the browser does not cancel a submitted job.

Inspect graph and history verifies the result before opening it in Workspace and asks before replacing an existing run. Open source in workspace replaces only the draft, after confirmation. Download source, trace and checkpoint preserve separate artefacts. Prepare retry / what-if starts a new job from the beginning, not from the old endpoint. Comparison baselines show settings and outcomes; equal counts alone do not establish equal graphs.

Project access uses the other person's account identifier. Viewers can inspect and download; editors can also save versions and manage runs. Only owners manage membership. The hosted pilot has project, file, job and execution allowances; a busy or quota response is not a graph result. Avoid sensitive data and retain your own exports.

Local API token and Firebase test accounts are development connection modes, distinct from the hosted email account. Setup commands and emulator instructions are maintained in `platform/README.md`. Do not enter real account passwords in test emulators or commit API tokens.

## Datasets and lambda programs

Work-item datasets accepts the defined work-items-v1 schema, not arbitrary business JSON. Items have unique IDs and pending or processed statuses. Browser import is limited to 1,000 items and a 1 MiB file; server dataset import has its separate 256 KiB limit. Preview checks the mapping before loading. Tables use 25 rows per page and show counts for the inspected state.

Fixed processing replaces pending statuses. Guarded enable first activates a processing rule with a new revision, then processes items. Dataset loading supplies the processing profile and a step bound covering the pending items, any activation rewrite and a final applicability check. Explicit user overrides take precedence; the preview displays the effective bound. Search, memory and recording allowances continue to constrain larger datasets.

Reference v1, Indexed v2, Framed v3 and Joined v4 are execution profiles. The latter profiles reuse checked search or observation facts for supported shapes and fall back to general admission otherwise. They do not remove resource limits. Framed v3 supports the fixed self-status work-item shape; Joined v4 also supports a restricted dictionary lookup. Large native measurements are not browser or hosted capacity guarantees.

Calendar normalisation is available on the larger local API route. It maps shopping-session month labels through a dictionary while preserving all eighteen input columns. Its independent report checks every row; output pages contain 25 rows. This route is not enabled on the hosted pilot and is not a general CSV connector.

Lambda program prepares graph source and its recommended run settings, then Load program executes it. A selected lambda fixture uses its catalogue profile; custom closed and symbolic-input terms use the bounded lambda profile. User overrides remain in force, and a failed compilation retains the previous draft and settings. Closed and Symbolic inputs profiles use call-by-name evaluation to weak head form. Evaluation stops at a function or unresolved input. Readback uses binding indices: 0 is the nearest parameter. Unresolved names remain symbolic. Graph steps include bookkeeping as well as reductions, so the readable result can appear before the run finishes. Custom terms can require a different step allowance or exhaust browser resources.

## Errors and recovery

Validate draft reports source errors and marks their locations when available. Go to line selects the affected text. Failed validation or loading does not replace the current graph. Standard editor undo and redo remain available.

If the engine worker fails, Recover engine checks and restores the last acknowledged history. It cannot restore unacknowledged work. If application assets are missing or corrupt, reload after repairing the deployment. After an uncertain server submission, retry the unchanged request to reuse its submission key; after a page reload, inspect Runs before creating another job.

Graph-resident rules share one finite stored carrier with their data. Graph cycles and indefinitely continuing execution are possible within this representation. Matching and validation remain defined by the engine. The walkthrough demonstrates ordinary rewriting, preservation, termination of its particular program and a witnessed dependency. Broader convergence and reflective claims require the separately stated research assumptions.
