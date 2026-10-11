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

## Ops (`crates/relay-bus/src/ops/gym.rs`)

| op | who | what |
| --- | --- | --- |
| `gym.summary` | any | when it was exported, totals, this week and the twelve before, the streak, recent workouts, records, the lifts done most, open goals |
| `gym.sessions` | any | workouts newest first, paged, optionally only those with a lift |
| `gym.session.get` | any | one workout: every exercise and set with its e1rm, the journal |
| `gym.lifts` | any | every lift, done most first, with its best set and its last four weeks |
| `gym.lift.get` | any | one lift's history, a point per session, records marked |
| `gym.cardio` | any | cardio entries newest first |
| `gym.series` | any | a chart's numbers (below) |
| `gym.import` | the person | read an export from a path on the PC; replaces the copy; emits `gym.changed` |
| `gym.reset` | the person | forget the copy; emits `gym.changed` |

Reads are `register_unlocked`. The import is staged: the file is read, parsed and written to
`gym.db` in `prepare`, outside the store's lock.

`gym.series` takes `measure` (`volume`, `sessions`, `sets`, `minutes`, `e1rm`, `top_weight`,
`cardio_minutes`, `distance`), `by` (`week`, `month`, `session`), `periods` (weeks or months, 12 by
default) and `lift` (needed for `e1rm` and `top_weight`). It answers `labels`, `days`, `unit` and one
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

- **The sidebar's Avex key** sits under Arbiter's.
- **Overview**: when the copy was exported, then this week (sessions of Avex's target, volume against
  last week, the streak, the last workout), twelve weeks of volume, recent workouts and records beside
  the lifts done most and open goals. With nothing imported, three steps and an Import key.
- **Workouts**: by month, 60 at a time. A row opens the **Workout** page: its figures, the journal,
  and each exercise's sets (warm-ups as W, RPE, AMRAP, the best set's e1rm in lime).
- **Lifts**: every lift with its best set and its four-week change. A row opens the **Lift** page:
  its best, its last four weeks, the e1rm over time with records marked, and every session.
- **Data**: import a newer export, forget the copy, and what is here.
- **The thread panel's Avex source** (beside Tally and Arbiter, and a chip in the message box):
  Overview, Workouts and Lifts; the limits line says "Reads only".
- The palette has each page and "Avex: import training history".
- A display smoke run opens a workout or a lift with `RELAY_NATIVE_AVEX=<id or name>` beside
  `RELAY_NATIVE_PAGE=avex-workout` or `avex-lift`, and shows a thread panel's source with
  `RELAY_NATIVE_SOURCE=avex`.

## Tests

`crates/relay-gym` tests the reader (the full export, the files it refuses), the readings (kg and lb,
untracked sessions, warm-ups, records, streaks, Sunday weeks, every series grouping) and the store's
round trip, against `crates/relay-gym/tests/avex_export.json`, a generated twelve-week export in
Avex's format. `crates/relay-core/tests/gym.rs` imports it across the bus twice, reads every op back,
refuses the weekly file, a Tally backup and a relative path, and refuses imports and resets to an
agent.

## Later, if wanted

- Avex could write `avex_export.json` into the backup folder its Settings already lets the person
  pick, and a sync app (Syncthing) carry that folder here; Relay would then re-import when the file
  changes. Avex would still never touch the network.
- Body weight and measurements would need Avex's export to carry them first.
