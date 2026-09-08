# Full-app audit fixes

Base: `580519ef8d3d8ee1cc62724a7df18e585b09321c`. Work in progress.

## Recovery and startup (A01–A07)

- Resume database sidecar cleanup even when the base rename already completed.
- Persist rollback direction and retain immutable originals until recovery settles.
- Preserve committed original WAL frames when reverting a restored database.
- Copy a consistent SQL snapshot when VACUUM INTO cannot produce the backup.
- Gate Room and DataStore until recovery and restored-schema validation finish.
- Run recovery and startup preference reads on IO while showing a private-data-free waiting state.

Validation: 45 focused recovery/startup tests passed, 0 skipped. This includes real SQLite WAL
reopening, interrupted recovery retries, schema/value snapshot copying, gated preference reads
and writes, and startup with the real Hilt application graph. Full-suite and physical-device
checks are not yet complete.

## Watch program resolution (A08–A09)

Phone refresh, watch display and watch logging share the customized session-plan resolver.
The mirror observes customization and program revisions. Watch logging/mirroring and reminder
and recap workers load the saved program before resolving it; unknown day keys are rejected.

Validation: 11 focused plan, current-slot and application-startup tests passed. Customized
removal, addition, ordering and prescription updates are covered with real Room. Paired-watch
cold wake and live synchronization remain device checks.

## Atomic watch outcomes and finished-session guard (A10–A11)

Room v37 stores a watch command's acknowledgement in the same transaction as its mutation.
Duplicate deliveries replay it after transport or timer failure. Completed legacy ledger entries
remain recognized. Timers and acknowledgement publication run after commit. Live set insertion
checks its parent session inside the writer transaction and rejects a finished or missing parent.

Validation: 21 focused tests passed, covering concurrent duplicate deliveries, failure after
mutation, lost acknowledgement, post-commit timer failure, upgrade compatibility, and races
between session finish and set insertion. A real v36 SQLite fixture opened through the production
Room builder retains its workout history after migration. Android migration tests compile;
instrumented execution and paired-watch behavior remain unverified.

## Engine performance and freshness (A12–A16, O02)

History assembly and adaptation, weekly-coach, lesson and directive evaluation run on Default.
The unsafe cross-request time-only cache is removed. A directive passes one snapshot through
readiness and life-event assessment, avoiding the former repeated finished-history reads.
Stats listens to actual finished history and other engine inputs, with duplicate suppression and
coalescing. Rolling charts refresh at date boundaries; planned targets observe effective program
customizations independently of heavy history aggregation.

Validation: 14 engine/coach tests passed. New Room tests cover same-count set edits, suppression
of unfinished-session journal edits, fresh snapshots after writes/deletes and forward/backward
clock changes, and a retained stats subscription across midnight and plan edits. A mixed-SDK
Robolectric run failed in native loading; the consistent-SDK group passed. No device frame-time
benchmark or speedup is claimed.

## History, resets, freestyle recovery and release guard (A17–A22, O01)

Historical comparisons select the predecessor before the viewed finish time with an id tie-break.
Weekly count, volume and dots consistently attribute a workout to its start. Settings reset
preserves custom exercise/type definitions and freestyle drafts. Storage counts the shared export
directory. The release workflow only overwrites an existing draft; published or unknown states
are refused. No release was created or changed.

Freestyle drafts retain a UUID through autosave/resume. Room v38 atomically commits that identity,
the exercise/set graph, and finish totals. Retries reuse the saved session; draft resume recognizes
consumed identities and retries idempotent health mirrors after commit. Session time/day indices
cover common history reads without a table scan or sort in the measured SQLite query plans.

Validation: 16 focused freestyle/history/reset tests passed, plus the production v36-to-current
Room migration and compiled Android migration cases. Four mocked release states verify creation
for absence, replacement for drafts, and refusal for published/unknown metadata. Device crash-kill
and Health Connect replay testing remain outstanding.
