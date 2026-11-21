# Area 07 — Phone↔Watch sync protocol, Wear module, Health Connect, Widget

Scope audited (fully read): `shared/src/main/kotlin/com/forge/shared/protocol/*` + its tests,
`app/.../service/wear/*` (10 files) + tests, the whole `wear/` module (15 files, 1702 lines),
`app/.../data/health/HealthConnectManager.kt` (912 lines), `app/.../domain/health/*`,
`app/.../widget/ForgeWidget.kt`, plus the write paths they call into
(`SetLogUseCase`, `WorkoutRepository`, `CardioRepository`, `WeightFormatter`, `WeightParser`)
and both AndroidManifests.

---

## [CRITICAL] The watch logs sets in POUNDS while labelling and stepping them as KG / ST

**File:** `app/src/main/java/com/forge/app/service/wear/WatchSessionMirror.kt:112-115` and `:148-151`;
`wear/src/main/java/com/forge/wear/ui/SessionScreen.kt:59-60`, `:99`, `:110`, `:147`, `:156`, `:179`, `:223`, `:334-339`;
`app/src/main/java/com/forge/app/domain/session/SetLogUseCase.kt:115-122`;
`shared/src/main/kotlin/com/forge/shared/weight/WeightSteps.kt:23-28`

**What:** Weights are **always stored in pounds** (`WeightFormatter.kt:16`, `:149-154` — the phone's
input field holds the *display-unit* value via `weightInputValue(lb, unit)` at
`components/SetInputRow.kt:120`, and `toStoredWeightText()` converts it back to lb on submit at
`DaySessionContent.kt:271`). The wear mirror does **none** of that conversion:

- `WatchSessionMirror.kt:113` sets `targetWeightText = <LoggedSet>.weightText` — the raw **stored lb**
  string (e.g. `"220.5"`). `:150` does the same for the history prefill.
- The same DTO carries `unit = ProtocolWeightUnit.KG` (`:140`) and
  `weightStep = WeightSteps.weightStep(protoUnit, isPlates)` = **2.5**, which `WeightSteps` documents as
  *"Steps are in the DISPLAY unit"* (`WeightSteps.kt:16`).
- `SessionScreen.kt:59-60` seeds `weightValue = session.targetWeightText?.toDoubleOrNull()` → `220.5`,
  renders it as **the** serif figure (`:156`) with the label `unitLabel(session)` = `"KG"` (`:179`, `:334-339`).
- `SessionScreen.kt:110` steps it: `weightValue + detents * weightStep` → +2.5 **on an lb number**.
- On log, `:223` echoes the bare number back; `SetLogUseCase.kt:122` parses it with
  `WeightParser.parse(weightText, effectivePlan.unit, plateLb)`, whose contract is
  *"a bare number is literal pounds"* (`WeightParser.kt:14-22`).

So `SessionLiveDto` mixes a stored-lb value with a display-unit label and a display-unit step. The DTO's
own kdoc (`WearDtos.kt:28`) claims `targetWeightText` is *"preformatted by the phone ("185 lb")"* — it is not.

**Scenario:** A kg user's last squat set was 100 kg, stored as `weightText = "220.5"`.
1. Phone screen shows `100` under a `KG` field. Watch screen shows **`220.5`** under `KG`.
2. User spins the bezel one detent up, intending 102.5 kg. Watch shows `223`, still labelled `KG`.
3. Taps **Log set** → `LogSetCommand(weightText = "223")` → `WeightParser.parse("223", …)` = **223 lb = 101.2 kg**.
4. The set is stored as 101.2 kg, not 102.5 kg. Every bezel detent moves 2.5 **lb** (1.13 kg) while the
   wrist says kg — a 10-set session drifts ~12 kg from what the user believed they logged.
5. Worse for stones: the wrist shows `220.5` labelled `ST` (real value 15.75 st) and steps by 0.5 **lb**.
6. These wrong weights feed `flagPrForLoggedExercise` (`SetLogUseCase.kt:165`) and the all-time frontier,
   and the jump-confirm bound at `:126-134` compares them against lb frontier maxima — so the
   "big jump" guard silently never fires for kg users' real jumps.

Plate exercises are unaffected (`weightText` genuinely holds a plate count there), and lb users are
unaffected — the bug is exactly the KG and ST user populations.

**Fix:** Convert at the protocol boundary, in both directions.
In `WatchSessionMirror.buildDto`, send `targetWeightText = weightInputValue(WeightParser.parse(stored, …), unit)`
(display units, matching `weightStep`); in `SetLogUseCase.logFromWatch`, run the echoed
`cmd.weightText` through `toStoredWeightText(cmd.weightText, settingsRepo.weightUnit.first())` before
`WeightParser.parse`. Add a round-trip test at 100 kg / 15 st that asserts the wrist figure equals the
phone figure and that a one-detent bump lands on the next display-unit step.

---

## [CRITICAL] The 4-second "Not logged" timeout invites a re-tap that DUPLICATES the set — the idempotency key is regenerated per tap

**File:** `wear/src/main/java/com/forge/wear/ui/SessionScreen.kt:89-94` and `:215-229`;
`wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:130-144`, `:195`;
`app/src/main/java/com/forge/app/service/wear/CommandDeduper.kt:12-23`

**What:** `SessionScreen.kt:90-94` is:

```kotlin
LaunchedEffect(pendingId) {
    if (pendingId == null) return@LaunchedEffect
    delay(4_000)
    if (pendingId != null) { statusLine = "Not logged · reconnecting"; pendingId = null }
}
```

The comment on `:89` says *"the command may still land; dedup makes retry safe"*. It does not.
Every tap of the capsule (`:220`) calls `repo.sendLogSet(...)`, which mints a **brand-new**
`commandId = UUID.randomUUID()` (`WearDataRepository.kt:141`, `:195`). `CommandDeduper` keys strictly on
`commandId` (`CommandDeduper.kt:20-21`), so a re-tap is a *different* command and is not deduped.
There is no path anywhere in the wear module that re-sends a command with a stable id — no retry
mechanism exists at all. The deduper only defends against the Data Layer delivering the *same* Message
twice; it cannot defend against the user-visible retry the UI actively prompts for.

