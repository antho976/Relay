# Full-app audit fixes

Base: `580519ef8d3d8ee1cc62724a7df18e585b09321c`. Implementation and automated validation complete; physical-device gates are listed below.

## Recovery and startup (A01–A07)

- Resume database sidecar cleanup even when the base rename already completed.
- Persist rollback direction and retain immutable originals until recovery settles.
- Preserve committed original WAL frames when reverting a restored database.
- Copy a consistent SQL snapshot when VACUUM INTO cannot produce the backup.
- Gate Room and DataStore until recovery and restored-schema validation finish.
- Run recovery and startup preference reads on IO while showing a private-data-free waiting state.

Validation: 45 focused recovery/startup tests passed, 0 skipped. This includes real SQLite WAL
reopening, interrupted recovery retries, schema/value snapshot copying, gated preference reads
and writes, and startup with the real Hilt application graph. Full-suite gates are recorded at the end; physical-device checks remain outstanding.

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

## Watch telemetry and visible state (A23, A24, A34, A35)

- Split buffered HR into 64-sample chunks. Updated phones accept legacy buffers up to 240 samples
  and acknowledge only after persistence; updated watches retain unacknowledged chunks for retry.
  Duplicate timestamps do not consume remaining capacity. Older phones retain transport-only
  compatibility. The existing four-minute in-memory watch buffer and session-end policy remain.
- Anchor timer receipt to the payload in the process repository and persist it across recreation.
  Reopening a timer screen no longer starts the payload's duration again.
- Publish the actual signed load adjustment in an additive DTO field, and label the watch home,
  tile and complication as LOAD. Legacy signed payloads remain readable; new phones leave the
  misleading legacy readiness field empty.
- Reconcile complete logged-set rows on the phone, including same-count watch RPE changes.

Validation includes full 200-sample ingestion, duplicate chunk retries, failed-write acknowledgement
refusal, timer remount/repository reconstruction, signed DTO/text compatibility and existing timer
and protocol suites. Bluetooth loss/reconnect, paired UI interaction and watch process death were
not exercised on physical hardware. This is not a durable unlimited HR queue.

## Coach calculations (A25, A26, A32)

Corrective reversals may undo an older coach decision after the user-facing seven-day undo window,
but only while that decision still owns the live slot. Failed corrections leave the original ledger
intact. Load rounding converts kg, stones and configured plates to canonical pounds before rounding
down, and never suggests a minimum weight above the requested reduction. Balance goals and project
scans count each exercise's history once even when it appears on multiple program days.

Tests cover day-14 correction and later user ownership, light stacks, kg/stones rounding, and
unchanged balance after duplicating a program slot.

## Session and profile UI (A27–A31, A39)

Timed-only sessions keep their per-exercise set tables in all detail lenses. The last exercise still
opens the add action. Profile name editing waits for actual focus before treating focus loss as a
commit. Date-picker seeds use the photo's local date encoded at UTC midnight. The imported Health
Connect reading is labelled LEAN MASS, matching its source.

Gallery pair selection precomputes calendar dates and keeps only three selected pairs rather than
allocating and sorting the entire candidate population. It preserves the former greedy ranking and
exclusion behavior against a full-sort oracle. Pair comparisons remain quadratic; no device frame-time
or speedup percentage is claimed.

Real Compose tests exercise the timed-only detail lenses, last-exercise expansion, name edit/commit,
and an unchanged 23:00 Toronto photo date. Randomized pair-selection comparisons and archives up to
1,000 photos also pass.

## Protection boundary (A33)

Disabling an active app/gallery lock requires the system credential. The write boundary rechecks the
process authentication state. Manual photo-containing backups and backup-folder changes use the same
boundary. Settings-only reset preserves the lock flags, timeout, privacy flag and recovery sentinel.
The existing authorized scheduled backup destination remains usable in the background.

A real SettingsRepository/AppLockManager regression verifies gallery-only denial, reset preservation,
photo-export denial, then success after authentication. Physical system biometric/credential UI was
not driven.

## History export/import and freestyle drafts (A36, A37, A40–A42)

