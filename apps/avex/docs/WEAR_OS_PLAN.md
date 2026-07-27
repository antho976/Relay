# Avex on the Wrist — Wear OS + Watch-Ecosystem Integration Plan

> Revision 2 — re-verified against the codebase at 0.8.8.3, Room schema v29 (every claim
> below was checked against actual code; file:line refs are from that pass). Reference
> hardware: **Samsung Galaxy Watch 6 Classic** (Wear OS, rotating bezel, BioActive sensor:
> optical HR + BIA body composition; GPS; ECG/BP exist but stay inside Samsung Health
> Monitor and never reach Health Connect — out of scope). No Wear code or dependencies
> exist anywhere in the repo yet; `:wear`/`:shared` are greenfield.

---

## Scope decisions (locked)

- **End-state watch features**: in-session logging ✅ · live heart rate ✅ · glanceables
  (tiles/complications) ✅ · **ecosystem reads** ✅ (see below). **Standalone phone-free
  sessions: OUT** — declared as a future slot, not built.
- **Two delivery tracks, one integration.** Track 1 is the watch *app* (`:wear`, Data
  Layer). Track 2 is the watch *data* — everything the watch already writes to Health
  Connect via Samsung Health (continuous HR, HRV, sleep, steps, exercise sessions with HR
  series + routes, body composition from the BIA sensor). Track 2 needs **no watch app at
  all** and ships on its own phases (W5/W6). A user with a Galaxy Watch gets real value
  before the watch APK even exists.
