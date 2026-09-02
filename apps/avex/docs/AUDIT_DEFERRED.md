# What the audit passes did not fix

Everything from the 99-finding audit and the follow-up re-audit that is **not** in the merged
work, with the reason and what it would take to finish. Written so nobody has to re-derive it from
PR descriptions.

Two audits are tracked here. The **99-finding audit** and its re-audit come first; the
**2026-09-01 production source audit** has its own section at the end, and that is the one to read
for the current state of the work.

For the 99-finding audit: a third pass (H1, H5, H6, M3, M5, M7, M8) closed seven findings a re-audit
found still open — some fixed in the wrong place, some fixed on one path of two. None of them are
listed here as open, and the two rows they touch are annotated. **H2, H4 and M2 are unchanged**, and
for the same reason as before: they need a compiler this environment does not have, or a device.

For the 2026-09-01 audit: read that section, not this paragraph. It was rewritten on 2026-09-02
after an independent verification pass found the previous summary false — it claimed every Medium
and Low closed over a `main` that did not compile. What is closed, what is partial and what is open
is tabulated there, along with the CI runs each claim rests on.

Three categories, and the distinction matters:

- **Blocked** — I could not do it safely in the environment I had, not a judgement about whether it
  should be done.
- **Decided against** — I looked, disagreed with the recommendation, and left the code alone. These
  are the ones to overrule if you disagree.
- **Partially done** — the reported defect is fixed; a narrower case behind it is not.

---

## Blocked: needs a local Android build

Everything in this section is blocked by the same thing. `dl.google.com` is unreachable from the
environment these changes were made in (organisation network policy), so there is no Android SDK
and no Room compiler. CI is the only thing that compiles the project.

That is survivable for ordinary code — CI catches the mistakes. It is **not** survivable for a
Room schema change, because Room's exported schema JSON carries an `identityHash` that only the
Room compiler can compute. `MigrationChainTest` requires `app/schemas/…/37.json` to exist and match
the declared version, and a hand-written hash would break `MigrationTestHelper` — the very test
that proves the migration works.

### H2 — Wear command deduplication is not crash-safe

`CommandDeduper` keeps command IDs in a synchronised in-memory LRU, and `isNew()` records the ID
**before** the command's side effect runs. Two failure modes:

- Process death after a successful write but before the ack loses the ledger, so a retry writes
  again.
- An exception after `isNew()` but before completion suppresses every retry for the rest of the
  process.

`ADD_30` is also not idempotent, and the log path starts the rest timer before the database insert,
which widens the partial-effect window.

**What it needs.** A `wear_command` table storing the command ID and its outcome, written **in the
same transaction as the mutation it describes**. Then the handler becomes: if a row exists, replay
the stored ack; otherwise run the effect and record the outcome atomically. A crash before commit
leaves no row and the retry re-runs correctly; a crash after commit replays the ack — which also
fixes the lost-ack case.

Recording the outcome *after* the effect without a transaction does not work: it just moves the
window. The coupling to the Room mutation is the whole point, which is why this needs a table and
therefore a migration.

### H4 — live-session uniqueness is transactional but not schema-enforced

`SessionWrites` closes the known race in-process, and that is a real fix. It is not the requested
contract: nothing stops a direct DAO writer, an importer, a seeder or a future second Room instance
from creating two rows for one slot, or two sets at the same index.

**What it needs.**

1. A stored `effective_slot_key` column on `logged_exercise`. The key is `COALESCE(slot_id,
   exercise_id)`, and SQLite expression indexes cannot be declared through Room's `@Index` — an
   index Room does not know about fails its own schema validation on device.
2. `UNIQUE(session_id, effective_slot_key)` and `UNIQUE(logged_exercise_id, set_index)`.
3. Migration 36 → 37: add the column, backfill it, **de-duplicate rows that already violate the new
   constraints**, then create the indexes.
4. Exported `37.json`, plus migration and concurrency tests.

Step 3 is the one to be careful with. Existing databases can already hold duplicates — the UI
refresh path defensively collapses legacy duplicate slots, which is direct evidence of it — so a
migration that creates the index without cleaning first fails outright and takes the app with it.

**Why I did not do it blind.** A wrong `UNIQUE` constraint on the core write path is a
`SQLiteConstraintException` on every set logged, for every user, with no way to roll back a
migration that has already run. That is a worse outcome than the defect it fixes, and the
in-process transaction means the known race is already closed.

### M2 — no baseline profile ships

The benchmark's package target was corrected, but `app/src/main/baseline-prof.txt` does not exist,
so ProfileInstaller is a no-op and the profile buys nothing. The source comments say as much.

**What it needs.** Run the generator on a supported device or emulator, inspect the output, commit
it, and add a CI assertion that a non-empty profile is actually packaged — otherwise this silently
regresses to the same state.