The 4 s window is easily exceeded: the Data Layer round trip over BT is 0.5–3 s each way, and the ack
comes back as a **DataItem** (`WearStatePublisher.kt:126-128`), which is the slower, batched channel.

**Scenario:**
1. Watch: bench 185×8, tap **Log set**. `LogSetCommand(commandId = "A")` goes out.
2. The message lands on the phone at t=1.2 s; `SetLogUseCase.logFromWatch` writes the row and
   `publishAck` puts `/cmd/ack`. BT is momentarily busy syncing the tile; the ack DataItem arrives at t=5.1 s.
3. At t=4.0 s the wrist prints **"Not logged · reconnecting"** and clears `pendingId`.
4. User taps **Log set** again. `LogSetCommand(commandId = "B")` — a different id.
5. Phone: `deduper.isNew("B")` = true → `logFromWatch` writes a **second identical 185×8 row**.
6. Both sets count toward session volume, the exercise's set count, and the PR frontier. The mirror then
   shows "SET 3 OF 3" when the user has done two.

Duplicates are the failure mode the user is *least* likely to notice mid-workout and the hardest to
unwind afterward.

**Fix:** Make the retry idempotent, not the timeout longer. Hold the `commandId` in `SetView` state and
reuse it for the retry of the same logical set (a new id only when the weight/reps/slot change). Persist
`CommandDeduper`'s window (a small Room table or DataStore keyed by `commandId` + `atMs`, pruned past
~10 min) so it survives the process death a `WearableListenerService` wake makes routine. Also make the
timeout line read "still sending…" rather than "Not logged", which currently states something false.

---

## [CRITICAL] A wrist command sent while the phone is unreachable is dropped forever — no queue, no retry, no persistence

**File:** `wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:184-193`

**What:**

```kotlin
private fun sendBytes(path: String, bytes: ByteArray) {
    scope.launch {
        runCatching {
            val nodes = Wearable.getNodeClient(appContext).connectedNodes.await()
            nodes.forEach { node ->
                Wearable.getMessageClient(appContext).sendMessage(node.id, path, bytes).await()
            }
        }
    }
}
```

Three separate defects in nine lines:

1. **No persistence, no retry.** `MessageClient.sendMessage` is fire-and-forget and fails outright when
   the node is not currently connected. The failure is swallowed by `runCatching` and the payload is
   discarded. Nothing is queued.
2. **`connectedNodes` empty → silent success.** If the phone is out of BT range, `nodes` is empty, the
   `forEach` body never runs, `runCatching` completes normally, and the caller gets no signal.
3. **First-node failure aborts the loop.** `runCatching` wraps the *whole* `forEach`. With two connected
   nodes (a phone plus, say, a tablet or a second paired node), a throw on the first `await()` skips every
   remaining node — including the one running Avex.

This directly contradicts `WearProtocol.kt:8` (*"Messages carry COMMANDS (fire-once, commandId-deduped,
acked)"*) — there is no mechanism that makes "fire-once" survive a disconnect.

**Scenario:** The plan's own "phone in the bag" promise. User leaves the phone in a locker on the far side
of the gym; BT drops.
1. Watch logs 3 sets of squats. Each `sendLogSet` finds `connectedNodes` empty (or throws) → **all three
   commands are discarded**, with no queue and no disk trace.
2. `SetView` shows "Not logged · reconnecting" per set (finding above), so the user re-taps — those
   re-taps are discarded too.
3. The user walks back into range. Nothing re-sends. **Three sets are permanently lost.**
4. If the watch app process is killed in the meantime, even the in-memory intent is gone.

The same hole drops HR batches (`WearHrService.kt:120-123` clears `pending` *before* the send result is
known, so the samples are gone whether or not the send succeeded) and the haptic ack.

**Fix:** Give the wear app a small persisted outbox (Room or a DataStore-backed list) written *before*
the send: `{commandId, path, bytes, createdAtMs, attempts}`. Drain it on `CapabilityClient`/node-connected
callbacks and on app start; remove an entry only when its `/cmd/ack` arrives (the entries are already
idempotent once the `commandId` is stable — see the previous finding). Move `runCatching` inside the
`forEach` so one node's failure can't skip the others, and treat an empty `connectedNodes` as a failure
to enqueue-and-report, not as a success.

---

## [HIGH] The persisted `/cmd/ack` DataItem is replayed on every watch app start, re-arming undo + RPE against a stale set id

**File:** `wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:74-84` and `:106-113`;
`wear/src/main/java/com/forge/wear/ui/SessionScreen.kt:234-253`;
`wear/src/main/java/com/forge/wear/ui/TimerView.kt:81-100`;
`app/src/main/java/com/forge/app/domain/session/SetLogUseCase.kt:170-178`

**What:** `start()` seeds from *every* existing DataItem:

```kotlin
val buffer = dataClient.dataItems.await()
for (item in buffer) applyItem(item.freeze(), deleted = false)
```

`/cmd/ack` is never deleted — `WearStatePublisher.deleteItem` is only ever called for
`PATH_SESSION_LIVE` (`:53`) and `PATH_TIMER_STATE` (`:80`). So the *last ack ever published*, which may be
hours or days old, persists on the node and is re-applied on every cold start. `applyItem` then stamps it
with a **fresh local clock**:

```kotlin
if (ack.ok && ack.setId != null) {
    _lastLog.value = LastLog(ack.setId!!, System.currentTimeMillis())   // ← local now, not ack.atMs
}
```

`SetView`/`TimerView` gate the undo + rate row on `nowMs - log.atLocalMs < LAST_LOG_WINDOW_MS` (12 s), so
the freshness window is measured from *app launch*, not from when the set was logged. The row appears
immediately and looks legitimate.

**Scenario:** Wear OS kills background app processes aggressively.
1. 10:02 — user logs bench set 2 from the wrist. Phone acks `{ok, setId = 8213}`. That DataItem now sits on
   the node indefinitely.
2. 10:05 — the watch app process is reclaimed while the user is on the phone between exercises.
3. 10:09 — user raises the wrist mid-session; `MainActivity` starts, `WearDataRepository.start()` seeds the
   old ack, `_lastLog = LastLog(8213, now)`.
