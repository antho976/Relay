# Avex on the Wrist — Wear OS Integration Plan

> Session ground rules: planning only — no app code edits in this session. The deliverable
> is this plan, committed as `docs/WEAR_OS_PLAN.md` on `claude/planning-session-a0yqrb`.
> Baseline: 0.8.8.2, Room schema v23, no Wear code or dependencies exist anywhere in the
> repo (verified — every "wear/watch" hit is Health Connect wearable-data reads or UI tiles).

---

## Scope decisions (locked)

- **End-state watch features**: in-session logging ✅ · live heart rate ✅ · glanceables
  (tiles/complications) ✅. **Standalone phone-free sessions: OUT** — declared as a future
  slot, not built.
- **Sync model: companion-first.** The phone's Room DB stays the single source of truth;
  the watch is a live remote display + control surface over the Wearable Data Layer
  (Bluetooth, no network — preserves the app's no-INTERNET-permission ethos on both APKs).
- **Health Connect write-back: IN.** Finished gym sessions written to HC as
  `ExerciseSessionRecord` (+ HR series once the watch supplies it). This is what makes
  sessions appear in Samsung Health / Fitbit / Google Fit automatically.
- **One app covers Google and Samsung.** Every Galaxy Watch since 2021 (Watch4+) runs
  Wear OS; a single Wear OS app is the native app for both. Tizen (pre-2021 Samsung) is
  explicitly out of scope — dead platform, separate codebase, shrinking base.

---

## Governing principles

### Principle 1 — The phone is the brain; the wrist is a limb
Every watch interaction is a command routed through the phone's existing write paths
(`WorkoutRepository`, `DayViewModel` handler logic extracted where needed). The watch never
owns state, never writes to its own database, never makes coach decisions. If Bluetooth
drops, the watch degrades to "reconnecting" — it never invents state. This keeps every v2
coach invariant intact for free: watcher-judged writes, undo, one-active-session.

### Principle 2 — The watch earns its place set-by-set, not screen-by-screen
The wrist surface is for the ~2 seconds around each set: see the target, log it, start
rest, feel the timer end. It is not the app shrunk down. No stats, no history browsing, no
settings beyond essentials. Anything that needs thought happens on the phone. (This is
Decision Zero's little sibling: the watch answers "what now, this instant?" only.)

### Principle 3 — Additive signals, fail-soft everywhere
Same rule as Health Connect integration today: no watch paired ⇒ zero behavior change on
the phone. Watch HR absent ⇒ effort model behaves exactly as today. Tile data stale ⇒ tile
shows last-known with its age, never wrong-confident. All new coach inputs go through
`AdaptThresholds`-style gates.

---

## Target architecture (end state)

```
:app (phone)                                :wear (watch)
┌──────────────────────────┐                ┌──────────────────────────┐
│ DayViewModel / handlers  │                │ Wear Compose UI          │
│        │                 │                │  · SessionScreen         │
│ WorkoutSessionBridge ────┼── observe ──┐  │  · RestTimerScreen       │
│ WorkoutRepository        │             │  │  · Tiles + complications │
│ AdaptationRepository     │             │  │        │                 │
│        │                 │             │  │ Health Services          │
│ WearSyncService ◄────────┼─ commands ──┼──┤  (ExerciseClient: HR)    │
│  (WearableListener)      │             │  │        │                 │
│ WearStatePublisher ──────┼─ DataItems ─┼─►│ WearSessionRepository    │
└──────────────────────────┘             │  └──────────────────────────┘
             │                           │
   :shared (Kotlin module)  ◄────────────┘
   protocol DTOs · timer math · exercise names · serialization
```

### Module layout

- **`:wear`** — new Wear OS app module. `applicationId = com.quietsoftware.avex` (same as
  phone — required for Play's wear-track distribution and Data Layer pairing), namespace
  `com.forge.wear`. Compose for Wear OS (wear-compose material3), Tiles (ProtoLayout),
  complications-data-source, Health Services, play-services-wearable. Reuses the release
  keystore; same signing config pattern as `app/build.gradle.kts`.
- **`:shared`** — new Kotlin/Android library, the first real sibling module (README's
  "single :app module" note gets updated). Contains ONLY what both sides need:
  - Protocol DTOs + kotlinx.serialization codecs (versioned, see below).
  - Rest-timer math (extract the pure core of `domain/timer/RestTimerController` —
    wall-clock `endAtMs` anchoring ports perfectly; phone controller becomes a thin wrapper).
  - Exercise display names keyed by `exerciseId` (the catalogue is code, not DB — a
    generated or referenced subset lives here so the watch renders names offline).
  - No Room, no Hilt wiring, no UI.
- **`:app` additions** — `service/wear/` package: `WearSyncService` (WearableListenerService;
  wakes the phone process on watch commands even if the app was killed — commands still land
  in Room), `WearStatePublisher` (observes `WorkoutSessionBridge` + rest timer + readiness
  and writes DataItems), `WearHrIngest` (receives HR batches → Room).

### Data Layer protocol (`:shared`, versioned)

All payloads carry `protocolVersion`; unknown-version messages are dropped with a "update
the other app" surface, never a crash. DataItems for state (latest-wins, survives
disconnect), Messages for commands (fire-once), ChannelClient unused (no bulk data).

**Phone → watch DataItems:**
- `/session/live` — active session mirror: sessionId, dayKey + day title, current exercise
  (id, name, slot), set index/total, prescribed target (weight text × reps, from the same
  suggestion the phone chips show), logged-set ticks, PR flag, session elapsed.
- `/timer/state` — rest timer: `endAtMs`, duration, running/paused. Watch renders countdown
  locally from wall clock (no streaming ticks — battery + BT sanity; both clocks are
  NTP-synced in practice, drift is cosmetic).
- `/glance/today` — tile payload: readiness scale (from `AdaptationRepository.readinessScale()`),
  next planned day, week volume/sessions summary, computed-at timestamp. Refreshed at the
  existing surface points (app open, session finish, weekly pass) — never a hot path.

**Watch → phone Messages:**
- `/cmd/log-set` — log current set: as-prescribed, or with wrist-adjusted weight/reps deltas.
- `/cmd/timer` — skip / +30s / start.
- `/cmd/undo-set` — undo last set (routes through the existing undo path).
- `/hr/batch` — HR samples (t, bpm) batched every ~5s during an active session only.

### Live heart rate (Phase W3)

Watch side: Health Services `ExerciseClient` with `STRENGTH_TRAINING` exercise type while
the phone session is active (auto start/stop follows `/session/live` presence). This gives
sensor-fused HR + calories and marks the watch "in workout" (ongoing activity chip, ambient
support). Samples batch to the phone via `/hr/batch`.

Phone side: new Room entity `SessionHrSample(sessionId, atMs, bpm)` (+ migration v24,
schema JSON, `MigrationTest` per the locked pattern). Consumers, all gated and additive:
- Session detail: avg/max HR line + sparkline (post-session, not live-hot-path).
- Coach: per-session HR effort becomes a real input where `AdaptationSnapshot` currently
  has nothing — intra-session strain into the fatigue/deload drivers, and a live source for
  Coach v3's cardio-interference term (this plan feeds Coach v3 Phase B's ReadinessV2;
  declare it in the SignalRegistry as `watch_hr` when that lands).
- HC write-back: HR series attached to the written exercise session.

### Health Connect write-back (Phase W0 — independent of the watch entirely)

Extend `HealthConnectManager` with a write permission set for `ExerciseSessionRecord`
(+ `HeartRateRecord` later), following the existing independently-opt-in permission-set
pattern. On `WorkoutRepository.finishSession(...)` — beside the existing
`maybeWriteActiveCalories` — write the session (type strength_training, start/end, title
from dayKey). Cardio sessions write with their real exercise type. Fail-soft like every HC
call. **This alone delivers "my gym sessions show up in Samsung Health" — ship it first.**

### Glanceables (Phase W4)

- **Tiles** (swipe-right surfaces): *Today* tile — readiness scale + next day + one-tap
  open-on-phone (or start rest-timer shortcut during a session); *Week* tile — sessions
  done/planned, volume. Rendered from `/glance/today`, always stamped with data age.
- **Complications**: readiness (short-text/ranged), next session (short-text), and during
  an active session the rest timer (countdown complication).
- **Coach v3 handshake**: when Coach v3 Phase B ships the Today Directive, the Today tile
  consumes it verbatim (the directive IS the tile). Until then the tile degrades to
  next-planned-day + readiness — same degrade-to-principled-never-blank rule as the
  cold-start directive.

### Watch design language

`.claude/DESIGN.md` doctrine translated to the wrist, not reinvented: AMOLED black ground,
the user's single accent at the same 1.0/0.6/0.15 ladder, mono uppercase micro-labels, one
big serif figure per screen (the timer countdown / the target weight), dry imperative copy,
no boxes around passive content. Haptics on the wrist take over the phone's timer-done
vibration when connected (phone stays silent — one buzz, on the body part that feels it).
Load the forge-design skill before any watch UI work; add a Wear addendum section to
DESIGN.md in Phase W1 (round-screen rules, ambient mode = dimmed mono only).

---

## Phases (each independently shippable)

### Phase W0 — "Ecosystem write-back" (no watch required)
- `ExerciseSessionRecord` write on gym + cardio session finish; HC settings page gains the
  opt-in row. Sessions appear in Samsung Health / Google Fit.
- Smallest possible release; also the fastest user-visible "Samsung integration" win.

### Phase W1 — "The timer on your wrist"
- `:shared` + `:wear` modules, Gradle/version-catalog/signing wiring, protocol v1.
- Phone: `WearSyncService`, `WearStatePublisher` tapping `WorkoutSessionBridge`.
- Watch app: session-aware home (idle: "no active session" + today glance-lite; active:
  current exercise + rest timer screen), timer controls (skip/+30s), wrist haptic on timer
  done (suppressing the phone buzz while connected).
- Rest-timer math extracted to `:shared` with the existing unit tests moved/kept green.

### Phase W2 — "Log from the wrist"
- Watch set screen: prescribed target rendered big, one-tap log-as-prescribed, crown/+−
  adjust for weight & reps, auto-advance to rest, logged-set ticks, PR moment (gold flash +
  haptic — reserved PR gold honored).
- Phone: `/cmd/log-set` + `/cmd/undo-set` routed through the exact write paths the phone UI
  uses (extract a `SetLogUseCase` from `DayViewModel` handlers if needed so both surfaces
  share one entry point — the only `:app` refactor this plan requires).
- Works with phone in the bag: `WearableListenerService` wakes the process; the foreground
  session service keeps state warm.

### Phase W3 — "Live heart rate"
- Health Services `ExerciseClient` lifecycle bound to session presence; `/hr/batch` stream.
- Room v24: `SessionHrSample` + migration + tests. Session detail avg/max + sparkline.
- Coach signal (gated): intra-session HR strain → fatigue/deload inputs; register as
  `watch_hr` in Coach v3's SignalRegistry when it exists.
- HC write-back upgraded: HR series + calories attached to the exercise session.

### Phase W4 — "Glanceables"
- Today + Week tiles, readiness / next-session / rest-timer complications.
- `/glance/today` publisher at existing surface points; data-age stamping.
- Coach v3 Today Directive consumed verbatim once available.

### Declared future slots (not built, contract-visible like SignalRegistry)
- `standalone_sessions` — watch-only workouts with sync-back (explicitly deferred).
- `watch_cardio` — outdoor cardio recorded from the wrist (GPS via Health Services).
- `warmup_flow` — guided warm-up protocol on the wrist pre-session.

---

## Critical files

- **New**: `:shared` module (protocol, timer math, exercise names) · `:wear` module (Wear
  Compose UI, tiles, complications, Health Services) · `app/src/main/java/.../service/wear/`
  (`WearSyncService`, `WearStatePublisher`, `WearHrIngest`).
- **Touched in `:app`**: `service/WorkoutSessionBridge.kt` (observed, minor additions) ·
  `domain/timer/RestTimerController.kt` (core extracted to `:shared`) ·
  `data/health/HealthConnectManager.kt` (exercise-session write set) ·
  `data/repo/WorkoutRepository.kt` (finish hook, HR persist) · `data/db/` (Migrations v24,
  new DAO/entity) · `ui/gym/train/Day*Handlers` (extract shared `SetLogUseCase`) ·
  `settings` HC page. The live-session screen itself stays untouched (DESIGN.md frozen).
- **Build**: `settings.gradle.kts`, `gradle/libs.versions.toml` (wear-compose, tiles,
  health-services, play-services-wearable, kotlinx-serialization), new module build files.

## Verification

- `:shared` is pure → full unit coverage: protocol codec round-trips (including
  unknown-version drop), timer math (ports existing tests), exercise-name lookups.
- Room v24 migration test per the locked pattern; schema JSON committed.
- Phone-side command handling tested with fake `MessageEvent`s → assert Room writes equal
  phone-UI writes for the same action (single-write-path invariant).
- End-to-end on paired emulators (Wear emulator pairs to phone AVD): log a full session
  from the wrist, kill the phone app mid-session, verify commands still land; BT-drop test
  (watch shows reconnecting, no ghost state).
- Fail-soft audit per phase: unpair the watch, deny HC writes — phone behavior must be
  byte-identical to today.

## Interplay with Coach v3

This plan is deliberately upstream-compatible with `COACH_V3_PLAN.md`: watch HR feeds
ReadinessV2's interference/strain inputs (Phase B/W3), the Today tile is the Today
Directive's wrist surface (Phase B/W4), and the SignalRegistry gains `watch_hr` as an
ACTIVE slot instead of COMING_SOON once W3 ships. Neither plan blocks the other; W0–W2
have zero coach coupling.

## Next step after approval

Phase W0 as its own small branch/release, then W1. Watch UI work loads the forge-design
skill first and adds the Wear addendum to DESIGN.md before the first screen is built.
