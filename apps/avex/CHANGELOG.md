# Changelog

All notable changes to **Forge** are recorded here.

> **Note:** Versions up to **0.4.5.1** were reconstructed after the fact from git
> history (no notes were written at the time), so their entries are inferred from
> the actual file changes. From **0.4.6 onward**, each `Version x.y` commit gets a
> real note written here at release time.

---

## What's new (user-facing)
### 0.5
- animations polish
- onboarding
- Intelligent plan making v1
- Auto-coach: a weekly brief that reviews your week and proposes plan changes you can apply or undo.
- Workout fixes: tapping a finished/skipped exercise no longer hides others; the rest timer now sits right under your sets instead of below the controls.
- Session time is now tracked per sitting — leave and resume a workout and your real active time adds up (13 min + 40 min = 53), instead of the clock restarting.
- Exports now include your notes, journal, per-set RPE, effort ratings, mood, and the per-sitting time breakdown.
- New "You" hub (account icon on the home screen): lifetime stats, progress photos, on-this-day memories, trophy points & near-misses, and recaps — all on your phone, no account needed.
- The "You" hub leveled up into a **rank ladder**: earn XP for every workout, set, PR, lb moved and week you train, and climb 30 ranks across six forge tiers — Ember → Iron → Steel → Tempered → Forged → Damascus. Tap the rank bar to see exactly how XP works. A new **Standing** card estimates where you'd place vs typical lifters on consistency, streak and weekly volume (an offline estimate — nothing leaves your phone), plus a **Signature** card (top lift · most-logged day · usual hour) and a profile photo.
- "Beat the ghost": every set is now a duel with last session — a live "beating last: N/M sets" scoreboard during the workout, a "beat 45×10" target on the input row, and a duel result + confetti when you finish strong.
- Bigger finish: confetti now also fires for a best-ever session or a clean sweep of the duel.
- Streak hook on the home screen — a "🔥 N-day streak — keep it alive" line (forgiving: rest days and vacations never break it).
### 0.4.5.1
- Smoother animations throughout the app.
- Faster, snappier active-workout screen (only the exercise you touch refreshes, not the whole list).
- Stats load and recalculate faster.

### 0.4.4.1
- Internal cleanup — removed unused code. No visible changes.

### 0.4.4
- Numeric keypad for entering weights and reps.
- Safer database upgrades so your data survives app updates.
- Backup improvements.

### 0.4.3
- Brand-new Stats dashboard with 5 tabs: Snapshot · Strength · Volume · Effort · Body.
- Interactive estimated 1-rep-max.

### 0.4.2
- Small UI tweaks on Overview, Trophies, and the exercise cards.

### 0.4.1
- Tap an exercise to see its own progress chart during a workout.

### 0.4
- Mark sets as AMRAP, assisted, drop sets, and other advanced set types.

### 0.3.x
- Editorial "Pearl" visual redesign (clean, card-free look).
- Onboarding, monthly recap, program editor, full Settings screen, home-screen widget.
- Bodyweight log, goals, backups, PDF export, session history, notes search.

---

## Developer history (reconstructed)

### [0.5] — Auto-coach + active-workout fixes (in progress)
- Auto-coach (5 phases): DB load ceiling, Week Brief + shadow planner, suggestion-outcome
  calibration, propose/apply/undo + outcome watcher, earned autopilot. Pure systems in
  `domain/coach/`; DB v16→19.
- Active-workout fixes: `DaySessionContent` no longer orphans incomplete exercises when a
  later done/skipped one is opened (UP NEXT now lists all remaining work, not just forward
  of the shown index); the inline rest timer moved inside `ExerciseCard` (under the set log).
- **Per-sitting session timing** (DB v19→20): `session_segment` table + `session.active_seconds`.
  A workout spanning "resume later" sittings sums real ACTIVE time (segments) instead of the
  clock resetting on each open; `finishSession` stamps the total; exports list the breakdown.