4. `SetView` renders (a session is live) and immediately offers **"undo   rate →"**.
5. User taps **rate →**, dials RPE 9, Save. `SetRpeCommand(setId = 8213, rpe = 9.0)` goes out.
6. `rpeFromWatch` only checks `now - set.completedAt > RPE_WINDOW_MS` (**10 minutes**,
   `SetLogUseCase.kt:208`). At 7 minutes the check passes → **RPE 9 is written onto bench set 2**, two
   exercises back, while the user believes they rated the set they just did.

The **undo** half is worse-targeted still: `sendUndoSet` passes only `session.sessionId`, and
`undoLastFromWatch` deletes *the session's most recent set within 15 s* — which after a phone-logged set is
**the phone's set**, not the one named in the row.

**Fix:** Three changes. (a) Have `WearStatePublisher` delete `/cmd/ack` once consumed, or stamp it with the
`sessionId` and drop acks whose session doesn't match the live one. (b) In `applyItem`, use the ack's own
`atMs` (already on the DTO, `WearDtos.kt:173`) instead of `System.currentTimeMillis()` — a genuinely old ack
then fails the 12 s window on its own. (c) Make undo target a set id like RPE does (add `setId` to
`UndoSetCommand`) so it can never delete a set the user didn't see.

---

## [HIGH] Health Connect reads have no pagination — the default 1000-record ascending first page silently truncates recent days to zero

**File:** `app/src/main/java/com/forge/app/data/health/HealthConnectManager.kt:434-451` (`readDailyStepTotals`),
`:395-425` (`readRecovery` — sleep, resting HR, HRV), `:471-478` (`readStepsDay`), `:637-644` (`readHrSeries`),
`:665-674` and `:747-756` (session matching);
caller `app/src/main/java/com/forge/app/ui/overview/OverviewViewModel.kt:125-137`

**What:** Only `readWeightHistory` (`:245-272`) paginates. Every other read is a bare
`client.readRecords(ReadRecordsRequest(Type::class, timeRangeFilter = range))` with **no `pageSize` and no
`pageToken` loop**. `ReadRecordsRequest` defaults to `pageSize = 1000` **and `ascendingOrder = true`, so the
single page returned is the OLDEST 1000 records in the window** and everything newer is dropped without any
error, exception, or log.

`readDailyStepTotals` is the sharp end: `OverviewViewModel.kt:131` requests a **15-day** window, and
Samsung Health / Google Fit / Fitbit write `StepsRecord` in short sub-hour buckets (commonly 50–150 rows per
active day). 15 days × 100 rows/day = 1500 rows > 1000.

**Scenario:** A Galaxy Watch user with two weeks of synced step history opens the Overview screen.
1. `readDailyStepTotals(today-14d, now)` returns the oldest 1000 `StepsRecord` rows — days 1 through ~10.
2. Days 11–15, **including today**, are not in the page at all.
3. `OverviewViewModel.kt:133`: `todaySteps = days.lastOrNull { it.dayStartMs == todayStartMs }?.steps ?: 0`
   → the Home movement line reads **"0 steps"** after a 14,000-step day.
4. `:134-135` computes `typical` from the truncated older days, so the compare is wrong too.
5. The same truncated `dailySteps` list is what `readRecovery` (`:425-427`) hands the coach, so the
   readiness/deload drivers see a phantom sedentary user and can recommend a load increase off it.

The same shape applies to the HRV read at `:417-422` (an 18-day window at
`AdaptationRepository.kt:82`; watches that write per-interval RMSSD rows exceed 1000 easily) and to
`readHrSeries`, whose `HR_SERIES_MAX_SAMPLES` cap is applied *after* the page truncation, so it never binds.

**Fix:** Extract the `do { … } while (token != null && resp.records.isNotEmpty() && out.size < cap)` loop
already written in `readWeightHistory:252-269` into a private `readAllPages(request, cap)` helper and route
every read through it, each with its own sane cap. Where only the newest rows matter, pass
`ascendingOrder = false` with an explicit `pageSize` so a truncation drops the *oldest* data rather than
today's.

---

## [HIGH] Watch-workout distance and calories are aggregated with no `dataOriginFilter` — two apps writing the same run double-count it

**File:** `app/src/main/java/com/forge/app/data/health/HealthConnectManager.kt:697-722`

**What:** `ExerciseSessionRecord.toWatchWorkout` enriches a matched watch session with:

```kotlin
client.aggregate(
    AggregateRequest(
        metrics = metrics,                                   // DISTANCE_TOTAL, ENERGY_TOTAL
        timeRangeFilter = TimeRangeFilter.between(startTime, endTime)
    )
)
```

`AggregateRequest` accepts a `dataOriginFilter` and it is not supplied, so the aggregate sums **every**
`DistanceRecord` and `TotalCaloriesBurnedRecord` from **every app** in that time window — not the ones
belonging to the matched session's origin (which is right there in `metadata.dataOrigin`, already used for
`isSelfWritten` at `:614-615`). Health Connect is explicitly a multi-writer store; overlapping writers for
the same activity are the norm, not the exception.

**Scenario:**
1. User runs 5 km with a Galaxy Watch. Samsung Health writes an `ExerciseSessionRecord` plus a
   `DistanceRecord` of 5.0 km. Google Fit, also connected to Health Connect and also tracking the phone in
   the user's pocket, writes its own `DistanceRecord` of 4.9 km over the same span.
2. `matchWatchSession` correctly picks the Samsung session, then aggregates the window:
   `DISTANCE_TOTAL = 9.9 km`, and `ENERGY_TOTAL` likewise doubled.
3. The cardio screen shows **"watch measured 9.9 km"** for a 5 km run.
4. If the user accepts the import suggestion, that 9.9 km is written into a local `CardioEntry` — and
   `CardioRepository.mirrorToHealthConnect` writes it **back** to Health Connect as a third record.

**Fix:** Pass `dataOriginFilter = setOf(metadata.dataOrigin)` in the `AggregateRequest`, so the enrichment
only ever sums the data the matched session's own author wrote.

---

## [HIGH] `writeActiveCalories` has no `clientRecordId` — a re-finish duplicates the calorie record while both sibling mirrors correctly upsert