---

## Decided against

### The conditioning island (finding 72)

`ConditioningRepository` has **zero** references anywhere, and `ZoneCoach`, `ConditioningPlanner`,
`AerobicBase` and `ConditioningProfile` are reachable only from it and from `ConditioningTest`. By
reference count it is about a thousand lines of garbage, and the audit recommended deleting it.

It is Engine E-A → E-D, built ahead of its surface:

- **E-A has shipped.** `ConditioningLoad.interferencePenalty` is what `ReadinessAdvisor` consumes
  for its cardio deduction, and `AcademyRepository` teaches it.
- E-B–E-D are complete and covered by `ConditioningTest`.
- `TodayDirective`'s own docstring names the dependency: *"cardio stays a suggestion until Engine
  E-B can actually prescribe one."*

The actual defect was that **nothing in the code said it was a hold**, which is why a sweep flagged
it. That is now written at the top of `ConditioningRepository`, the way `SessionBreakDao` already
does one layer down.

Say the word and it is a clean cut: four engines, the repository, and the matching half of
`ConditioningTest`.

### `SessionBreakDao` (finding 79)

Write-only *on purpose*, and it says so at length. `WorkoutRepository.logBreak` writes a row per
break; the rows are the raw material for a "where did the time go" reading, they are bounded by
session count, they cascade with their session, and dropping the write throws away history that
cannot be reconstructed. Left alone.

### `sessionNearDate` made tracked-only — flag this if you disagree

"On this day" reads like history, and the working rule is that untracked sessions stay in history.
I made it tracked anyway, because it is delivered as an engagement hook on the Overview and inside
the weekly recap notification, beside numbers that already exclude untracked work. A session
someone deliberately kept out of their record resurfacing a year later as a memory seemed like the
same category error as counting it.

It is a one-line revert if you read it the other way.

---

## Deferred: the rest of the dead-code list

Findings **73–78, 80, 82–93**. Coach types, Stats aggregations, orphan DTO/projection pairs, unused
DAO methods, dead UI files, Wear constants, Remotion leftovers.

Two of the nine dead-code categories turned out to be **live code with documented reasons** (the
conditioning island and `SessionBreakDao`). That is a bad prior for deleting the other twenty by
inspection, and without a compiler a wrong deletion is only caught by a CI cycle each time.

The pieces that were provably dead and self-contained were removed: the trophy near-miss path,
`WorkoutRepository.updateExercise`, `SessionDao.idsAtStart` and `countAtStart`, and 4.5 MB of
snapshot directories.

Finding **98** (about 60 MiB of tracked media, no LFS) is advisory and unchanged.

---

## Partially done

| # | Fixed | Still open |
|---|---|---|
| H3 | The finish commits as one transaction, so a crash leaves either "not finished" or "finished and complete". | Process death **between the commit and the side effects** still skips the Health Connect mirrors for that session. Converging it needs durable per-session outbox state — a column, so a migration. Rotation and the widget are self-correcting; the mirrors are not. |
| H5 | Every progression, reminder, milestone, comparison and prefill consumer is tracked-only. `SessionDao.lastFinishedDayKey()` — the last inclusive input to `resolveNextUp`, missed in the first pass — was filtered in the third. | `StatsViewModel.openDay()` stays inclusive **deliberately** — it is a history list. The heatmap above it is tracked, so the two can differ; that is a presentation question, not a data one. |
| M9 | A custom move's unit and hold times survive into a reused template, so a bodyweight movement no longer returns as weighted and a hold no longer returns as reps. | **Muscle is not recoverable.** No logged row stores it — it exists only in the draft that created the move. It defaults, now explicitly rather than by accident. Fixing it properly means a column. |
| 38 | The structural-minimum half (M8): the planner no longer works toward a cap its own slot count makes unreachable, and — since the third pass — neither does the number the Coach screen prints, which is where the unreachable cap was actually visible. | `PersonalProfile` documents caps as "clamped within ±35%" and the code *snaps* to a band edge — every qualifying muscle moves the full 35%, with no proportionality to evidence strength. Whether the doc or the code is wrong is a product decision that changes prescriptions for everyone with eight weeks of history on a muscle. |

---

## The environment constraint, stated once

No Android SDK, so nothing here was compiled locally. Every change was checked with a
dependency-free Kotlin structural checker, traced by hand to each of its call sites, and verified
against the constants and DAO projections it depends on rather than assumed. CI was the compiler,
and it caught what that method misses — a removed declaration with one surviving reference.

That is the honest reliability bound on this work, and it is the reason the Room migrations are not
in it.