All three JSON export routes share the same set serializer, including duration, assistance, tags,
RPE and completion time. The export menu accurately calls JSON training history and directs users to
Backup for a restorable copy including photos. Strong/Hevy CSV parsers preserve duration-only holds.
Freestyle drafts carry movement identity and bodyweight/timed shape, including unknown imported IDs;
legacy drafts with typed holds retain them. Weekly tonnage groups the full population before applying
the week limit, while the separate session-series helper retains its own 30-session default.

Tests round-trip a timed assisted drop set through single-session, weekly and full JSON imports,
import 90-second Strong/Hevy holds, resume custom/imported draft shapes across a unit change, and
compare eight complete equal-volume weeks beyond the former 30-session cutoff.

## Remaining bounded performance work (A38, A43, O03–O05)

Academy covers decode on IO at sizes bounded by rendered constraints (maximum 1,024 pixels on the
longest side), behind a serialized decoder and an 8 MiB decoded cache. Visible Coach state refreshes
on lifecycle return, training/input revisions and midnight, with derived work on Default. The hub
only collects this while the Coach page is selected. A real Hilt ViewModel/Room test retains the
same ViewModel, adds a workout while away and verifies the new count after return.

Overview removes duplicate init/resume reads, coalesces bursts and queues a fresh follow-up when a
request arrives during a read. Freestyle opening uses an EXISTS availability query; full template
history is subscribed only inside the open picker. Program lookup uses one immutable indexed snapshot
per replacement, preserving first-duplicate precedence and old seed-name fallback.

Tests cover bounded image sampling, refresh bursts plus an in-flight follow-up, and indexed lookup
precedence/replacement. Device cold-start, frame-time, memory and large-import traces remain unmeasured.

## Conservative cleanup

Removed `OverviewTiles.kt` (CardioTile, StatsTile, TrophiesTile) and `DayEditComponents.kt`
(NameSection, ExerciseEditRow, AddWarmupRow, AddExerciseRow): seven unused entry points and their
implementation, with no source/test callers and baseline release shrinker confirmation. Lowered the
design-debt ratchet accordingly. Intentionally retained launch scenes, mixed live/dead files and
remaining candidate helpers were not bulk-deleted. A no-reference scan alone is not sufficient proof
that every candidate should be removed, nor does stripped code cause release runtime lag.

## Final automated gates

- Phone: 1,680 tests; watch: 50; shared: 30. Zero failures and zero skipped tests.
- `:app:verifyRoborazziDebug` passed without replacing baseline images.
- Phone and watch release lint: zero errors/fatal issues; existing warnings remain.
- `:app:bundleRelease` and `:wear:bundleRelease` passed with release minification.
- `:app:compileDebugAndroidTestKotlin` passed; migration behavior is also covered by real SQLite/Room
  JVM regressions. This compile task is not an on-device instrumentation run.
- No physical phone/watch run, signing/upload, merge or release was performed.

One later rerun exposed Robolectric's intermittent native-loader collision while mixing default
SDK 36 and explicit SDK 34 sandboxes. The default test SDK is now explicitly 34, matching the
screenshot fixtures, and ordinary unit tests use a plain Application. The real startup and retained
Coach tests explicitly opt into ForgeApp; API-specific notification tests keep their SDK overrides.
This avoids unrelated background Hilt startup in DAO tests and keeps the test environment stable.

## Additional release-dependency findings

The push surfaced 53 existing GitHub dependency alerts. Resolving both release runtime graphs showed
three alerts affecting two shipped libraries: phone Guava 31.1-android (alerts 10/11) and watch
protobuf-javalite 3.21.8 (alert 13). Constraints now select Guava 33.3.1-android, matching the existing
watch version, and protobuf-javalite 3.25.5, the advisory's patched version. Shared catalog entries keep
these choices explicit.

The other alerted package coordinates were absent from both resolved release runtime graphs. That
is an exposure classification, not a declaration that every build/test-toolchain alert is resolved.
No broad toolchain migration or alert dismissal was performed. GitHub's default-branch alert count
will also continue to describe main until relevant changes are merged and its graph is refreshed.