**File:** `app/src/main/java/com/forge/app/data/health/HealthConnectManager.kt:585-611`;
callers `app/src/main/java/com/forge/app/data/repo/WorkoutRepository.kt:288-292`, `:300-312`, `:259-280`, `:490-494`

**What:** All three finish mirrors run through one helper (`WorkoutRepository.kt:288-292`):

```kotlin
maybeWriteActiveCalories(session, finishedAtMs = endMs, activeSeconds = activeSeconds)
maybeWriteSessionRecord(session, endMs = endMs)   // clientRecordId = "avex-session-${session.id}"
maybeWriteHrSeries(session, endMs = endMs)        // clientRecordId = "avex-session-hr-${session.id}"
```

Two of the three pass a stable `clientRecordId` + `clientRecordVersion`, which makes the HC write an
**upsert** — the `maybeWriteSessionRecord` kdoc at `:339-342` calls this out explicitly
(*"so a re-finish (orphan recovery after a crash) UPDATES the HC record instead of duplicating it"*).
`writeActiveCalories` (`HealthConnectManager.kt:594-610`) passes **`Metadata.manualEntry()` with no
`clientRecordId` at all**, so every invocation is a plain **insert**.

The same omission applies to `writeWeight` (`:271-288`) and `writeBodyFat` (`:349-368`).

**Scenario:**
1. User finishes a session. `writeFinishMirrors` writes an `ExerciseSessionRecord` (keyed), an HR series
   (keyed), and an `ActiveCaloriesBurnedRecord` of 340 kcal (**unkeyed**).
2. Any second pass over the same session — a retried finish coroutine, a double-tap on Finish before the
   first suspend completes, or `resolveOrphanSession` reaching `writeFinishMirrors` at `:494` for a session
   whose first mirror pass had already run — re-enters the helper.
3. The session record and HR series are *updated* in place. The calorie record is **inserted a second time**.
4. Samsung Health / Google Fit now show **680 kcal** of active energy for one workout. There is no local
   record of the HC row, so nothing can ever detect or repair it, and `deleteExerciseSession` (`:801-813`)
   only knows how to delete `ExerciseSessionRecord`s.

**Fix:** Give `writeActiveCalories` (and `writeWeight` / `writeBodyFat`) a `clientRecordId` +
`clientRecordVersion` exactly like its siblings — `"avex-session-kcal-${session.id}"` with
`clientRecordVersion = endMs` — so a re-finish updates rather than accumulates.

---

## [HIGH] The rest screen's undo is un-debounced and targets "the session's last set", not the set it names

**File:** `wear/src/main/java/com/forge/wear/ui/TimerView.kt:84-100`;
`wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:146-149`;
`app/src/main/java/com/forge/app/domain/session/SetLogUseCase.kt:181-191`

**What:** In `SetView` the undo tap is gated behind `pendingId` (`SessionScreen.kt:234`, `:241`), so a second
tap can't fire while one is in flight. `TimerView` has **no such gate**:

```kotlin
Text("undo", …, modifier = Modifier
    .clickable { session?.let { repo.sendUndoSet(it.sessionId) } }   // no pending gate
    .padding(6.dp))
```

`sendUndoSet` sets `_lastLog.value = null` synchronously, but that only removes the row on the *next*
recomposition — two taps inside one frame both fire. Each mints a fresh `commandId`
(`WearDataRepository.kt:148`), so `CommandDeduper` treats them as two distinct commands.

Compounding it, `UndoSetCommand` carries only a `sessionId` (`WearDtos.kt:128-132`), and
`undoLastFromWatch` resolves the victim itself:

```kotlin
val last = loggedSetDao.allForSession(session.id).maxByOrNull { it.completedAt }
if (clock.nowMs() - last.completedAt > UNDO_WINDOW_MS) return Result(false, …)   // 15 s
workoutRepo.deleteSet(last)
```

It deletes *the session's most recent set*, whatever that is — with no reference to the set the wrist's row
was offering to undo.

**Scenario A (double delete):** The rest screen is up after a set. The user taps "undo"; the watch is
mid-frame on a slow tick and registers a second tap 40 ms later. Two `UndoSetCommand`s with different ids
reach the phone within the 15 s window. Both pass `deduper.isNew`, both resolve `maxByOrNull { completedAt }`
— the first deletes set 3, the second deletes **set 2**. The user loses a set they never touched.

**Scenario B (cross-device):** User logs a set from the wrist (setId 8213) and the rest timer comes up.
Six seconds later they pick up the phone and log a different exercise's set from the day screen (setId 8214).
The wrist's rest screen still shows "undo" for 8213. They tap it. `undoLastFromWatch` deletes
`max(completedAt)` = **8214, the phone's set**. The wrist's own set survives; the phone's vanishes.

