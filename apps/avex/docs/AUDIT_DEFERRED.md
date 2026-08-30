# What the audit passes did not fix

Everything from the 99-finding audit and the follow-up re-audit that is **not** in the merged
work, with the reason and what it would take to finish. Written so nobody has to re-derive it from
PR descriptions.

A third pass (H1, H5, H6, M3, M5, M7, M8) closed seven findings a re-audit found still open — some
fixed in the wrong place, some fixed on one path of two. None of them are listed here as open, and
the two rows they touch are annotated. **H2, H4 and M2 are unchanged**, and for the same reason as
before: they need a compiler this environment does not have, or a device.

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
