# Changelog

All notable changes to **Forge** are recorded here.

> **Note:** All versions through **0.8.5** were reconstructed after the fact from
> git history — no notes were written at release time — so their entries are
> inferred from the actual file changes. From **0.5 onward**, sub-versions are
> rolled up into their `major.minor` line (e.g. `0.8.0`–`0.8.5` → **0.8**).

---

## What's new (user-facing)
### 0.8
- **Auto-Coach.** Once a week a "Coach Brief" reviews how your week went (volume, PRs, fatigue, stalled lifts) and proposes specific changes — deload weeks, exercise swaps with named replacements, rep-range shifts, and ±1 set per muscle. Apply, skip, or undo each one with a tap. If you keep accepting a kind of change a few weeks running, it earns "autopilot" and starts applying itself — except deloads, which always ask first. A "NEW BRIEF" banner shows up on the home screen when a pass is ready.
- **New "You" hub** (account icon on the home screen): a 30-rank forge ladder across six tiers — Ember → Iron → Steel → Tempered → Forged → Damascus — with XP earned from every workout, set, PR and pound moved; an offline **Standing** estimate ("Top X% of lifters by consistency" — nothing leaves your phone); a **Signature** card (top lift · most-logged day · usual hour); progress photos; your trophy case with near-misses; and month/year recaps. No account needed.
- **Animated rank emblem** — your tier shows as a large glowing badge, with a hand-drawn flickering flame for the Ember ranks, plus a tap-to-open "How XP works" breakdown.
- **"Beat the ghost":** every set is now a duel with last session — a live "beating last: N/M sets" scoreboard, a "beat 45 × 10" target on the input row, and a duel result with confetti when you finish strong. Confetti also fires for a best-ever session.
- **Streak hook** on the home screen — a "🔥 N-day streak — keep it alive" line (forgiving: rest days and holidays never break it).
- **Full-screen session detail** — tap any past session for a summary strip (volume, duration, sets, PRs, avg RPE), an overview chart, and a card per exercise with its full set table and chart. Weight / Volume / Reps and Bars ↔ Line toggles restyle everything at once.
- **Richer recent-session rows** — each gym row now shows set count + real active training time (no more "0 min"), a status pill (DELOAD, TEST, TECHNIQUE…), the session's marquee lift (e.g. "Bench Press 185 × 5"), and a volume trend arrow.
- **Weekday scheduling** — assign each day of the week to a program day (or rest) in Settings → Program. The day list, home screen and widget all follow the same schedule; miss a day and it simply rolls forward (sequence mode is still the default).
- **The exercise library now fits any gym** — ~42 new movements (barbell big lifts, generic cable/machine, Smith machine, trap bar, kettlebell, resistance bands) and equipment options expanded from 7 to 14.
- **Progress photos** — import physique photos via the system photo picker (no permission prompt), stored privately on-device in a grid with full-screen view and delete.
- **Trophy badges animate when tapped**, and same-icon trophies get tier pips so duplicates are easy to tell apart.
- **Optional sex step in onboarding** feeds sex-aware strength standards on the Stats Body tab.
- **"Skip onboarding" now builds a real program** — a generic bodyweight program (behind a confirmation) instead of silently loading the developer's preset.
- **Program-change guard** — generating, deloading, or re-rolling your program mid-workout now warns how many logged sets you'd lose first.
- **Smarter set entry** — weight and reps pre-fill from what you actually did last time (not the plan target), the gold ★ marks the set that actually set the record, and volume figures respect your kg setting throughout the app.

### 0.7
- **New "COACH" feed on the home screen** — up to three plain-English recommendations: plateau ladders (a small load bump → rep-range shift → variation swap) and a one-tap "Generate deload week". Each card explains its reason and can be dismissed (muted for 14 days).
- **Smarter deload timing** — instead of a fixed session counter, deloads are suggested from a transparent fatigue score (effort creep, mid-session rep drop-off, e1RM regression, low-mood streaks, sick/sore flags, being overdue), and the brief lists exactly which signals fired.
- **The rest timer learns your pace** — it tracks how long you actually rest after each set and gradually nudges future countdowns toward your real rhythm, separately for compound vs isolation work.
- **"Suggested order" banner** before your first set when two same-muscle exercises sit back-to-back, with a one-tap apply (supersets are never split).
- **Daily readiness nudges** — on a normal day, mood, recovery spacing and recent volume fold into a small (±5%) tweak on that day's suggested weights; an explicit Light/Hard pick always overrides.
- **Big Stats expansion** — Snapshot gains a momentum grid, a progressive-overload sparkline, a "readiness pulse" and a "week vs plan" bar; a brand-new **Trends** tab (consistency heatmap, when-you-train, effort/RPE/mood trends, session length); a new **Strength** tab (e1RM projection, plateau flags, PR drought + dated timeline, movement-pattern radar); and Volume gains balance ratios and planned-vs-actual muscle targets. Many insights are now computed by the engine and only show when there's enough data.
- **Stats animate in** — cards fade up, charts draw left-to-right, big numbers count up (respects your reduced-motion setting).
- **Holidays** — add named date ranges in Settings; they bridge your training streak and pause the "overdue" advisor.
- **Program settings** reorganised into focused sub-pages (split & schedule, goal & experience, emphasis & priorities, auto-refresh).