The third pass is the evidence for the other half of that bound: a re-audit of merged work found
seven fixes that compiled, passed their tests, and did not fully close the finding. A structural
check proves a file parses; it cannot tell you that `startupPreferences()` was the second reader of
a preference whose flow you fixed, or that the cap you clamped is also printed on a screen. Reading
every call site is what catches those, and it is worth budgeting for on anything merged from here.

---

## The 2026-09-01 production source audit: where it stands now

**Reviewed and re-worked on 2026-09-02**, after an independent verification pass against
`origin/main` at `47ca1a9` found the previous version of this section to be false in several
places. What it said, and what was actually true:

| This document claimed | What was true at `47ca1a9` |
|---|---|
| Every Medium and Low closed | M-03, M-06, M-10, M-15, M-18, M-30, M-32, M-33 and L-06 were all partly open |
| Thirteen performance opportunities closed | P-01, P-02, P-06, P-13 and P-15 were reductions, not closures |
| No PR exists and CI has never seen the work | PR #165 and #166 had merged, and CI had seen and failed both |
| Done means reviewed and tested | `main` did not compile: `OverviewViewModel` and `ProgramBuilderViewModel` |
| Restore snapshots remain until Room opens | A resumed restore swept them BEFORE Room opened |

That is the failure this table exists to prevent recurring: a ledger that reports a state nobody
checked is worse than no ledger, because it stops the next person checking.

### The two blockers `main` was carrying

Both are fixed on this branch and both were compile errors, so nothing downstream of them had ever
run:

- **M-32** — six flows select `combine`'s vararg overload, whose transform takes one `Array<Any?>`,
  against six declared parameters. `:app:compileDebugKotlin` failed on it.
- **H-13** — `var dayDialog … private set` already generates `setDayDialog(DayDialog)`; the explicit
  action of the same name is a platform declaration clash, masked until the first error was gone.

### Ledger

Closed means the defect the finding describes is gone AND a test fails if it comes back. Partial
means the reported defect is fixed and a narrower case behind it is not — those are listed
underneath with what remains.

| Finding | State | Where |
|---|---|---|
| C-01 / H-01 restore | **Closed** | `RestoreApply` is a three-state protocol: membership published before the first rename, an explicit awaiting-validation state, sweeping only when nothing is in flight |
| H-02 block phases | **Partial** | `composedLoadScale` wires the phase into the per-set suggestion and the pre-session brief; Peak's test-week prescription is still copy |
| H-03 Coach apply | Closed | One Room transaction; its seven tests failed only because they raced the real `ForgeApp` |
| H-04 body fat | Closed in code | Play Console declaration and a real provider grant are external release checks |
| H-05 Health Connect history | **Closed** | `HealthConnectFeatures` gates the request AND the retry affordance; unsupported and declined render differently |
| H-06 volume response | **Closed** | `ExerciseBout.performedExerciseId`; e1RMs group by the lift performed, muscle still by the slot |
| H-07 weekday schedule | **Closed** | `Placement.TODAY / UPCOMING / UNSCHEDULED`; only TODAY may open a session |
| H-08 wrist commands | **Partial** | The watch side is durable; the phone-side exactly-once window is open — see below |
| H-09 – H-12, H-14, H-15 | Closed | Unchanged by this pass; the verification found no contrary case |
| H-13 Program Builder | **Closed** | The setter clash, and the picker's query and ticks through `rememberSaveable` |
| M-03 import correction | **Partial** | The print now covers the timings, counts and mood it writes; source-identified replacement is open — see below |
| M-06 generation saga | **Closed** | Intended after-signature + operation id + one mutation mutex; a third signature is superseded, not applied |
| M-10 wrist edit recovery | **Closed** | The pending edit is a file, written before the optimistic removal, cleared only on an ack |
| M-15 Cardio time zone | **Closed** | The zone travels with the day anchor; a doctrine test bans a remembered one anywhere in `ui/` |
| M-18 folder grants | **Closed** | One owner for persisted trees; a take that fails changes nothing |
| M-30 widget routing | **Closed** | `Program.readiness` replaces the four-second poll; the request survives recreation in saved state |
| M-32 Home + Cardio clocks | **Closed** | The compile blocker, and Cardio's own goals and deadline captions on the day signal |
| M-33 bodyweight baseline | **Closed** | The first weigh-in at or after the goal's creation, read back after the write |
| Other M / L rows | Closed | Unchanged by this pass |
| L-06 goal pins | **Closed** | Cleanup behind `GoalRepository.clearGoal`; reconcile before the cap; `take` so Undo keeps what it restored |
| P-01 export | **Closed** | Four batched queries, streamed through `JsonWriter` to a scratch file, cancellable, with progress |
| P-02 wear glance | **Closed** | A capability listener, one publish per arrival, one shared fact assembly |
| P-06 Coach loads | **Closed** | The invisible loads are gone; each action still refreshes what it changes |
| P-13 sparkline | **Closed** | Geometry built once per (series, size) instead of once per frame |
| P-15 Home inputs | **Closed** | Three dead inputs and five dead state fields removed |
| P-03, P-05, P-07, P-17 | Open | Unchanged — see the section above |
| P-09 image resize | Closed in code | The oversize-bitmap fallback still needs a device pass |