- Equipment correctness: `Equipment.INCLINE_BENCH` split from flat `BENCH` (MWM preset excludes
  it); removed Face Pull / Seated Low Row / DB Hip Thrust (not doable on Antho's gear).
- Richer exports: weekly + full JSON and the session PDF now carry notes, journal, per-set RPE,
  effort, mood, and the per-sitting time breakdown.
- Engagement layer: "You" hub (`ui/profile/`, all local) + **beat-the-ghost** (`DayUiState.ghostBeats/
  ghostComparable` via `beatsPriorSet`; live hero scoreboard, "beat" target, summary duel result,
  best/clean-sweep confetti) + a forgiving streak hook on Overview. New files only / `ui/gym/train`
  + `ui/overview` — no overlap with the concurrent equipment/library/onboarding work.
- **Rank & XP system** (`domain/rank/`, pure): `XpEngine` (earned XP from workouts/sets/PRs/volume/
  active-weeks/trophy points), `RankLadder` (6 tiers × 5 sub-ranks = 30 ranks, all thresholds in one
  tunable object), `StandingEngine` (offline "Top X%" estimate vs a documented population model — no
  server, no accounts). `ProfileRepository` assembles the snapshot + runs the engines (one fan-out,
  AdaptationRepository pattern) feeding a fully rebuilt `ProfileScreen` (rank track, ledger, standing,
  signature, mirror-test photos, trophy-case grid with progress rings, on-the-record recaps, "How XP
  works" sheet, avatar via the Photo Picker). No DB migration — everything derives from finished
  sessions; avatar is an app-private file. Tests: `RankLadderTest` / `XpEngineTest` /
  `StandingEngineTest`. **Pending:** on-device check.

### [0.4.5.1] — Motion & performance polish
- Added animation system (`Motion.kt`).
- Incremental day-refresh architecture (`DayViewModelRefresh`) — refreshes only the
  touched exercise/set on the hot path instead of rebuilding the whole screen.
- Added `OverviewUiStateMapper`; split stats aggregation into
  `StatsEffortAggregations` / `StatsStrengthAggregations` / `StatsVolumeAggregations`.
- 25 files changed (+1200 / −833).

### [0.4.4.1] — Dead-code purge
- Removed orphaned/unused code: Vacation, Tutorials, Bodyweight sheet, weekly-cardio
  card, placeholder screen, and numerous unused stats/overview/trophy components.
- 48 files changed (+5 / −3033). No user-facing changes.

### [0.4.4] — Data safety + input
- Room migrations (`Migrations.kt`) + migration test.
- Numeric keypad for set entry (`NumericKeypad.kt`).
- Unit tests: weight parser, PR detector, volume calculator.
- Backup enhancements.
- 24 files changed (+859 / −123).

### [0.4.3] — 5-tab Stats dashboard
- New Stats tabs: Snapshot / Strength / Volume / Effort / Body
  (`StatsSnapshotExtra` / `StatsStrengthExtra` / `StatsVolumeExtra` /
  `StatsEffortExtra` / `StatsBodyExtra`, `StatsTabs`).
- Interactive 1RM.
- DB migration to schema v13.
- 36 files changed (+6492 / −406).

### [0.4.2] — Minor UI tweaks
- Small adjustments to Overview, Trophies, ExerciseCard, History sheet.
- 12 files changed (+66 / −69).

### [0.4.1] — Per-exercise chart sheet
- Added `ExerciseChartSheet` — in-session progress chart for a single exercise.
- 17 files changed (+547 / −161).

### [0.4] — Set-type annotations
- Schema + `LoggedSet` changes for AMRAP, assisted, drop sets, and advanced set
  types (myo-reps, rest-pause, etc.) — roadmap items 140–143.
- 22 files changed (+2543 / −259).

### [0.3.6.1] — Cleanup
- 1 file changed (−12).

### [0.3.6] — "No monolith files" split
- Broke large files into focused components across Cardio, Stats, Day handlers,
  Overview tiles, Settings dialogs, and Trophies.
- 43 files changed (+4059 / −4311).

### [0.3.5] — Refactor
- 9 files changed (+392 / −417).

### [0.3.4] — Overview & Settings sheets
- Day-edit sheet, history sheet, settings sub-pages + primitives.
- 23 files changed (+3685 / −1383).

### [0.3.3] — Small feature batch
- 9 files changed (+837 / −154).

### [0.3.2] — Minor tweak
- 3 files changed (+37).

### [0.3.1] — Editorial "Pearl" redesign
- No-card hairline visual overhaul: DayScreen, ExerciseCard, Overview, Trophies,
  Settings, and theme rewritten. Agenda-style design mockups added.
- 34 files changed (+6813 / −897).

### [0.3.0] — Analytics & retention wave
- Bodyweight log, goals + extended goals, program customization, rest-day/vacation/
  warmup tables, backup repo, PDF export, reset, sample-data seeder.
- Background services: auto-backup, weekly recap, foreground session service.
- Session history, notes search, PRs subtab, onboarding, monthly recap, program
  editor, full Settings screen, home-screen widget, confetti, empty states, haptics,
  advanced charts.
- 147 files changed (+15132 / −604).

### [0.2.0] — App becomes functional
- All core DB tables (Session, LoggedExercise, LoggedSet, Cardio, Mood, Trophies),
  repositories, and domain logic (PR detector, volume calc, weight parser, trophy
  evaluator, rest-timer controller).
- Full active-workout UI: set logging, rest timer, swap picker, warmup gate, session
  summary. First Stats screen + charts, Trophies, Overview.
- 102 files changed (+8574 / −117).

### [0.1.0] — App skeleton
- Navigation graph, Pearl theme, screen stubs (Overview / Cardio / Day list / Day /
  Trophies / Welcome), Room DB + Hilt DI + Clock abstraction.
- 45 files changed (+1153).

### [0] — Initial scaffold
- Project scaffold (3405 lines).