### 0.6
- **Program Viewer** — tap "view program" to see your whole week, each day as its big spine word (PUSH, PULL, LEGS…); tap a day to expand its exercises and targets, with your next day highlighted.
- **Leave without losing work** — backing out of a workout now asks "Resume later" (sets kept), "Discard", or "Keep going"; an "IN PROGRESS" banner and "Resume session" button bring you back. Opening a different day while one's in progress explains the conflict instead of silently discarding.
- **Bodyweight & plate-count exercises** — bodyweight moves log reps only; cable/machine moves log a plate count, with a setting for how much one plate weighs.
- **Reps field pre-fills** the top of your target rep range.
- **"VIDEO" button** opens a YouTube how-to search for the exercise.
- **Done/skipped exercises** stay in a "DONE / SKIPPED" section you can re-open, and skipping an exercise now jumps you to the next one.
- **Rest timer fixed** — it no longer drifts or lags when the app is backgrounded.
- **"Accent emphasis" setting** tints big numbers and titles with your accent colour (Off / Subtle / Medium / Strong).
- **Onboarding rebuilt** into 6 grouped pages, each with a one-line explanation of what your choices change.
- **Add custom exercises** right inside the day-edit sheet.
- **Backups now include your settings**, and exports open the system share sheet (save to Files / Drive) instead of hiding in app storage.
- **Trophy refresh** — equipment-agnostic trophies replace dumbbell-specific ones, and near-miss tracking is fixed.
- **Journal note** — write a free-text note when you finish a session.
- **Weekly-stat fixes** — home-screen counts/volume and cardio now use consistent week windows.

### 0.5
- **Intelligent plan making** — Forge now generates your whole training program from your goal, experience and available equipment: the split, the exercises, the sets and the rep ranges — instead of a single hard-coded routine.
- **Onboarding builds that program with you** and seeds your first week.
- **Customise the generated program** from Settings (split, goal, experience, emphasis, priority muscles) and re-roll any day you don't like.

### 0.4.6
- New app icon.
- Groundwork for the program system and another safer database upgrade (under the hood).

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

### [0.8] — Auto-Coach, "You" hub, session detail, generalized library
- **Auto-Coach (5 phases), DB v16→v19, pure domain in `domain/coach/`.** Phase 0: `maxDbWeightLb`
  ceiling in `ProgressionAdvisor`. Phase 1 (v16→17): `AutoCoachPlanner` + `WeeklyReview` + `coach_pass`
  / `coach_decision` tables + `CoachBriefScreen`/`CoachBriefViewModel` (shadow mode — decisions shown,
  not applied). Phase 2 (v17→18): `suggestion_outcome` table + `SuggestionCalibrator` (per-exercise
  8-sample gate, FASTER/CONSOLIDATE). Phase 3 (v18→19): `coach_decision` apply lifecycle
  (`day_key`/`payload`/`applied_at`/`undo_data`/`outcome`); propose mode with coach-locked slots,
  concrete swap replacements, fatigue-gated volume ±1; `OutcomeWatcher` + `CoachRepository`
  apply/skip/undo/revert (every write routes through existing user-confirmed paths). Phase 4:
  `TrustLedger` (per-type accepted-streak, blast-radius thresholds 3/4), `COACH_MODE` pref, Settings
  Coach page, `CoachGenBias` (surviving applied changes feed `volumeBias`/`prefer`/`avoid` back into
  every `generate`/`rerollDay`). Overview "NEW BRIEF" banner.