### Still open, and why

**H-08 — the phone-side exactly-once window.** The file ledger records a command's outcome AFTER the
Room mutation commits, so a process death in the milliseconds between them re-runs the command on
the watch's retry. Closing it means a `wear_command` result row written in the SAME transaction as
the mutation, and replaying the stored acknowledgement for a duplicate id instead of executing
anything.

That is a new table, which is a Room migration, which needs the Room compiler to emit
`app/schemas/…/37.json` with an `identityHash` only it can compute — see **The environment
constraint** above. Hand-writing that hash would break `MigrationTestHelper`, the very test that
proves the migration works, and a wrong one is a "cannot verify the data integrity" crash on every
launch. It is not attempted here. The watch half IS closed: the pending edit survives the process,
comes back as an offer to re-send under its original id, and is retired only by an acknowledgement.

**M-03 — correction versus a second workout.** The duplicate print now covers every field the insert
writes, including the session's end time, active duration, PR count, mood and each set's completion
instant, which it did not before — so a corrected export is no longer silently discarded. It still
lands BESIDE the original rather than replacing it, because a session row carries no source
identity to replace by. That wants a `source_ref` column (source-install namespace plus the exported
session id) and a transactional upsert, and it is the same migration blocker.

**H-02 — Peak's test week.** The phase now reaches the weight on the bar. A Peak week's test-day
behaviour is still descriptive copy rather than a tagged prescription the session can act on.

### The product decision this pass made

H-02 needed one and it is recorded here so it can be overruled: **the block phase's ambition and the
day's autoregulation multiply, and the product is clamped to 0.80–1.10.** They answer different
questions — the phase is what this week of the mesocycle asks for, readiness/intensity is how the
athlete is today — so both apply, and the clamp stops them compounding into a cut or a jump neither
asked for. With no active block the composition is the identity, so the majority of sessions are
unchanged. See `BlockPhase.composedLoadScale`.

### Test integrity

Nineteen tests failed at `47ca1a9`. Sixteen of them were the harness, not the product, and a suite
that reports defects that are not there costs a real investigation every time:

| Suite | What was wrong |
|---|---|
| `GoalPinTest` (×5) | One DataStore file shared by every test in the class; results depended on method order |
| `BackupFolderGrantTest` | Teardown closed the database and left the persisted URI grants and folder preferences |
| `CoachRepositoryApplyTest` (×7) | Robolectric booted the real `ForgeApp`, whose async `ensureLoaded()` replaced the process-global `Program` mid-fixture |
| `CardioDayAnchorTest` | Compared two cell lists that cannot differ — a future day and an untrained past day are the same value |
| `OnboardingDraftKeeperTest` (×2) | `advanceUntilIdle()` does not advance time for `backgroundScope`, so the debounced write never ran |
| `RestoreApplyTest` | A real regression, and the test asserting the old sweep asserted the bug |
| `DesignDoctrineTest` | A stale allowlist row for a violation P-16 had already removed |
| `DoctrineParityTest` | `ForgeShimmerHost` lived in `ui/common/` and DESIGN §8 did not name it |

### How this branch was verified

No Android SDK here either — `dl.google.com` is blocked by the environment's network policy — so CI
is the compiler, driven by `workflow_dispatch` on this branch rather than assumed. Every claim of
"closed" above rests on a CI run of this branch, not on a local build.

Two of those runs found compile errors this environment could not: a package move that left two
same-package callers without an import, and a `buildSet` whose element type inference could not fix
from a `null` branch. That is the reliability bound, again, and the reason the workflow now ends a
failed `Verify` by printing the compile errors and failing test names — a red run used to require
downloading an artifact to read.

**Nothing here has been run on a physical device.** Health Connect history availability, Program
Builder recreation, watch disconnect and process death, timezone rollover, and restore
validation/revert all still want a real device pass before release.

### Merge process

PR #165 and PR #166 both merged with `Verify` unstarted, and both broke `main`. The workflow-side
half is fixed on this branch — all three jobs check out an immutable commit rather than a transient
PR merge ref, which is why #166's late `Verify` failed before compiling anything. The rest is
repository settings and needs an admin: see `docs/CI_MERGE_POLICY.md`.

### Notes carried forward from the first pass

`docs/AVEX_PRODUCTION_SOURCE_AUDIT_2026-09-01.md` stays frozen as the original audit. It is the
statement of what was found; this file is the statement of what was done about it, and only this
file is edited as the work moves.