**Fix:** Add `setId: Long?` to `UndoSetCommand` and have `undoLastFromWatch` delete *that* row (falling back
to the current behaviour only when it's null, for old watch builds); it already has `RPE_WINDOW`-style
id-targeted precedent in `rpeFromWatch`. Gate the `TimerView` undo behind the same pending state `SetView`
uses so a double-tap cannot issue two commands.

---

## [MEDIUM] Unknown enum values fail the whole payload instead of falling back to the field default — `coerceInputValues` is off

**File:** `shared/src/main/kotlin/com/forge/shared/protocol/WearCodec.kt:15-18`, `:35-44`;
`shared/src/main/kotlin/com/forge/shared/weight/WeightSteps.kt:9-10`;
`shared/src/main/kotlin/com/forge/shared/protocol/WearDtos.kt:39`, `:122-124`

**What:** The codec's `Json` config is:

```kotlin
val json: Json = Json {
    ignoreUnknownKeys = true
    encodeDefaults = true
}
```

`ignoreUnknownKeys` covers added *fields*, which is what `WearProtocol.kt:13-15` promises
(*"Additive fields don't bump [the version]"*). It does **not** cover added *enum constants*.
`coerceInputValues = true` — the setting that makes an unparseable enum value fall back to the property's
declared default — is not set. Two enums ride the wire: `ProtocolWeightUnit` (`SessionLiveDto.unit`, which
*has* a default of `LB`) and `TimerCommand.Action`.

`WearCodecTest.kt` covers added fields (`:65-70`), a newer version stamp (`:58-62`) and corrupt bytes
(`:73-77`) — but has **no test for an unknown enum value**, so nothing catches this.

**Scenario:** A future release adds `ProtocolWeightUnit.G` for a grams display option. Per the documented
rule this is "additive" and `VERSION` stays at 1, so the phone never bumps it and the watch never shows
its update screen.
1. Phone publishes `/session/live` with `"unit":"G"`, `"v":1`.
2. Watch: `probeVersion` reads 1, `1 > 1` is false, so the version gate passes.
3. `json.decodeFromString<SessionLiveDto>` throws on the unknown constant → `DecodeResult.Invalid`.
4. `decodeInto` maps `Invalid` to `Unit` (`WearDataRepository.kt:121`) — a **silent drop**.
5. `_session` keeps its previous value or stays null. The wrist shows the idle glance **mid-workout**,
   with no session, no log button, and no explanation. Every subsequent republish is dropped the same way.

The `unit` field has a perfectly good `LB` default that would have degraded gracefully — one Json flag away.

**Fix:** Set `coerceInputValues = true` in `WearCodec.json`; every enum field that could grow already
carries a default. Add a `WearCodecTest` case decoding `{"v":1,…,"unit":"G",…}` and asserting
`DecodeResult.Ok` with `unit == LB`. Where a *new* enum value must not be silently coerced (a new
`TimerCommand.Action`), bump `VERSION` — and note in `WearProtocol.kt:13-15` that new enum constants and
new fields without defaults are both breaking, which the current wording does not say.

---

## [MEDIUM] Version handling is one-directional — the phone has no "newer watch" surface and drops such commands without an ack

**File:** `app/src/main/java/com/forge/app/service/wear/WearSyncService.kt:44-49`;
`app/src/main/java/com/forge/app/service/wear/WearCommandHandler.kt:25-28`, `:44-47`, `:62-65`;
`wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:68-70`, `:120`

**What:** The watch handles a newer phone: `decodeInto` latches `_newerVersion = true` and `WearRoot.kt:78`
renders the honest `UpdateScreen`. The phone has no equivalent. Every command handler collapses both
failure modes into a bare return:

```kotlin
val cmd = when (val d = WearCodec.decode<LogSetCommand>(bytes)) {
    is WearCodec.DecodeResult.Ok -> d.value
    else -> return   // NewerVersion/Invalid: dropped
}
```

Two consequences: nothing on the phone ever tells the user their watch app is ahead, and — because the
`return` happens *before* `publisher.publishAck` — **no ack is ever published**, so the wrist cannot
distinguish "your phone app is too old" from "bluetooth dropped".

**Scenario:** Wear apps update on their own Play schedule and routinely land ahead of the phone build.
1. Watch updates to protocol v2; phone is still v1.
2. User taps **Log set**. The v2 `LogSetCommand` reaches the phone.
3. `WearCodec.decode` returns `NewerVersion`; the handler returns; no ack, no write.
4. `SetView` sits at "LOGGING…" for 4 s, then prints **"Not logged · reconnecting"**.
5. The user re-taps — which is exactly the duplicate-generating loop of the CRITICAL finding above, except
   here every attempt is guaranteed to fail forever. The user has no idea why and no path to a fix.

**Fix:** In `WearCommandHandler`, publish a refusal ack on the `NewerVersion` branch —
`CmdAckDto(commandId = <probed from the raw JSON>, ok = false, reason = "update Avex on your phone")` — so
the wrist can surface a real cause instead of "reconnecting". `WearCodec.probeVersion` already parses the
stamp from an otherwise-unreadable payload; add a matching `probeCommandId` and the ack becomes possible
even when the body can't be decoded.

---

## [MEDIUM] `/cmd/ack` is one latest-wins DataItem — back-to-back acks coalesce and the earlier command's pending state never resolves

**File:** `app/src/main/java/com/forge/app/service/wear/WearStatePublisher.kt:126-128`, `:137-145`;
`wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:106-113`;
`wear/src/main/java/com/forge/wear/ui/SessionScreen.kt:79-88`

**What:** Every ack for every command type is written to the single path `PATH_CMD_ACK`:

```kotlin
suspend fun publishAck(ack: CmdAckDto) { putItem(WearProtocol.PATH_CMD_ACK, WearCodec.encode(ack)) }
```

The Data Layer is a key–value store with latest-wins semantics per path, and it does **not** guarantee that
every intermediate value is delivered — a put that replaces an unsynced value simply supersedes it. The
watch matches acks by `commandId` (`SessionScreen.kt:81`), so a superseded ack leaves that command's
`pendingId` unresolved forever.

**Scenario:**
1. User taps **Log set** (command A). Phone writes the row and puts ack A.
2. Within the same second, BT is busy; ack A hasn't synced.
3. User taps **rate →** and saves an RPE (command B). Phone puts ack B **to the same path**, superseding A.
4. The watch only ever sees ack B. Command A's `pendingId` never resolves → the 4 s timeout →
   "Not logged · reconnecting" for a set that **was** logged → re-tap → duplicate (CRITICAL finding above).

The same coalescing means a `needsConfirm` ack (the big-jump confirm flow, `WearDtos.kt:169`) can be lost,
leaving the wrist with no way to complete a legitimately heavy set.

**Fix:** Key acks per command — put them at `"/cmd/ack/$commandId"` and have the watch delete the item once
consumed — or accumulate the last N acks in one payload (`CmdAckBatchDto`) so a superseding write can't drop
an unconsumed one. This also resolves the stale-ack replay in the HIGH finding above.

---

## [MEDIUM] The widget never refreshes on session start or finish — "WORKOUT IN PROGRESS" persists for an hour or more

**File:** `app/src/main/java/com/forge/app/widget/ForgeWidget.kt:77-99`, `:165-185`, `:222-231`;
`app/src/main/java/com/forge/app/data/repo/ProgramRepository.kt:307`;
`app/src/main/res/xml/forge_widget_info.xml:5`

**What:** `ForgeWidget().updateAll(context)` is called from exactly one place — `ProgramRepository.kt:307`,
behind `refreshWidget = true`, reached only from the three program-mutation paths at `:94`, `:156`, `:244`.
No session start, session finish, set log, or cardio entry triggers a widget update. The only other refresh
is the manifest's `android:updatePeriodMillis="3600000"` (1 h), which Android clamps to a 30-minute floor and
**defers indefinitely under Doze**.

Everything the widget renders is session-derived: the active-session branch (`:77-80`, `:165-185`), the
streak (`:139`, `:224-229`), and the this-week dot row (`:136-143`, `:230`).

**Scenario:**
1. 07:00 — user starts a Push day. The widget still shows "PUSH A · 5 exercises" (the pre-session state);
   there is no "in progress" indication until the system's own periodic update lands.
2. 08:10 — user finishes the workout. The widget flips to **"WORKOUT IN PROGRESS — Tap to resume"** whenever
   the periodic update happens to fire, then keeps showing it for up to another hour after the session is
   already in history.
3. The streak counter and the week dots stay a workout behind all day.
4. Tapping "Tap to resume" launches MainActivity, which has no session to resume — and the deep-link extra
   doesn't arrive anyway (next finding).

**Fix:** Call `ForgeWidget().updateAll(context)` from `WorkoutRepository` after `startSession`,
`finishSession`, `discardSession` and `resolveOrphanSession` (wrapped in `runCatching` like the existing
call site). Those are the four moments that change what the widget claims.

---

## [MEDIUM] The widget's deep-link extras are passed as Glance `activityOptions`, not intent extras — `EXTRA_START_DAY_KEY` never reaches MainActivity

**File:** `app/src/main/java/com/forge/app/widget/ForgeWidget.kt:63`, `:103-119`, `:152-155`;
`app/src/main/java/com/forge/app/MainActivity.kt:121`, `:241`

**What:** The widget builds an extras `Bundle` and passes it as the third positional argument:

```kotlin
.clickable(actionStartActivity(MainActivity::class.java, actionParametersOf(), extrasBundle))
```

with the comment at `:107-109` asserting *"Glance actionStartActivity(Class, ActionParameters, Bundle)
passes the Bundle as activity extras"*. In Glance (1.1.1, `app/build.gradle.kts:209`) that third parameter is
**`activityOptions: Bundle?`** — the `ActivityOptions.toBundle()` slot handed to
`PendingIntent.getActivity(…, options)`. It is not intent extras. Intent extras come from the
`ActionParameters` argument, which is passed empty (`actionParametersOf()`). The `@OptIn(ExperimentalGlanceApi::class)`
at `:63` is itself the tell — that annotation is required precisely by the `activityOptions` overload.

`MainActivity` genuinely consumes the extra, at `:241` on cold start and `:121` in `onNewIntent`, so this is
a live feature, not dead code:

```kotlin
pendingWidgetDayKey = intent?.getStringExtra(com.forge.app.widget.EXTRA_START_DAY_KEY)
```

**Scenario:** User taps the widget showing "PULL B".
1. Glance fires the PendingIntent with `extrasBundle` as *activity options*. The intent carries **no extras**.
2. `MainActivity` reads `getStringExtra(EXTRA_START_DAY_KEY)` → **null**. `pendingWidgetDayKey` stays null.
3. The nav host lands on Overview instead of the Pull B day screen — the widget's whole tap purpose.
4. `EXTRA_RESUME_SESSION` is likewise never delivered, so the "Tap to resume" state can't resume anything.
5. Secondary risk: an arbitrary (and at `:118` an *immutable* `Bundle.EMPTY`) bundle is handed to the
   framework as `ActivityOptions` — benign on current releases, but not what the API expects.

**Fix:** Put the day key in the `ActionParameters`, which Glance does write into the intent:
`actionParametersOf(dayKeyParam to nextDayKey)` with
`val dayKeyParam = ActionParameters.Key<String>(EXTRA_START_DAY_KEY)`, and drop the third argument (and the
`@OptIn`) entirely. Then verify end-to-end that `MainActivity:241` reads a non-null key.

---

## [MEDIUM] Watch HR samples are dropped on every BT flap and timestamped against a different clock than the filter that admits them

**File:** `wear/src/main/java/com/forge/wear/service/WearHrService.kt:79-89`, `:116-125`;
`app/src/main/java/com/forge/app/service/wear/WearHrIngest.kt:33-38`

**What:** Two separate clock/reliability problems in the HR path.

*Drop on flap* — `batchLoop` clears the pending buffer **before** knowing whether the send worked:

```kotlin
val batch = synchronized(pending) {
    if (pending.isEmpty()) emptyList() else pending.toList().also { pending.clear() }
}
if (batch.isNotEmpty()) repo.sendHrBatch(sessionId, batch, totalKcal)
```

`sendHrBatch` → `sendBytes`, which swallows every failure (see the CRITICAL delivery finding). The samples
are gone. The entity's `(session_id, at_ms)` primary key with `OnConflictStrategy.IGNORE`
(`SessionHrSampleDao.kt:12-14`) makes a *re-send* idempotent — but nothing ever re-sends.

*Clock mismatch* — the watch stamps samples with its own wall clock, reconstructed at
`WearHrService.kt:80`:

```kotlin
val bootInstant = Instant.ofEpochMilli(System.currentTimeMillis() - SystemClock.elapsedRealtime())
val atMs = point.getTimeInstant(bootInstant).toEpochMilli()
```

The phone then filters those watch-clock timestamps against a **phone-clock** boundary
(`WearHrIngest.kt:34`): `it.atMs >= active.startedAt`.

**Scenario:**
1. The watch's clock is 4 s behind the phone's (routine — Wear time sync is periodic, not continuous).
2. The user starts the session on the phone at phone-time T. The watch's HR service starts and emits its
   first samples at watch-time T-4s … T+1s.
3. `WearHrIngest` drops every sample with `atMs < T` — **the first ~4 seconds of the trace vanish** at
   every session start.
4. Separately, the user walks out of BT range for 3 minutes mid-session. Each 5 s `batchLoop` iteration
   clears `pending` and hands it to a send that silently fails → **36 batches, ~180 samples, permanently
   lost**. The `PENDING_CAP = 240` buffer that could have held them is emptied before it can help.
5. The gapped trace feeds the session-detail HR graph, the HRR stat, and the Health Connect HR series
   write-back at `WorkoutRepository.kt:323-330`.

**Fix:** Only clear `pending` after a confirmed send (have `sendHrBatch` return success, or re-queue the
batch on failure, capped at `PENDING_CAP`). For the clock skew, either carry the watch's own
`nowMs` in `HrBatchDto` so the phone can compute and apply a skew offset, or relax the ingest filter to
`atMs >= active.startedAt - CLOCK_SKEW_TOLERANCE_MS` (~60 s) — the `MAX_SAMPLES_PER_SESSION` cap already
bounds the damage from a genuinely bogus batch.

---

## [MEDIUM] `recentWatchWorkouts` can be starved to empty by Avex's own write-backs

**File:** `app/src/main/java/com/forge/app/data/health/HealthConnectManager.kt:681-694`;
caller `app/src/main/java/com/forge/app/ui/cardio/CardioViewModel.kt:100`

**What:**

```kotlin
client.readRecords(
    ReadRecordsRequest(ExerciseSessionRecord::class, timeRangeFilter = range,
        ascendingOrder = false, pageSize = limit * 2)
).records.filterNot(::isSelfWritten).take(limit)
```

The self-written filter runs **after** the page is fetched, and the page is only `limit * 2` deep. Every
finished Avex gym session (`WorkoutRepository.kt:346`) and every non-rest cardio entry
(`CardioRepository.kt:73`) writes an `ExerciseSessionRecord` into the same window — so Avex is usually the
single most prolific writer in its own candidate pool.

**Scenario:** `CardioViewModel.kt:100` calls with `limit = 6`, so `pageSize = 12`.
1. A regular user logs 5 gym sessions and 4 cardio entries in the lookback window — 9 Avex-written
   `ExerciseSessionRecord`s.
2. The 12 most recent records are fetched; 9 are Avex's own.
3. `filterNot(::isSelfWritten)` leaves **3**, of which some may already have a matching cardio entry and be
   filtered out by the caller.
4. The user's actual watch runs — the whole point of the feature — sit at positions 13, 15, 18 and are
   never seen. "Recorded with your watch — import?" silently stops appearing for exactly the most active
   users.

**Fix:** Add `dataOriginFilter` to exclude `context.packageName` server-side (Health Connect supports
excluding by origin via filtering to the origins you want), or page until `limit` non-self records are
collected rather than taking one fixed-size page.

---

## [MEDIUM] The wrist's timer-done buzz is racy — the phone's "paused" DataItem usually unmounts TimerView before the local countdown reaches zero

**File:** `app/src/main/java/com/forge/app/service/wear/WearStatePublisher.kt:130-135`, `:62-84`;
`app/src/main/java/com/forge/app/domain/timer/RestTimerController.kt:136-138` (in `shared/`);
`wear/src/main/java/com/forge/wear/ui/WearRoot.kt:82-85`;
`wear/src/main/java/com/forge/wear/ui/TimerView.kt:55-64`

**What:** When the controller's countdown expires it sets `RestTimerState(secondsRemaining = 0, isPaused = true)`
(`RestTimerController.kt:137`). `toDto()` maps that to `TimerStateDto(endAtMs = 0, paused = true,
pausedRemainingSeconds = 0)` (`WearStatePublisher.kt:131-134`), and the `paused` flip is structural
(`:73`) so it republishes immediately. On the watch, `WearRoot.kt:83` computes
`timerLive = t != null && (!t.paused || t.pausedRemainingSeconds > 0)` → **false**, and `TimerView` is
unmounted — taking with it the only code that fires the buzz:

```kotlin
LaunchedEffect(remainingSec, timer.endAtMs) {
    if (!timer.paused && remainingSec == 0 && buzzedForEndAt != timer.endAtMs) { … haptics.timerDone() … }
}
```

Whether the wrist buzzes is a race between the watch's local 200 ms tick reaching zero and the phone's
paused-DataItem arriving. It's also skewed by the clock difference: `endAtMs` is computed from the
**phone's** `clock.nowMs()` (`:131`) and counted down against the **watch's** `System.currentTimeMillis()`
(`TimerView.kt:52`).

**Scenario:**
1. Rest timer expires. The phone's controller flips to `isPaused = true` and the publisher puts the
   paused DataItem within milliseconds (`setUrgent()`, `:141`).
2. The DataItem reaches the watch in ~300 ms — often before the watch's next 200 ms tick evaluates
   `remainingSec == 0`.
3. `WearRoot` swaps to `SetView`. `TimerView` is gone. **No buzz, no `/haptic/ack`.**
4. `WearConnection.hapticAckedWithin` (`:45-46`) is false, so the phone buzzes instead — in the user's
   locker, per the plan's own "phone in the bag" scenario. The design's stated promise
   (`WearProtocol.kt:33`, "one buzz, one body part") silently inverts to the wrong body part.
5. With a 4 s watch-behind clock skew the opposite happens: the wrist buzzes 4 s early and acks, so the
   phone stays silent and the user gets the signal before the rest is actually over.

**Fix:** Fire the buzz from a lifetime that outlives `TimerView` — a `LaunchedEffect` in `WearRoot` (or the
repository) keyed on the *last non-null* `timer.endAtMs`, so an expiry detected either locally or by the
arriving `paused` DTO buzzes exactly once per `endAtMs`. Have `TimerStateDto` carry the phone's
`publishedAtMs` so the watch can compute and correct for clock offset rather than trusting a raw
cross-device wall-clock instant.

---

## [MEDIUM] The widget loads every finished session on every update

**File:** `app/src/main/java/com/forge/app/widget/ForgeWidget.kt:84`, `:88-90`, `:136-143`

**What:** `provideGlance` calls `entryPoint.sessionDao().allFinished()` — the full `Session` entity list with
no `LIMIT` and no projection — and then walks it three times: once for today's day keys (`:88-90`), once for
`lastFinishedDayKey` via `maxByOrNull` (`:96`), and once to build a `TreeSet` of every finished date
(`:136-138`). Only two things are actually needed: the most recent finished session's day key, and the dates
within the current Mon–Sun week. `statsRepository().currentStreakDays()` (`:139`) is a further independent
query on the same path.

**Scenario:** A three-year daily user has ~900 finished sessions. Every widget update — and there is one on
every program regenerate plus the system's periodic pass — deserializes 900 full entities, allocates 900
`ZonedDateTime`s, and builds a 900-element sorted set to answer "which of these 7 days have a dot". On a
budget device with a cold Room connection this runs inside the `AppWidgetService` update window and risks a
visibly blank or reverted widget.

**Fix:** Replace `allFinished()` with two narrow queries: `SELECT day_key FROM session WHERE finished_at IS
NOT NULL ORDER BY finished_at DESC LIMIT 1`, and `SELECT finished_at FROM session WHERE finished_at >= :mondayMs`.

---

## [LOW] `newerVersion` latches permanently and blocks the entire watch UI

**File:** `wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:68-70`, `:120`;
`wear/src/main/java/com/forge/wear/ui/WearRoot.kt:78`

`_newerVersion` is set to `true` by *any* path's decode and never cleared, and `UpdateScreen` sits at the top
of `WearRoot`'s `when`, ahead of every other state. One newer-version `/glance/today` payload — a path the
watch could simply have ignored — therefore blocks session mirroring, set logging and the rest timer for the
rest of the process lifetime, even if `/session/live` is still perfectly decodable. Consider latching per
path, and clearing the flag when a subsequent payload on the same path decodes `Ok`.

## [LOW] The replayed stale ack also re-fires the PR haptic and gold wash on app launch

**File:** `wear/src/main/java/com/forge/wear/ui/WearRoot.kt:52-65`;
`wear/src/main/java/com/forge/wear/data/WearDataRepository.kt:74-84`, `:106-108`

`consumedAckId` starts as `null`, so the ack seeded from the persisted `/cmd/ack` DataItem (see the HIGH
finding) is treated as fresh: if it carried `pr = true`, opening the watch app plays the PR double-tick and
paints the 1.2 s gold wash for a PR set hours old — and does so on every launch until a new command is sent.
Fixed by the same change (use the ack's own `atMs`, or clear the item once consumed).

## [LOW] `weekVolumeText` is always formatted in pounds regardless of the user's unit

**File:** `app/src/main/java/com/forge/app/service/wear/WearStatePublisher.kt:119`, `:155-156`

`formatVolumeLb` hardcodes the `lb` suffix, while the phone's own surfaces use
`formatVolumeCompact(volumeLb, unit)` (`WeightFormatter.kt:75-81`). A kg user's Week tile reads
"12.4k LB" where the phone reads "5.6k kg". Honest (the unit is labelled) but inconsistent — pass
`settingsRepo.weightUnit` through and reuse `formatVolumeCompact`.

## [LOW] `lastSetWasPr` is a per-exercise flag, not per-set — and is never read by the watch

**File:** `app/src/main/java/com/forge/app/service/wear/WatchSessionMirror.kt:118-120`;
`shared/src/main/kotlin/com/forge/shared/protocol/WearDtos.kt:34-35`

`wasPr` lives on `LoggedExercise`, so once any set of an exercise sets a PR, *every* subsequent
`SessionLiveDto` for that exercise reports `lastSetWasPr = true`. Nothing in `wear/` reads the field
(`WearRoot` uses `ack.pr` instead), so it's inert today — but it is wrong data on the wire and would
misfire the moment a surface starts consuming it. Either derive it from the last set's own PR status or
drop the field.

## [LOW] `readHrSeries`'s sample cap doesn't bound memory — `sortedBy` materialises the whole sequence first

**File:** `app/src/main/java/com/forge/app/data/health/HealthConnectManager.kt:633-645`

`.asSequence().flatMap { it.samples }.mapNotNull { … }.sortedBy { it.timeMs }.take(HR_SERIES_MAX_SAMPLES)` —
`Sequence.sortedBy` is a stateful operation that buffers every element into a list before emitting any, so
`take(12_000)` runs after the full set is already in memory. The comment describes the cap as guarding
against "a runaway provider"; it does not. Cap before sorting, or read into a bounded structure.

## [LOW] `wear/proguard-rules.pro` is empty while `isMinifyEnabled = true`

**File:** `wear/proguard-rules.pro` (0 bytes); `wear/build.gradle.kts:45-49`;
compare `app/proguard-rules.pro:15-25`

The phone module keeps a deliberate, documented `-keepclassmembers enum * { <fields>; … }` rule
(*"PERSISTENCE-CRITICAL"*) and the watch module has no rules at all. kotlinx.serialization does ship its own
consumer rules (verified: `META-INF/proguard/kotlinx-serialization-common.pro` is present in the artifact),
so the `@Serializable` wire format is safe as things stand — but the asymmetry is a trap for the next
release-only bug, and the watch has no release smoke test that would catch one. Mirror the phone's enum rule
into the wear module.

## [LOW] Every Health Connect write is marked `Metadata.manualEntry()`

**File:** `app/src/main/java/com/forge/app/data/health/HealthConnectManager.kt:283`, `:362`, `:604`, `:668`, `:697`

Sessions, HR series and active calories are all written with `recordingMethod = MANUALLY_ENTERED`, but they
are auto-recorded (a completed session, a streamed HR trace). Some readers (Samsung Health) weight or
display manual entries differently. `Metadata.autoRecorded(device = …)` / `Metadata.activelyRecorded(…)` is
the accurate classification for the session, HR-series and calorie writes; keep `manualEntry` for the
user-typed weight and body-fat values.

## [LOW] `setIndex` is derived from a live count, so a mid-session delete produces duplicate indices

**File:** `app/src/main/java/com/forge/app/domain/session/SetLogUseCase.kt:156-162`, `:193-195`;
`app/src/main/java/com/forge/app/service/wear/WatchSessionMirror.kt:112-115`

`setIndex = doneCount` (the current row count). Deleting a middle set leaves indices `0, 2` with
`doneCount = 2`, so the next set is also written as index 2. The wrist's prefill then resolves
`maxByOrNull { it.setIndex }` (`WatchSessionMirror.kt:113`, mirrored at `SetLogUseCase.kt:195`) against a
tie and picks an arbitrary one of the two, so the target weight the wrist shows can flip between two
different sets. Noted as LOW because the phone's own path does the same thing
(`DayExerciseHandlers.kt:285`, `:302`) — it is shared pre-existing behaviour, not wear-introduced — but the
wrist's prefill is where it becomes user-visible.