- **Profile & rank system** (`domain/rank/`, no migration): `XpEngine`, `RankLadder` (6 tiers × 5 =
  30 ranks), `StandingEngine` (offline "Top X%"). `ProfileRepository` fan-out assembles `ProfileUiState`;
  `ProgressPhotoRepository` (`filesDir/progress_photos/` + JSON index, off-thread EXIF-rotated decode)
  and `AvatarRepository` (app-private photo) — both no Room table. `RankEmblem.kt` Canvas fire
  animation. Tests: `RankLadderTest` / `XpEngineTest` / `StandingEngineTest`.
- **Beat-the-ghost** (`DayUiState.ghostBeats/ghostComparable` via `beatsPriorSet`): live hero
  scoreboard, "beat" input target, summary duel result, best/clean-sweep confetti; forgiving streak
  hook on Overview.
- **Session Detail screen** (`ui/gym/session/`, no migration): `SessionDetailScreen`/`ViewModel`/`Charts`/
  `Components` fed by `StatsRepository.getSessionDetail(id)`; `SessionMetric` + `SessionChartStyle`
  toggles; reachable from history, the history sheet, and Overview recent rows (`Routes.SESSION_DETAIL`).
- **Per-sitting timing** (DB v19→20): `session_segment` table + `session.active_seconds`; segments
  sum real ACTIVE time across resume-later sittings; `Session.durationMinutes()` is the one canonical
  duration surface.
- **Overlay-source tagging** (DB v20→21): `source` column on `program_customization` /
  `exercise_customization` (`user`/`auto`) so regeneration clears coach-origin swaps but keeps user ones.
- **Swap re-attribution** (DB v21→22): `logged_exercise.slot_id` (exercise_id becomes the real performed
  exercise so PRs/stats attribute correctly; slot_id maps back to the day screen) +
  `exercise_customization.swapped_exercise_id`.
- **Equipment generalization**: `Equipment` enum 7→14 (BARBELL, SQUAT_RACK, SMITH_MACHINE, TRAP_BAR,
  EZ_BAR, KETTLEBELL, RESISTANCE_BAND, INCLINE_BENCH) + `ExerciseUnit.WEIGHT`; ~42 new exercises;
  `curatedOnly`/`pickBias`; `ExerciseLibrary.availablePool(...)` as the single filter for generator/swap/
  likes/engine. **Frozen "Developer's preset"** via `EquipmentPreset.frozenIds`. `ProgramChangeGuard`
  Singleton + host. `SessionType` enum. `WeightFormatter` locale-safety pass + `formatVolume`.
- Migration test grows to the full 12→22 chain; unit suite ~183→~282 tests.
- **172 files changed (+19224 / −741).** DB v16→v22.

### [0.7] — Adaptation engine + Stats overhaul
- **Adaptation engine (6 pure systems) in `domain/adapt/`** — deterministic, clock-free functions of an
  immutable `AdaptationSnapshot`: `ProgressionAdvisor` (double-progression + plateau ladder),
  `RestAdvisor` (realized-rest tuning per movement role), `OrderingAdvisor` (fatigue-aware, superset-safe
  ordering), `InsightEngine` (10 confidence-gated observations), `DeloadAdvisor` (multi-signal additive
  fatigue score, replacing the fixed 24-session counter), `ReadinessAdvisor` (bounded daily ±% scale).
  `RecommendationArbiter` resolves conflicts; thresholds centralized in `AdaptThresholds`.
  `AdaptationRepository` is the only impure piece (snapshot fan-out off the per-set hot path).
- **DB v15→v16**: `rest_event` (planned vs realized rest per set) and `advice_event` (shown/applied/
  dismissed for cooldowns) — both additive. Rest capture wired through `DayTimerHandlers`.
- `DayViewModelBuilders`: old `computeWeightSuggestion` deleted for `ProgressionAdvisor.suggestNextLoad()`
  (unit-correct — fixes the plates ~15× bug).
- **Stats restructured**: new `StatsStrengthTab` / `StatsTrendsTab` extracted; `StatsEngineUi.kt` (pure
  engine→UI mapping), `StatsMotion.kt` (staggered entrance kit, `CountUpText`, draw-in progress),
  `StatsPrTimeline.kt`. Many speculative `StatsUiState` fields deleted and replaced with engine-backed
  ones (`overload`, `prRecency`, `patternRadar`, `readinessPulse`, `plateauFlags`, `balanceRatios`,
  `plannedSetsByMuscle`, `weeklyTonnage`, `trainingTimes`, `weeklyDurations`).
- `VacationCalendar` (`domain/vacation/`) + `VacationRepository` + `SettingsVacationPage`;
  `SettingsProgramPage` splits the program section into 4 sub-pages. `VolumeTargets` (planned weekly
  sets/muscle). 13 new engine test files.