- **Sync model: companion-first.** The phone's Room DB stays the single source of truth;
  the watch is a live remote display + control surface over the Wearable Data Layer
  (Bluetooth, no network — preserves the app's no-INTERNET-permission ethos on both APKs).
- **Health Connect write-back: IN.** Finished gym sessions written to HC as
  `ExerciseSessionRecord` (+ HR series once the watch supplies it). This is what makes
  sessions appear in Samsung Health / Fitbit / Google Fit automatically.
- **One app covers Google and Samsung.** Every Galaxy Watch since 2021 (Watch4+) runs
  Wear OS; a single Wear OS app is the native app for both. Tizen is out of scope.
- **Not available, by platform fact (do not chase):** VO2max (Samsung doesn't sync it to
  HC), ECG, blood pressure (Samsung Health Monitor silo), Samsung stress score
  (proprietary). Skin temperature sync is uncertain — verify at W6 build time, treat as a
  declared-future signal until proven.

---

## Governing principles

### Principle 1 — The phone is the brain; the wrist is a limb
Every watch interaction is a command routed through the phone's existing write paths.
The watch never owns state, never writes to its own database, never makes coach decisions.
If Bluetooth drops, the watch degrades to "reconnecting" — it never invents state. The
invariant to protect is the **single set-write path**: set log = `logSet` →
`WorkoutRepository.logSet` → `loggedSetDao.insert`, one entry point for phone UI and watch
alike. (Correction from rev 1: there is no per-set "watcher" judging writes —
`OutcomeWatcher` judges weekly coach decisions, not set writes. The invariant is the
single path itself.)

### Principle 2 — The watch earns its place set-by-set, not screen-by-screen
The wrist surface is for the ~2 seconds around each set: see the target, log it, start
rest, feel the timer end. It is not the app shrunk down. No stats, no history browsing, no
settings beyond essentials. Anything that needs thought happens on the phone.

### Principle 3 — Additive signals, fail-soft everywhere
No watch paired ⇒ zero behavior change on the phone. Watch HR absent ⇒ effort model
behaves exactly as today. Tile data stale ⇒ tile shows last-known with its age, never
wrong-confident. All new coach inputs go through `AdaptThresholds`-style gates.

---

## Target architecture (end state)

```
:app (phone)                                :wear (watch)
┌──────────────────────────┐                ┌──────────────────────────┐
│ DayViewModel / handlers  │                │ Wear Compose UI          │
│        │                 │                │  · SessionScreen         │
│ SetLogUseCase (shared    │                │  · RestTimerScreen       │
│  entry: phone UI + watch)│                │  · Tiles + complications │
│ WatchSessionState ───────┼── observe ──┐  │        │                 │
│ WorkoutRepository        │             │  │ Health Services          │
│ AdaptationRepository     │             │  │  (ExerciseClient: HR)    │
│ WearSyncService ◄────────┼─ commands ──┼──┤        │                 │
│  (WearableListener)      │             │  │ WearSessionRepository    │
│ WearStatePublisher ──────┼─ DataItems ─┼─►│  (mirror of DataItems)   │
└──────────────────────────┘             │  └──────────────────────────┘
             │                           │
   :shared (Kotlin module)  ◄────────────┘
   protocol DTOs · timer math · weight-step table · serialization
```

### The publisher problem (the load-bearing W1 decision)

**Reality check (verified):** `WorkoutSessionBridge` today carries only `dayName`,
`startedAtMs`, and a `timerDone` ping (`service/WorkoutSessionBridge.kt:26-39`).
Everything `/session/live` needs — sessionId, current exercise, set index/total,
prescribed target, logged-set ticks, PR flag, elapsed anchor — lives in `DayUiState`
inside the ViewModel. "Tap the bridge" is not enough.

**Decision:** introduce a `WatchSessionState` published through the bridge, but built
**repository-side, not VM-side** — the same session-context resolution the
`SetLogUseCase` needs (see W2) produces the mirror state. The VM becomes one consumer of
that resolution instead of its owner. This is more work than rev 1 admitted, and it is
scheduled work, not incidental: it's what makes both `/session/live` and
phone-killed command handling possible from one mechanism.

### Module layout

- **`:wear`** — new Wear OS app module. `applicationId = com.quietsoftware.avex` (same as
  phone — required for Play wear-track distribution and Data Layer pairing), namespace
  `com.forge.wear`, **minSdk 30** (Wear OS 3+, Watch4+; phone stays minSdk 26). Compose
  for Wear OS, Tiles (ProtoLayout), complications-data-source, Health Services,
  play-services-wearable. Reuses the release keystore; **needs its own versionCode
  scheme** (same applicationId ⇒ distinct versionCodes per APK; decide the offset
  convention, e.g. wear = phone + 100000, in W1). Play listing needs wear screenshots +
  wear-track opt-in before first release.
- **`:shared`** — new Kotlin/Android library, the first real sibling module (README's
  "single :app module" note is already stale — `:baselineprofile` exists — fix it here).
  Contains ONLY what both sides need:
  - Protocol DTOs + kotlinx.serialization codecs (versioned; serialization is net-new to
    the repo — plugin + catalog entries).
  - Rest-timer math — verified pure and cleanly extractable: wall-clock `endAtMs`
    anchoring, injectable `Clock`, zero Android deps, 13 FakeClock tests move with it
    (`domain/timer/RestTimerController.kt`).
  - **Weight-step table** — the unit→increment mapping (KG 2.5 / LB 5 / ST 0.5 /
    PLATES 0.5×plate) currently hardcoded inline in `SetInputRow.kt:351`; extracted so
    phone ± buttons and watch bezel-adjust share one table.
  - ~~Exercise display names~~ — **dropped from rev 1.** `/session/live` already carries
    the display-name string, DataItems persist across disconnects, and swap-aware naming
    (`Program.exerciseDisplayName`) is stateful phone-side. A mirrored catalogue in
    `:shared` is drift risk with no payoff.
  - No Room, no Hilt wiring, no UI.
- **`:app` additions** — `service/wear/` package: `WearSyncService`
  (WearableListenerService; wakes the phone process on watch commands even if the app was
  killed), `WearStatePublisher` (publishes `WatchSessionState` + rest timer + glance
  DataItems), `WearHrIngest` (HR batches → Room). Plus `domain/session/SetLogUseCase`
  (see W2).

### Data Layer protocol (`:shared`, versioned)

All payloads carry `protocolVersion`; unknown-version messages are dropped with an
"update the other app" surface, never a crash. DataItems for state (latest-wins, survives
disconnect), Messages for commands (fire-once). Path constants live in one additive
registry so later revs (Engine E-C's `/cardio/live`, `/cmd/cardio`) extend without
breaking v1 watches.

**Phone → watch DataItems:**
- `/session/live` — active session mirror: sessionId, dayKey + day title, current exercise
  (id, **display name string** — authoritative, no watch-side lookup), slot, set
  index/total, prescribed target (weight text × reps, from the same suggestion the phone
  chips show), logged-set ticks, PR flag, session-elapsed **anchor timestamp** (watch
  renders elapsed locally — never a ticking stream), weight unit + step + plate weight
  (for bezel adjust).
- `/timer/state` — rest timer: `endAtMs`, duration, running/paused. Watch renders
  countdown locally from wall clock.
- `/config` — **new in rev 2**: the user's accent (`SettingsRepository.accentColorHex` +
  `accentEnabled`) and weight unit. The design doctrine requires the watch to render "the
  user's single accent" — without this item the watch can't obey it.
- `/glance/today` — tile payload: readiness scale (nullable below data gates — degrade,
  never blank), next planned day, week volume/sessions summary, computed-at timestamp.
  **A minimal version ships in W1**, not W4 — the W1 idle home ("today glance-lite")
  needs it; W4 only enriches it.

**Watch → phone Messages:**
- All commands carry a **`commandId` (UUID)**; the phone dedups on it (idempotent
  processing) and acks via a `/cmd/ack` DataItem keyed by commandId. Watch UX is
  pending → confirmed-by-mirror-update, never optimistic: the tick appears when
  `/session/live` reflects the write, and a command with no ack within a grace window
  surfaces "not logged — reconnect" instead of silently dropping. This is protocol v1,
  not a later patch — double-taps and BT flaps must not double-log sets.
- `/cmd/log-set` — log current set: as-prescribed, or with wrist-adjusted weight/reps.
- `/cmd/timer` — skip / +30s / start.
- `/cmd/undo-set` — undo last set.
- `/hr/batch` — HR samples (t, bpm) batched every ~5s during an active session only.

### Set logging from the wrist (Phase W2) — the real shape of the refactor

**Reality check (verified):** the phone's set-log path is a ~125-line ViewModel handler
(`DayExerciseHandlers.kt:215-339`) entangled with UI state: weight-jump confirm dialog
(early-returns into a dialog), rest-timer auto-start pushed into compose state, lazy
`addExerciseToSession`, coach calibration write (`recordSuggestionOutcome`), a 5-second
undo window held as VM state, auto-collapse. PR flags and prescribed targets are computed
in VM builders. Rev 1's "extract a SetLogUseCase if needed — the only :app refactor" was
an understatement. The actual work:

- **`SetLogUseCase` resolves session context from Room + `Program` with no ViewModel** —
  current exercise, set index, suggestion/prescribed target, rest prescription — and owns
  the write-path side effects that ARE the write (insert, lazy add-exercise, calibration
  write, RestEvent open/close, timer start). Phone-only effects (jump dialog, auto-
  collapse) stay in the handler, which becomes a thin wrapper.
- **Undo moves out of VM state.** The 5s `undoableSetId` window is VM-held today
  (`DaySessionHandlers.kt:23-33`); a killed-then-woken process has no VM, so `/cmd/undo-set`
  needs the undo window owned by the use case (or a repository-level "delete last set for
  session" with its own window).
- **PR at write time.** No PR event exists at write today — `wasPr` is derived at
  card-build time, and `computePrFlags` disagrees with `PrDetector.isPr` on a first-ever
  session (returns false vs "empty history = PR"). The use case computes PR via
  `PrDetector` at write time (one rule, reconciled), so the watch gold-flash and the phone
  agree — and so a dead-phone-process log still knows it was a PR.
- **Weight-jump policy on the wrist:** bezel adjustment is bounded (hard cap on delta per
  set relative to last logged/suggested), and a past-threshold value asks for one extra
  bezel-press confirm on the watch. A misspun bezel must not log 500 lb into PR detection
  and coach calibration.
- **Phone killed mid-session works because:** `WearableListenerService` wakes the process,
  `ForgeApp.onCreate` re-seeds `Program` (`ensureLoaded()`), and the use case needs only
  Room + Program. Verified: the foreground `WorkoutSessionService` exists but keeps only
  dayName/startedAt warm — it is not the state source, the DB is. Acceptance test: kill
  the app mid-session, send `/cmd/log-set`, assert the Room write equals the phone-UI
  write byte-for-byte.

### Live heart rate (Phase W3)

Watch side: Health Services `ExerciseClient` with `STRENGTH_TRAINING` while the phone
session is active (auto start/stop follows `/session/live` presence). Handle the two
platform realities rev 1 skipped: **BODY_SENSORS / READ_HEART_RATE is a runtime
permission on the watch** (needs a wrist permission screen + a permanent
works-without-HR degraded state), and **ExerciseClient is exclusive** — if another watch
app owns the exercise, degrade to "HR unavailable" gracefully, never a retry loop.
While active, show **live HR (current bpm) + live calories** on the session screen —
the data is already on the watch; displaying it is free.

Phone side: new Room entity `SessionHrSample(sessionId, atMs, bpm)` — migration to the
**then-current next schema version** (v30+ as of this revision; rev 1's "v24" is stale —
six migrations landed since), schema JSON, pairwise + full-chain `MigrationTest` cases
per the locked pattern. Consumers, all gated and additive:

- **Session detail: the training HR graph.** `SessionDetailCharts.kt` gains an HR line
  over the session timeline with **set markers** (`LoggedSet.completedAt` timestamps every
  set) and **per-exercise bands** — "what was my heart rate while I was doing squats" is
  directly answerable. Avg/max per session and per exercise.
- **Heart-rate recovery (HRR) between sets** — the sleeper feature: `RestEvent` already
  records every rest window (sessionId, exerciseId, setIndex, realized seconds, loggedAt).
  Join HR samples into those windows → HR drop over the first 60s of rest. That's a
  fitness signal most gym apps can't compute (needs set-timestamps + rest-events + HR —
  we have all three). Session-detail stat first; coach input later, gated.
- **Coach:** intra-session HR strain into fatigue/deload drivers; a live source for Coach
  v3's cardio-interference term. `AdaptationSnapshot.HealthSnap` today has only
  restingHr + sleepNights — new fields are additive, `AdaptThresholds`-gated. Declare
  `watch_hr` in the SignalRegistry when Coach v3 **Phase A2** lands it (declared there as
  COMING_SOON — the source ships, the consumer doesn't yet).
- **HC write-back upgraded:** HR series attached to the written session
  (`WRITE_HEART_RATE` joins the session write set), and **real watch calories replace the
  MET estimate** in `maybeWriteActiveCalories` when present.

### Health Connect write-back (Phase W0 — independent of the watch entirely)

Extend `HealthConnectManager` with a write permission set for `ExerciseSessionRecord`
following the verified independently-opt-in pattern (each integration = its own
permission set + launcher + Recovery-page row + pref; calories is the closest precedent:
`hcWriteCalories` → `canWriteActiveCalories()` → `writeActiveCalories`, all fail-soft).

**Corrections from code verification:**
- Strength: hook beside `maybeWriteActiveCalories` in `WorkoutRepository.finishSession`
  (`WorkoutRepository.kt:258`) as planned. The **orphan-recovery finish path**
  (`WorkoutRepository.kt:380-399`) intentionally skips side effects — decision: recovered
  sessions DO get the HC write (they're real sessions), so the write call moves into a
  small shared finish-side-effects helper both paths call.
- **Cardio does NOT go through `finishSession`.** It has its own save path
  (`CardioViewModel.saveEntry` → `CardioRepository.add/update`) with no HC hook. The
  cardio write hooks there, with its real exercise type. Cardio entries are editable and
  deletable after the fact: write with a `clientRecordId` derived from the entry id, so
  edits **update** and deletes **delete** the HC record instead of duplicating.
- **Self-exclusion filter:** `matchSessionRoute` (`HealthConnectManager.kt:393-417`)
  picks the best time-matching HC session for GPS-route offers, and the codebase has zero
  `DataOrigin` filtering — once we write our own (routeless) sessions, they can shadow
  the user's real watch session and suppress its route. All HC session reads gain a
  filter: skip records whose dataOrigin is our own package.
- Manifest gains `android.permission.health.WRITE_EXERCISE`.

**This alone delivers "my gym sessions show up in Samsung Health" — ship it first.**

### Ecosystem reads (Phases W5/W6 — no watch app required)

The Galaxy Watch already writes, via Samsung Health → Health Connect: steps, distance,
floors, active/total calories, exercise sessions (with HR series + GPS routes),
continuous heart rate, **heart-rate variability**, resting HR, SpO2, respiratory rate,
sleep. The app currently reads a fraction (sleep duration, resting HR, weight, body fat,
daily steps into cardio surfaces only, routes). These phases close the gap — they are
phone-only, watch-app-independent, and shippable immediately after W0.

**W5 — "Your watch workouts, understood" (cardio-side reads):**
- **Cardio HR graph.** When a cardio entry time-matches a watch-recorded HC session (the
  `matchSessionRoute` window logic, generalized), read the `HeartRateRecord` series for
  that window → HR line chart on `CardioSessionDetailSheet` beside the existing route
  thumbnail, with avg/max and zone coloring. The manual `hr_zone` tag stays as the
  fallback; measured HR renders above it.
- **Session stat enrichment.** Same match also offers the watch's measured duration,
  distance, and calories into the entry (compare/adopt, never silently overwrite).
- **Watch-workout import suggestions.** Today the app only borrows the GPS route from
  watch sessions. New: recent HC exercise sessions with no matching cardio entry surface
  as "Recorded with your watch — import?" → prefilled `CardioLogSheet` (type mapped from
  the HC exercise type, duration, distance, calories, route). Self-written records
  excluded via the W0 dataOrigin filter; imports are suggestions, never automatic (the
  no-ghost-data rule).
- New "Heart rate (workouts)" read row on the Recovery page (own permission set:
  `READ_HEART_RATE`).

**W6 — "The day between sessions" (daily biometrics + readiness inputs):**
- **Steps to the Overview and the coach.** Hourly steps bars already exist
  (cardio-only surface); steps never reach Overview or `AdaptationSnapshot` — even though
  Coach v3's ReadinessV2 lists steps as an input (cross-plan gap, verified). New: a
  compact daily-movement card on Overview (the old recovery-snapshot slot,
  `OverviewScreen.kt:373-375`) + steps into `HealthSnap`, gated.
- **Overnight HR + HRV as readiness inputs.** Continuous `HeartRateRecord` (overnight
  curve) and `HeartRateVariabilityRmssdRecord` reads → ReadinessV2 inputs and future
  SignalRegistry slots (`hrv`), with the usual data-count gates. HRV is the single
  highest-value recovery signal the watch produces.
- **Sleep stages.** The sleep read is duration-only today (`readRecovery` caps at
  duration); `SleepSessionRecord` carries stages — read them for a sleep-quality line
  (deep/REM share) into readiness, and a richer sleep row in the Coach signals lens.
- **Body composition completion.** The BIA sensor writes more than body fat: add
  `LeanBodyMassRecord` import (skeletal muscle trend — THE body-comp number for a lifting
  app) next to the existing weight/body-fat rows in Profile → BODY, same
  import-if-newer pattern (`BodyFatSync.shouldImport` precedent).
- SpO2 / respiratory rate / skin temp: declared-future signals only — read nothing until
  a consumer exists (no data hoarding).

### Glanceables (Phase W4)

- **Tiles**: *Today* tile — readiness scale + next day + one-tap open-on-phone (or rest-
  timer shortcut during a session); *Week* tile — sessions done/planned, volume. Rendered
  from `/glance/today`, always stamped with data age. **Refresh mechanics specified:** a
  watch-side DataClient listener calls `TileService.getUpdater().requestUpdate()` on
  `/glance/today` change — tiles do not refresh themselves.
- **Complications**: readiness (short-text/ranged), next session (short-text), rest timer
  during active session — built on **`TimeDifferenceComplicationText`** so the countdown
  renders locally without burning the complication update budget.
- **Coach v3 handshake**: when Coach v3 **Phase B2** ships the Today Directive, the Today tile
  consumes it verbatim. Until then: next-planned-day + readiness, degrade-never-blank
  (readinessScale() verified nullable below data gates — the degrade path is real).

### Watch design language

`.claude/DESIGN.md` doctrine translated to the wrist, not reinvented: AMOLED black
ground, the user's single accent (delivered via `/config` — see protocol) at the same
1.0/0.6/0.15 ladder, mono uppercase micro-labels, one big serif figure per screen, dry
imperative copy. **Rotating-bezel-first inputs** on the reference hardware (RSB/rotary
API — same code path serves crown watches; touch ± fallback always present). Haptics on
the wrist take over the phone's timer-done vibration when connected — with a **handoff
ack**: the phone suppresses its buzz only when the watch acks the timer-done haptic
within a grace window; no ack ⇒ the phone buzzes late rather than nobody buzzing. The
suppression hook is the existing conditional gate in `WorkoutSessionService`
(`:116-124` — already checks alert pref + quiet hours; "watch acked" is a third
condition). **`OngoingActivity` from W1** (not W3): during a session the watch face shows
the session indicator for one-tap return — without it, leaving the watch app mid-session
means relaunching from the launcher, which kills the 2-second promise.
**Notification bridging**: the phone's timer/session notifications currently auto-bridge
to the watch; from W1 the watch has native surfaces, so bridged duplicates are excluded
(bridging config / dismissal sync) — one alert, on one body part.
Load the forge-design skill before any watch UI work; add the Wear addendum to DESIGN.md
in Phase W1 before the first screen (round-screen rules, ambient = dimmed mono only,
bezel affordances).

---

## Phases (each independently shippable)

### Phase W0 — "Ecosystem write-back" (no watch required) — SHIP FIRST
- `ExerciseSessionRecord` write on gym finish (both finish paths) + cardio save/edit/
  delete (clientRecordId lifecycle); dataOrigin self-exclusion on all HC session reads;
  Recovery-page opt-in row; `WRITE_EXERCISE` manifest permission.

### Phase W5 — "Your watch workouts, understood" (no watch app; right after W0)
- Cardio HR graph + stat enrichment from matched HC sessions; watch-workout import
  suggestions; `READ_HEART_RATE` permission set + Recovery row.

### Phase W1 — "The timer on your wrist"
- `:shared` + `:wear` modules; Gradle/catalog/signing + **wear versionCode scheme**;
  protocol v1 **including commandId/ack + `/config` + minimal `/glance/today`**.
- Phone: `WearSyncService`, `WearStatePublisher`, repository-side `WatchSessionState`.
- Watch: idle home (glance-lite) + active session screen (current exercise + rest timer),
  skip/+30s, wrist haptic with ack-based phone suppression, `OngoingActivity`,
  notification-bridging config, ambient timer.
- Rest-timer math to `:shared`, tests moved **and CI updated to run them**
  (`:shared:test` is not covered by the existing `testDebugUnitTest` invocation).
- DESIGN.md Wear addendum before the first screen (forge-design loaded first).

### Phase W2 — "Log from the wrist"
- Watch set screen: target big, one-tap log-as-prescribed, bezel/± adjust with shared
  step table, bounded + confirm-past-threshold jump policy, auto-advance to rest,
  logged-set ticks, PR moment (gold flash + haptic — reserved PR gold honored).
- Phone: `SetLogUseCase` (Room+Program context resolution, no VM; owns write-path side
  effects incl. PR-at-write via `PrDetector`, undo window, RestEvent, calibration write);
  handlers become thin wrappers. `/cmd/log-set` + `/cmd/undo-set` route through it.
- Acceptance: fake `MessageEvent`s ⇒ Room writes equal phone-UI writes; **cold-process
  test** (app killed → command → correct write); double-send dedup test (commandId).

### Phase W6 — "The day between sessions" (no watch app; parallel-safe)
- Overview daily-movement card; steps → `HealthSnap`/ReadinessV2; overnight HR + HRV
  reads → readiness (gated); sleep stages; `LeanBodyMassRecord` import in Profile BODY.

### Phase W3 — "Live heart rate"
- Health Services `ExerciseClient` bound to session presence; wrist permission flow +
  degraded no-HR state + exclusive-client conflict handling; live HR + calories on the
  session screen; `/hr/batch`.
- Room next-version migration: `SessionHrSample` + locked-pattern tests.
- Session detail: HR graph with set markers + per-exercise bands + avg/max; **HRR between
  sets** (RestEvent × HR join) as a session stat.
- Coach signal (gated): HR strain → fatigue/deload inputs; `watch_hr` in SignalRegistry
  when it exists. HC write-back: HR series + real calories.

### Phase W4 — "Glanceables"
- Today + Week tiles (DataItem-triggered refresh), readiness / next-session / rest-timer
  complications (TimeDifference text), data-age stamping, Today-Directive handshake.

### Declared future slots (not built, contract-visible)
- `standalone_sessions` — watch-only workouts with sync-back.
- `watch_cardio` — outdoor cardio recorded from the wrist (GPS via Health Services;
  Engine E-C's `/cardio/live` + `/cmd/cardio` protocol rev slots here).
- `warmup_flow` — guided warm-up on the wrist pre-session.
- `hrv`, `skin_temp`, `spo2` — readiness signals declared, read only when consumed.

---

## Critical files

- **New**: `:shared` (protocol + codecs, timer math, weight-step table) · `:wear` (Wear
  Compose UI, tiles, complications, Health Services) · `app/.../service/wear/`
  (`WearSyncService`, `WearStatePublisher`, `WearHrIngest`) ·
  `app/.../domain/session/SetLogUseCase`.
- **Touched in `:app`**: `service/WorkoutSessionBridge.kt` (grows `WatchSessionState`) ·
  `service/WorkoutSessionService.kt` (haptic handoff gate) · `domain/timer/
  RestTimerController.kt` (core → `:shared`) · `data/health/HealthConnectManager.kt`
  (session write set, HR/HRV/sleep-stage/lean-mass reads, dataOrigin filter) ·
  `data/repo/WorkoutRepository.kt` (finish hooks both paths, HR persist) ·
  `data/repo/CardioRepository.kt` + `ui/cardio/` (HC write hook, HR graph, import
  suggestions) · `data/db/` (next-version migration, `SessionHrSample`) ·
  `ui/gym/train/Day*Handlers` (thin wrappers over `SetLogUseCase`) ·
  `ui/gym/session/SessionDetailCharts.kt` (HR graph) · `ui/overview/OverviewScreen.kt`
  (daily-movement card) · `ui/profile/` (lean-mass row) · settings Recovery page (new
  rows). The live-session screen itself stays untouched (DESIGN.md frozen).
- **Build**: `settings.gradle.kts`, `gradle/libs.versions.toml` (wear-compose, tiles,
  health-services, play-services-wearable, kotlinx-serialization — all net-new), new
  module build files, **`.github/workflows/ci.yml`** (add `:shared:test` + `:wear` unit
  tests — verified not covered today), README module note.

## Verification

- `:shared` pure → full unit coverage: codec round-trips (incl. unknown-version drop +
  commandId dedup), timer math (ported tests), weight-step table.
- Room migration test per locked pattern (pairwise + full-chain); schema JSON committed.
- Phone-side command handling with fake `MessageEvent`s → Room writes equal phone-UI
  writes; cold-process (killed app) test; duplicate-command test.
- End-to-end on paired emulators: full session from the wrist, kill phone app
  mid-session, BT-drop (watch shows reconnecting, no ghost state, no double logs).
- Fail-soft audit per phase: unpair watch, deny each HC permission — phone behavior
  byte-identical to today.
- Haptic handoff: timer-done with watch reachable (wrist buzzes, phone silent), watch
  dead (phone buzzes after grace) — no silent case.

## Interplay with Coach v3 / Engine

Upstream-compatible with `COACH_V3_PLAN.md` and `ENGINE_PLAN.md`: watch HR (W3) feeds
ReadinessV2 strain/interference and Engine E-C's live zones; W6 supplies the steps input
ReadinessV2 already lists (currently missing from `AdaptationSnapshot` — this plan
closes that) plus HRV/sleep-stage inputs; the Today tile (W4) renders the Today
Directive; SignalRegistry gains `watch_hr` (+ later `hrv`) as ACTIVE slots. W0/W5/W1/W2
have zero coach coupling. Protocol path registry is additive for Engine's later
`/cardio/live` rev.

## Sequencing

W0 → W5 → W1 → W2 → (W6 any time after W5, parallel-safe) → W3 → W4, interleaved with
Coach/Engine releases per `ROADMAP.md`. W0 and W5 require no watch app and deliver
Galaxy-Watch-visible value immediately; W3 unlocks Engine E-C; W4 lands after Coach v3 B
so the directive exists to render. Watch UI work loads the forge-design skill first and
adds the Wear addendum to DESIGN.md before the first screen.
