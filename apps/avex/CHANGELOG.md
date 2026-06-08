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
