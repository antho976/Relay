# Avex in Relay

Avex ([antho976/Avex](https://github.com/antho976/Avex)) is Antho's gym app for Android: workouts,
sets, cardio, programs and a coach, all on the phone. It is the third source in the Threads space,
beside Tally and Arbiter (`docs/THREADS.md`): a sidebar key, its own pages, a panel beside a
thread, and `gym.*` ops a thread's agent reads.

Decided with Antho on 2026-10-09:

- **Exporting is enough.** Avex has no `INTERNET` permission, and its privacy policy says it
  "physically cannot upload anything". Live sync the way Tally does it (`crates/relay-remote`) would
  break that promise, so Relay reads the file Avex already writes instead. Avex is not changed.
- **The PC's copy is read only.** Workouts are logged at the gym, on the phone. Relay reads and
  charts them; nothing goes back, and an agent cannot change them.
- **Antho's own build may upload** (2026-10-11). A `personal` build type of Avex sends the same
  export to a cloud copy of Relay after each workout, and the PC takes it home from there
  (`gym.upload`, `gym.raw`; docs/CLOUD.md). The public build is unchanged and holds no INTERNET.
- **Each import replaces the copy whole.** The export is the phone's whole history, so the newest
  file is the truth; there is nothing to merge.

## The file

In Avex: **Settings → Export → Training history (JSON)**, which writes `avex_export.json`
(`BackupRepository.exportFullDataJson` in Avex, `exportVersion` 1). It carries every finished
session with its exercises and sets, cardio entries, coach goals, and a few settings (`useKg`,
`userName`, `daysPerWeek`, `firstDayMonday`). It does not carry body weight, measurements,
programs, photos or trophies; those stay in Avex's own ZIP backup, which Relay does not read.

`relay_gym::export::read` reads it as leniently as Avex's own `ForgeJsonImporter`: a missing or
mistyped field takes its default, a non-finite or implausible weight is dropped, a session without
a start is left out. It refuses, with a message saying which file to send:

- Avex's **weekly** export (`periodStart`, no `settings`) and its **one-workout** export (`session`):
  both carry the same session shape, but importing either would replace the whole history with a
  week or a workout;
- an export with a newer `exportVersion` than this build reads (`gym.too_new`);
- anything else (`gym.import`).

## Storage

`gym.db` beside the store (`money.db`'s and `arbiter.db`'s neighbour), its own SQLite file behind
its own mutex; the store's lock is never held for it. Tables `meta`, `session`, `exercise`,
`lift_set`, `cardio` and `goal`, versioned by `user_version` (`relay_gym::store::MIGRATIONS`). An
import replaces every row in one transaction. The engine keeps the loaded history in memory
(`crate::gym::Shelf`) and every reading is computed from it; the file is what survives a restart.

Weights are kept as Avex keeps them, in pounds, and shown in the person's unit (`unit`, from
Avex's `useKg`), weights to a tenth and volumes whole.

## Readings (`relay_gym::views`)

- A session's **volume, set count and record count are Avex's own**, so the PC says what the phone
  says. Per-lift figures are counted here from the sets.
- **Warm-up sets** (`setType` `warmup`) count toward nothing.
- **Untracked sessions**, which the person excluded in Avex, are listed and marked, and left out of
  every total, week, streak and series, as Avex leaves them out of its stats.
- **Lifts are grouped by the name Avex shows**, ignoring case. A lift may be asked for by a unique
  part of its name.
- An **estimated one-rep max** (`e1rm`) is Epley's, `weight × (1 + reps / 30)`, from sets of 1 to
  12 reps; past 12 it says little, so those sets have none. A set's best is by e1rm, then weight.
- **Weeks** start on Avex's first day of the week (`firstDayMonday`), in the PC's time zone.
- A lift's **change** compares its best e1rm of the last 28 days with the best of the 28 before.
- **Records** on the overview are the exercises Avex itself marked (`wasPr`), with that day's best
  set; a lift's history marks each session whose e1rm beat every earlier one.
- **As of a day.** `gym.summary`, `gym.lifts` and `gym.lift.get` take an optional `today`
  (`YYYY-MM-DD`): the weeks, the streak and each lift's 28 days count back from it instead of the
  PC's today. A copy exported last week read as of today has an empty week and four weeks with a
  hole in them; read as of the export's day it has the week the person last trained. `days_since_last`
  never goes below 0.
- **A lift without weight** (pull-ups, dips) has no best set and no e1rm; its row carries
  `best_reps`, the most reps in one set.
- **A workout knows its neighbours and each lift's last time.** `gym.session.get` gives `older_id`
  and `newer_id` (the workouts either side, untracked ones too) and, per exercise, `previous`: that
  lift in the last tracked workout before, in the shape of `gym.lift.get`'s history.
- **Goals.** Avex's lift goal is `lift_1rm` (an older export says `lift_target`), measured by the
  lift's best working e1rm; its `target_key` is Avex's exercise id. A lift goal's and a body-weight
  goal's `target_value` are in the person's unit, and a lift goal carries `best`, that lift's best
  set (found by exercise id, else by a unique part of its name). `totals.untracked` counts the
  untracked workouts that `totals.sessions` leaves out.

## Ops (`crates/relay-bus/src/ops/gym.rs`)

| op | who | what |
| --- | --- | --- |
| `gym.summary` | any | when it was exported, totals, this week and the twelve before, the streak, recent workouts, records, the lifts done most, open goals with each lift goal's best; optionally as of a day |
| `gym.sessions` | any | workouts newest first, paged, optionally only those with a lift |
| `gym.session.get` | any | one workout: every exercise and set with its e1rm and the lift's last time before, the journal, the workouts either side |
| `gym.lifts` | any | every lift, done most first, with its best set and its last four weeks; optionally as of a day |
| `gym.lift.get` | any | one lift's history, a point per session, records marked; optionally as of a day |
| `gym.cardio` | any | cardio entries newest first |
| `gym.series` | any | a chart's numbers (below) |
| `gym.import` | the person | read an export from a path on the PC; replaces the copy; emits `gym.changed` |
| `gym.reset` | the person | forget the copy; emits `gym.changed` |

Reads are `register_unlocked`. The import is staged: the file is read, parsed and written to
`gym.db` in `prepare`, outside the store's lock.

`gym.series` takes `measure` (`volume`, `sessions`, `sets`, `minutes`, `e1rm`, `top_weight`,
`cardio_minutes`, `distance`), `by` (`week`, `month`, `session`), `periods` (weeks or months, 12 by
default), `lift` (needed for `e1rm` and `top_weight`) and `today` (count back from that day, as
`gym.summary` reads a copy as of its export). It answers `labels`, `days`, `unit` and one
series whose values are null where a lift's best has nothing to read.

## The thread agent

`threads::AGENT_OPS` gives an agent every read above, and neither `gym.import` nor `gym.reset`. Its
prompt says the copy is only as new as the last export (and to say how old it is when that matters),
how to ask for an export when there is none, that e1rm is an estimate, that it cannot change Avex,
and that pain or injury is for a professional.

A chart block with `gym` instead of `query` asks `gym.series`:

```chart
{"type": "line", "title": "Bench press, estimated max", "gym": {"measure": "e1rm", "lift": "Bench Press", "by": "week", "periods": 16}}
```

The card reads its numbers when drawn and again on every `gym.changed`. A lift's line leaves out the
weeks it was not done and starts its axis near its lowest value, so progress shows.

## In `relay-native`

`apps/relay-native/src/avex_pages.rs`, styled by `css/avex.css` on top of Tally's and Arbiter's
classes.

- **The sidebar's Avex key** sits under Arbiter's. Three tabs: Overview, Workouts, Lifts.
- **How old the copy is.** Every page says when it was exported ("Exported Thu 8 Oct · 7 days ago").
  A copy exported before this week is *stale*: the Overview, the Lifts pages and the panel read it as
  of the export's day (the ops' `today`), so they show the week the person last trained rather than
  an empty one. The Overview then says "This copy is 7 days old. Workouts since then are on your
  phone.", names its strip "Week of Mon 5 Oct", and makes Import newer… its primary key.
- **Overview**: this week (or the copy's last week): workouts of Avex's target, volume against the
  week before (an empty week says so, never "−100%"), the streak, the last workout with its training
  and cardio minutes; twelve weeks of volume; recent workouts and records beside open goals (a lift
  goal with its best est. max against the target, and a meter until it is reached) and the lifts done
  most. A ⋯ menu beside Import newer… holds **About this copy** (exported, imported, file, span,
  workouts counted and untracked, sets, cardio, units) and **Forget the copy…**. With nothing
  imported, three steps and an Import key.
- **Workouts**: by month, 60 at a time, with the cardio and rest days between them by day. A row opens
  the **Workout** page: a quiet "‹ Workouts" back, Earlier and Later keys (and Alt+← / Alt+→), its
  figures, the journal, and one Exercises card: each exercise a sub-head over its sets (warm-ups as W,
  RPE, AMRAP, the best set's e1rm in ink) and a line comparing it with the last time ("vs Thu 1 Oct:
  +10 lb on the top set").
- **Lifts**: grouped **Moving up** (more than 1% on the four weeks before), **Holding** and
  **Slipping** (more than 1% down, or not done in six weeks). Each row shows "est. max" and the
  change as a chip, the only colour on the row: green up, amber down. A lift without weight shows its
  best reps. A row opens the **Lift** page: a sentence first ("Up 4.9% in four weeks, about +3 lb a
  week. Last record Mon 5 Oct.", or how long since the last record), then best set, best est. max and
  the four-week change, the e1rm over time with its highest and lowest values written on it, and every
  session.
- **The thread panel's Avex source** (beside Tally and Arbiter, and a chip in the message box): one
  scroll with no tabs. The copy's week (as of the export when stale, with "Your last export, 7 days
  old" and Import newer…), the last four workouts, the lifts moving most, and the newest three
  records, each block with a quiet way into Avex. A row does not leave the thread: it drafts a
  question into the open thread's message box in the person's words ("About Legs A on Thu 8 Oct: ")
  for them to finish. The limits line says "Reads only".
- The palette has each page and "Avex: import training history". An export older than the copy
  here (by its `exportedAt`) asks first, "Import an older export?": it would take the copy back to
  that day.
- A display smoke run opens a workout or a lift with `RELAY_NATIVE_AVEX=<id or name>` beside
  `RELAY_NATIVE_PAGE=avex-workout` or `avex-lift`, and shows a thread panel's source with
  `RELAY_NATIVE_SOURCE=avex`.

## Tests

`crates/relay-gym` tests the reader (the full export, the files it refuses), the readings (kg and lb,
untracked sessions, warm-ups, records, streaks, Sunday weeks, every series grouping, a summary as of
another day, a workout's neighbours and each lift's last time, best reps without weight, lift goals
against their lift's best) and the store's round trip, against
`crates/relay-gym/tests/avex_export.json`, a generated twelve-week export in Avex's format.
`crates/relay-core/tests/gym.rs` imports it across the bus twice, reads every op back (the summary
and the lifts as of the export's day too), refuses the weekly file, a Tally backup, a relative path
and a `today` that is not a date, and refuses imports and resets to an agent.

## Later, if wanted

- Avex could write `avex_export.json` into the backup folder its Settings already lets the person
  pick, and a sync app (Syncthing) carry that folder here; Relay would then re-import when the file
  changes. Avex would still never touch the network.
- Body weight and measurements would need Avex's export to carry them first.