- **103 files changed (+8175 / −1543).** DB v15→v16.

### [0.6] — Program viewer, session flow, bodyweight/plate modes
- New `ProgramViewerScreen` (read-only accordion over the active program; `Routes.PROGRAM_VIEWER`).
- New `Emphasis.kt` theme system (`AccentEmphasis` OFF/SUBTLE/MEDIUM/STRONG + `emphasized()` helpers),
  applied across Overview/Stats/Recap/Settings/ExerciseCard.
- `ExerciseUnit.BODYWEIGHT` / `PLATES` wired end-to-end (`SetInputRow`/`ExerciseCard`/`DaySessionContent`/
  `SetRow`); `WeightParser` overhaul (bare number on a PLATES exercise = plate count × configurable
  `plateLb`); new `PLATE_WEIGHT_LB` pref.
- `RestTimerController` rearchitected to wall-clock (`endAtMs` instead of decrement; `Clock` injected) —
  fixes background drift; `InlineRestTimer` moved out of `ExerciseCard` into a dedicated `DayContent`
  item (fixes a one-set lag). New `RestTimerControllerTest`.
- Session flow: `DiscardDialog` → `LeaveSessionDialog` (resume-later / discard / keep-going) +
  `CrossDaySessionDialog`; Overview resume banner via `observeActiveSession()`; "DONE / SKIPPED" section;
  skip auto-advances; journal field on session summary (`setJournal` → `SessionDao`).
- `BackupRepository` overhaul: full backup is now a ZIP (`database.db` + `settings.preferences_pb`),
  restore sniffs ZIP vs raw DB and stages `pending_restore_prefs.pb`, `VACUUM INTO` → WAL-checkpoint
  fallback for pre-Android-11; lossy exports renamed (`exportFullDataJson`) with fixed overwrite
  filenames and a `FileProvider` share intent.
- `ProgramRepository` expanded (reconcile stale customizations, `rerollAll`/`rerollDay`, widget refresh);
  dedicated cardio days removed from the generator; equipment enum pruned to entries with library
  coverage; bodyweight-fallback exercises added; YouTube demo intent on exercise cards.
- Trophies reworked (equipment-agnostic `precision_10`/`reps_50`/`pr_50`, `workouts_25` replacing
  dumbbell-specific and `all_4_days`); near-miss + Comeback-Kid (calendar-day) fixes. Onboarding
  collapsed to 6 pages; `StepEmphasis`/`StepPlateWeight` added, `StepCardio` removed; `WelcomeScreen`
  deleted. Weekly-window fixes across Session/Cardio DAO queries. New `StatsAggregationsTest` /
  `WeightFormatterTest`.
- **105 files changed (+2457 / −2714).** DB v15 (no change).

### [0.5] — Intelligent program generation
- The hard-coded routine is replaced by a generator: `ProgramGenerator` + `VolumeModel` (volume
  allocation per muscle from goal/experience) + `SplitTemplates` + `GoalProfiles` + `SessionEstimate`,
  with `ProgramRepository` (new, ~190 lines) orchestrating generation and persistence. The old
  `Swaps.kt` (442 lines) is deleted.
- `ExerciseLibrary` substantially expanded (+439) to back the generator; `Program`/`Types` extended.
- Onboarding rewritten to build the program with the user (`OnboardingSteps.kt`); Settings gains a large
  program-customization surface (`SettingsSubPages`/`SettingsViewModel`, +~460) for split/goal/
  experience/emphasis/priorities and per-day re-roll.
- DB v14→v15 (schema bump for the program-customization path). New tests: `ProgramGeneratorTest`,
  `VolumeModelTest`, `GoalProfilesTest`, `SessionEstimateTest`, `ExerciseLibraryTest`.
- **86 files changed (+5504 / −5862)** across `0.5`–`0.5.1` (the deletions include the six exported
  HTML design mockups and the temporary `CODE_AUDIT.md` / `BUGFIX_PLAN.md` working docs).

### [0.4.6] — Program data foundations
- New `ProgramDay` / `ProgramSlot` entities + `ProgramDao`; first `ExerciseLibrary` (132 lines); DB
  migration to schema **v14**.
- Adaptive launcher icon (`ic_launcher` mipmaps + `themes.xml`), `Type.kt` / `Motion.kt` polish,
  `ListMotion.kt`.
- 31 files changed (+1594 / −38).

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
