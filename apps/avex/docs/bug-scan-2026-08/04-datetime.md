# Area 04 — Date / time / timezone / scheduling correctness

Scope: `core/time`, `domain/schedule`, `domain/vacation`, `domain/notify`, `domain/timer`,
`domain/coach`, `data/repo` week + streak math, `service/` workers, `widget/`, importers,
and every date-picker → stored-timestamp path.

**General health.** The codebase has a real `Clock` abstraction (`shared/.../core/time/Clock.kt`)
injected into ~24 repositories, a single `WeekMath.mondayStartMs` anchor, and several places that
explicitly got this right (`TrophyRepository.checkComebackKid` uses `ChronoUnit.DAYS`,
`CardioWeekSeries`/`CardioWeekAggregate` bucket by `LocalDate` with DST comments,
`AppLockManager` uses `SystemClock.elapsedRealtime()`, `VacationCalendar` + the vacation UI are
UTC-consistent end to end, `ReminderScheduler` uses `ZonedDateTime` not `LocalDateTime`).
The findings below are the places that did **not** get the same treatment — and they cluster on
two patterns: (a) *elapsed-milliseconds arithmetic standing in for calendar days/weeks*, and
(b) *Material3's UTC-midnight `DatePicker` contract being honoured on the way out but not on the way in.*

---

## [CRITICAL] `ImportParsing` treats a UTC ISO-8601 timestamp as local time — every imported session lands on the wrong instant, often the wrong day

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/importer/GymImporter.kt:36`
(also used by `StrongImporter.kt:34`, `FitNotesImporter.kt:34`, `GenericCsvImporter.kt:25`, `HevyImporter.kt:27,42`)

**What:**
```kotlin
private val DATE_TIME_FORMATS = listOf(
    "yyyy-MM-dd HH:mm:ss", "yyyy-MM-dd'T'HH:mm:ss", "yyyy-MM-dd'T'HH:mm:ss'Z'",
    ...
```
The trailing `'Z'` is **quoted**, i.e. a literal character, not an offset field. So
`parseEpochMillis` (line 59-74) parses `2026-03-01T23:30:00Z` with
`LocalDateTime.parse(...)` — producing a *zone-less* local date-time — and then does
`.atZone(ZoneId.systemDefault())`. The `Z` (Zulu / UTC) marker is silently discarded and the
instant is re-interpreted in the device's zone. There is no `ISO_INSTANT` /
`ISO_OFFSET_DATE_TIME` / `XXX` pattern anywhere in the list, so a timestamp carrying a real
offset (`...+02:00`) doesn't parse at all and the row is dropped (`?: continue`).

**Scenario:** A user in Auckland (UTC+13, NZDT) imports a Hevy/generic CSV whose `start_time`
column is `2026-03-01T23:30:00Z`. The true instant is **2026-03-02 12:30 NZDT** — a Monday
lunchtime session. The importer writes `startedAt` = 2026-03-01 23:30 NZDT instead: **13 hours
early and one calendar day earlier**, and one ISO week earlier if the boundary is crossed
(a `2026-03-01T23:30:00Z` Sunday-in-UTC record belongs to W10 locally but is stored as W09).
Every downstream read — `computeStreak`, `buildWeeklySessionCounts`, `WeeklyReview`'s
"last week", the year heatmap, PR ordering by `sessionStartedAt` — is then wrong, permanently,
with nothing in the UI to hint at it. A US-Pacific user (UTC-8) gets the mirror image: the same
record lands 8 hours *late*, moving evening sessions onto the next day.

**Fix:** Try `DateTimeFormatter.ISO_INSTANT` / `ISO_OFFSET_DATE_TIME` **first** and use
`Instant.from(...)` for anything carrying an offset or `Z`; only fall back to
`LocalDateTime.parse(...).atZone(zone)` for genuinely zone-less patterns. Delete the
`"yyyy-MM-dd'T'HH:mm:ss'Z'"` entry — it can only ever be wrong.

---

## [CRITICAL] Cardio date picker pre-selects the wrong calendar day — confirming without changing anything silently moves the entry back a day

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioLogSheetSections.kt:243`
(consumed by `CardioLogSheet.kt:123,352-356`)

**What:** `rememberDatePickerState(initialSelectedDateMillis = dateMs)` is handed the entry's
**local-time** epoch millis. Material3's `DatePickerState` canonicalises the initial selection by
truncating to **UTC** midnight, so the highlighted day is the entry's day *in UTC*, not in the
user's zone. The OK handler then does the reverse conversion correctly
(`combineDay` at line 271-276 reads the picked day with `ZoneOffset.UTC` and re-attaches the
local time-of-day) — so the round trip is asymmetric and lossy. Note the file's own docstring
(line 235) warns that a wrong day here "would corrupt the 'this week' counts, the cardio streak
and the week aggregations". `BodyweightLogSheet.kt:329` does it correctly
(`date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()`), which is the proof this is a bug
and not a convention.

**Scenario:** A user in Auckland (UTC+13) logs a run on **Wed 2026-06-10 at 09:00 NZST**
(`entry.date` = `2026-06-09T21:00Z`). Later they tap the date row to check it, the picker opens
highlighting **Tue 9 June**, they tap OK without touching anything. `combineDay` returns
`2026-06-09` + `09:00` local → the entry is rewritten to **Tue 2026-06-09 09:00**. The run has
silently moved a day back; if that Wednesday was the only cardio day in the ISO week that spans
the boundary, the weekly cardio bar, the "active days" count and the cardio streak all change.
Same effect for any user east of UTC logging before (24h − offset) local, and for users west of
UTC logging late in the evening (they see tomorrow).

**Fix:** Pass the canonicalised UTC midnight of the entry's *local* date:
`Instant.ofEpochMilli(dateMs).atZone(zone).toLocalDate().atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()`
— exactly what `BodyweightLogSheet` already does.

---

## [HIGH] `DatePicker.isSelectableDate` compares a UTC-midnight against a local "now" — users east of UTC cannot select today until mid-day

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioLogSheetSections.kt:246`
and `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/ui/profile/BodyweightLogSheet.kt:332`

**What:** `override fun isSelectableDate(utcTimeMillis: Long) = utcTimeMillis <= maxDateMs`,
where `maxDateMs = System.currentTimeMillis()`. `utcTimeMillis` is the candidate day's **UTC
midnight**; `maxDateMs` is the current instant. For any zone with a positive UTC offset, today's
UTC midnight is *in the future* relative to "now" for the first `offset` hours of the local day.

**Scenario:** A user in Tokyo (UTC+9) finishes a 07:00 run and opens the cardio log at 07:30 JST
on 2026-06-10. Today's UTC midnight is `2026-06-10T00:00Z` = 09:00 JST — three and a half hours
*after* `now` — so **10 June is greyed out and untappable**. The user cannot date the entry to
today until 09:00 local. In Auckland (UTC+12/+13) the cut-off is 12:00/13:00 local; every morning
lifter in Sydney, Tokyo, Auckland hits this. Conversely, a user in Los Angeles (UTC-7) *can*
select tomorrow after 17:00 local, because tomorrow's UTC midnight has already passed.

**Fix:** Compare calendar days in the display zone, not instants:
```kotlin
val todayUtcMidnight = LocalDate.now(zone).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
override fun isSelectableDate(utcTimeMillis: Long) = utcTimeMillis <= todayUtcMidnight
```

---

## [HIGH] `WeeklyReview` derives week boundaries by subtracting 7 × 86,400,000 from a local Monday-midnight — DST weeks mis-attribute sessions, volume and PRs

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/WeeklyReview.kt:59-60`
(anchor supplied by `CoachRepository.kt:796-801`, consumed at `CoachRepository.kt:622`)

**What:**
```kotlin
val lastWeekStart  = weekStartMs -  7 * DAY_MS   // DAY_MS = 24L*60*60*1000
val priorWeekStart = weekStartMs - 14 * DAY_MS
```
`weekStartMs` is computed correctly as *this* Monday 00:00 local
(`previousOrSame(MONDAY).atStartOfDay(zone)`), but the two earlier boundaries are then obtained
by fixed 24-hour arithmetic. A calendar week containing a DST transition is 23 h or 25 h × 7,
so the derived boundary is off by an hour and no longer sits on a local midnight. Every filter
that uses it — `sessions.filter { it.startedAt in lastWeekStart until weekStartMs }` (line 62-63),
the PR count (line 75) and `cardioMinutesLastWeek` (line 105) — inherits the error.

**Scenario A (spring forward, boundary moves back into Sunday):** User in Europe/London.
This Monday is 2026-03-30 00:00 BST = `2026-03-29T23:00Z`. `weekStartMs − 7·DAY_MS` =
`2026-03-22T23:00Z` = **Sunday 2026-03-22 23:00 GMT**. A session logged Sunday 22 March at 23:30
is counted in "last week" *and* was already counted in the prior week's Brief a week earlier —
double-counted across two Briefs, and the prior-week volume baseline loses it, skewing
`volumeDeltaPct`.

**Scenario B (fall back, boundary moves forward into Monday):** User in America/New_York.
This Monday is 2026-11-02 00:00 EST = `2026-11-02T05:00Z`. Minus 7 days =
`2026-10-26T05:00Z` = **Monday 2026-10-26 01:00 EDT**. A session started Monday 26 October at
00:20 (a midnight lifter) falls in *neither* window: it is after the prior-week end and before
the last-week start. It vanishes from both the "Last week" session count and the volume totals,
and the Brief's focus line ("2 short of target last week") is then wrong.

**Fix:** Do the arithmetic in `LocalDate`, not millis:
`monday.minusWeeks(1).atStartOfDay(zone).toInstant().toEpochMilli()`. Better: pass the
`ZoneId`/`LocalDate` into `assemble` rather than a bare `Long`, so all three anchors come from
one calendar computation. The same `weekStartMs − N·(7·DAY_MS)` pattern appears at
`WeeklyReview.kt:172` (`mesocycleFocus`'s `weeksIn`), where the truncating integer division can
drop a week and make the mesocycle line silently disappear for the DST week.

---

## [HIGH] `WeeklyRecapWorker` is a bare 7-day periodic anchored to first-ever app launch, but reads the *current* ISO week — false "you've been away a week" nudges, and a partial-week recap

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/service/WeeklyRecapWorker.kt:90-110,158-175`
scheduled from `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/ForgeApp.kt:45`

**What:** `PeriodicWorkRequestBuilder<WeeklyRecapWorker>(7, TimeUnit.DAYS, 6, TimeUnit.HOURS)` with
`ExistingPeriodicWorkPolicy.KEEP` and **no initial delay and no Monday anchor**. The first run is
~7 days after the user's first app launch, on whatever weekday and hour that happened to be, and
it stays on that phase forever (KEEP means it is never re-anchored; only an app-data wipe changes
it). Meanwhile the payload is `statsRepo.observeWeeklyStats().firstOrNull()`, whose `workouts`
is `observeFinishedCountSince(weekStartMs)` where `weekStartMs` is **this Monday 00:00**
(`StatsRepository.kt:103-106`) — i.e. the *in-progress* week, not the week that just ended.

**Scenario:** A user installs Avex on a Monday morning. Their recap worker therefore fires every
Monday morning. They train Tue/Thu/Sat, religiously, for months. Every Monday at ~09:00 the
worker runs, `stats.workouts` for the current ISO week is **0** (the week is 9 hours old), the
`stats.workouts == 0` branch (line 92) fires, and they get:
> **Ready when you are** — "No pressure — your plan's right where you left it. Even one session
> this week keeps your momentum going."

…the come-back nudge whose own comment says "a whole week with no sessions = a lapse". A
consistent user is told weekly that they have lapsed. Roughly 2/7 of the install base (Monday and
Tuesday anchors) hits this every single week; for the rest, the "Your week in numbers" recap
describes a *partial* current week (a Wednesday-anchored user's recap counts Mon–Wed only, so it
reports 1 workout and 8,000 lb when the real week ends at 4 and 40,000 lb).

Secondary: because the coach's Week Brief is generated on the ISO-week boundary (Monday) but the
"your coach has an update" push rides this same arbitrarily-phased worker, the push can land up
to six days after the brief is ready.

**Fix:** Anchor the worker to the intended weekday/hour with `setInitialDelay(...)` computed with
`ZonedDateTime` (as `TrainingReminderWorker.initialDelayMinutes` already does), re-anchor on
boot/upgrade, and — critically — feed the recap from the **completed** week
(`mondayStartMs(now) − 7d … mondayStartMs(now)`, computed via `LocalDate`) rather than
`observeWeeklyStats()`'s in-progress week. Gate the come-back nudge on the *previous* full week.

---

## [HIGH] `formatRelative` labels sessions "Today"/"Yesterday" from elapsed milliseconds, contradicting the calendar-correct helper elsewhere in the app

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/ui/gym/train/components/DayCardComponents.kt:131-141`
(rendered at line 88 as each day card's "last trained" line)

**What:**
```kotlin
val deltaMs = nowMs - epochMs
deltaMs < day       -> "Today"
deltaMs < 2 * day   -> "Yesterday"
deltaMs < 7 * day   -> "${deltaMs / day} days ago"
```
This is a rolling-24-hour bucket, not a calendar-day comparison. The same app already has the
correct version 30 lines of code away in `OverviewUiStateMapper.kt:136-146` (`relativeDay`, which
compares `LocalDate`s), so the two surfaces disagree about the same session.

**Scenario:** A user finishes "Upper A" on **Tuesday at 22:30**. They open the day list on
**Wednesday at 08:00** — 9.5 hours elapsed, so `deltaMs < day` and the card reads
**"Upper A · Today"**, while the Overview's recent-items row correctly reads **"YESTERDAY"** for
the same session. The user concludes they have already trained today and skips Wednesday's
session. Symmetrically, a session at Tuesday 06:00 viewed Wednesday 09:00 (27 h) reads
"Yesterday" — correct by luck — but the same session viewed Thursday 09:00 (51 h) reads
"2 days ago" when it is calendar-day 2, which is right; the failure is concentrated exactly where
it hurts: late-evening sessions viewed the next morning.

**Fix:** Convert both instants to `LocalDate` in `ZoneId.systemDefault()` and compare
(`ChronoUnit.DAYS.between`). Reuse/lift `OverviewUiStateMapper.relativeDay` so there is one
implementation.

---

## [HIGH] The deload "week" is a rolling 7 × 24 h from the apply instant, and it stamps a persisted flag onto sessions in the *following* ISO week

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/WorkoutRepository.kt:90,121-129` and `:406`
(window start written at `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/AdaptationRepository.kt:360`)

**What:** `setDeloadWeekStartMs(clock.nowMs())` records the *instant the user tapped Apply*.
`startSession` then does
`inDeloadWeek = deloadStart > 0 && clock.nowMs() - deloadStart in 0 until DELOAD_WEEK_MS`
(`DELOAD_WEEK_MS = 7L*24*60*60*1000`) and persists the result into the session row as
`deloadMarkedHere` — a **stored column**, not a derived view. Every other "week" in the coach is
ISO Mon–Sun, so this one is off-phase with all of them, and it is written into history.

**Scenario:** The coach's Monday brief is opened in the evening; the user applies the deload at
**Mon 2026-06-01 19:00** local. The rolling window runs to **Mon 2026-06-08 19:00**. On Monday
8 June at 08:00 the user starts the *first session of the new, post-deload block* — still inside
the rolling window — and it is written with `deloadMarkedHere = true`. Two consequences, both
silent and both permanent:
1. `WeeklyReview.mesocycleFocus` (`WeeklyReview.kt:167-171`) anchors the block on
   `sessions.lastOrNull { sessionType == DELOAD || deloadMarkedHere }?.startedAt`. The anchor
   jumps forward a week, so `weeksIn` is understated and the phase copy is a week behind for the
   whole next block.
2. `DeloadAdvisor`'s repeat-suppression and the stall/fatigue filters treat a normal heavy session
   as a deload session, so its (higher) volume is excluded from the reads that decide the *next*
   deload.
The mirror case also exists: the user who applies at Monday 07:00 and trains Monday 8 June at
20:00 correctly falls outside — so whether history is corrupted depends on the time of day the
user happened to tap a button.

**Fix:** Store the deload's **ISO week id** (or the local Monday-midnight of the apply), and test
membership with the same `weekId(date)` / `mondayStartMs` helper the rest of the coach uses.

---

## [HIGH] `BlockPlanner.advance` is keyed on ISO-week *string equality*: two passes 30 minutes apart across a Sunday→Monday midnight burn two block weeks and run auto-apply twice

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/BlockPlanner.kt:58`
driven by `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/CoachRepository.kt:170-197`

**What:** `if (block.lastAdvancedWeek == weekId) return block`. The guard is equality against the
*last* week id, not an ordering check, and `ensureWeeklyPass` triggers on **any app open** whose
ISO week has no recorded pass — it is not gated to Monday, and it is not gated on elapsed time.
`runCatching { blockRepository.advanceForWeek(weekId) }` (line 197) plus
`autoApplyEarnedTypes(weekId)` (line 238) both run on that path.

**Scenario:** A Sunday-night lifter. They open the app on **Sun 2026-06-07 at 23:30** to start a
session — `ensureWeeklyPass` sees no `2026-W23` pass yet this week, runs the pass, advances the
block to week 3, and in `auto` mode auto-applies the earned decisions. They finish the session at
**00:20 Monday**; the Overview reload calls `ensureWeeklyPass` again, `weekId` is now
`2026-W24`, no pass exists, so a **second full weekly pass runs 50 minutes later**: the block
advances to week 4 and a second round of decisions is generated and auto-applied. A five-week
block has burned two of its weeks — and two sets of coach changes have landed — inside one
workout. (`VOLUME_DRIFT_CAP = 2` in `AutoCoachPlanner.kt:264` bounds the volume damage but not
the block phase, the rep-shifts or the swaps.)

The converse also holds and is worth stating: because advance is "one step per pass, keyed by
week id", a user who does not open the app for three weeks advances the block by **one** week,
not three. `BlockPlanner.describe`'s "Deload in N weeks" is then wrong by however many weeks the
app went unopened.

**Fix:** Compare ISO weeks **ordinally** (store the week's Monday epoch-day or use
`ChronoUnit.WEEKS.between`) and advance by the number of whole weeks elapsed since
`lastAdvancedWeek`, clamped. At minimum, refuse to advance twice inside `MIN_ADVANCE_MS` of real
elapsed time so a midnight rollover cannot double-step.

---

## [MEDIUM] "This week" is an ISO week in some places and a rolling 7 × 24 h window in others — the Brief's numbers and the decisions it drives disagree

**Files:**
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/ui/overview/OverviewViewModel.kt:172` — `weekStartMs = clock.nowMs() - 7L*24*60*60*1000`, computed **once in the VM constructor**, used for `observeDistanceKmSince` (line 203)
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/AutoCoachPlanner.kt:268` — `s.sessions.count { it.startedAt >= s.nowMs - 7 * DAY_MS }`
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/GoalPortfolio.kt:120,174` and `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/adapt/InsightEngine.kt:144` — same rolling form
- vs. ISO-week anchors in `StatsRepository.kt:103-106`, `WeekMath.kt:12-14`, `CoachRepository.kt:796-801`, `StatsEffortAggregations.kt:80,102`

**What:** `StatsRepository.kt:100-102` carries an explicit comment that a rolling `now − WEEK_MS`
window "disagreed with the dots on early weekdays" and was replaced by the ISO week. That fix was
never propagated: the coach's own gate for "did you hit your session target this week" is still
rolling, and Home's weekly cardio distance is rolling *and* frozen at VM construction.

**Scenario:** The weekly pass runs Monday 2026-06-08 at 10:00 with a 3-session target.
`WeeklyReview`'s ISO window (Mon 1 Jun 00:00 → Mon 8 Jun 00:00) counts **2** sessions and the
Brief prints "1 short of target last week". `AutoCoachPlanner`'s rolling window
(Mon 1 Jun 10:00 → Mon 8 Jun 10:00) also picks up the session logged Monday 8 June at 07:00,
counts **3**, decides the target was met, and proposes **+1 set** on a muscle. The user reads
"you missed your target" directly above "adding a set because you're earning more".

Separately, `OverviewViewModel.weekStartMs` never re-anchors while the ViewModel lives: leave the
app open (or backgrounded with the activity alive) for two days and Home's "this week" cardio
distance still describes the 7 days ending when the screen was first opened.

**Fix:** Route every "this week" through `WeekMath.mondayStartMs(clock.nowMs())`, recomputed per
emission rather than captured in a `val`.

---

## [MEDIUM] `StatsRepository.observeWeeklyStats` freezes the week anchor at subscription time while recomputing "today" per emission

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/StatsRepository.kt:103-106` vs `:119`

**What:** `weekStartMs` is computed once, when the `Flow` is *built*; `todayDate` is recomputed
inside the `combine` lambda on every emission. They can therefore describe different weeks.
`weekDaysTrained`, `bestSessionThisWeekLb` and `workouts/volume/cardio` all derive from the frozen
anchor; `nextUpDayKey` and `trainedTodayKeys` derive from the live `todayDate`. Collected via
`SharingStarted.WhileSubscribed(5_000)` in `OverviewViewModel.kt:292-294`, so it re-anchors after
5 s with no subscribers — but not while the screen stays subscribed, and not for
`WearStatePublisher.kt:101` / `WeeklyRecapWorker.kt:90`, which take `.first()` of a freshly built
flow each time (correct) but with no guarantee about which week the worker's anchor lands in.

**Scenario:** A user leaves Avex open on the Overview at the gym on **Sunday 2026-06-07 at 23:50**
and finishes a set at 00:05 Monday. The lit day-dots and the workout count still describe the
week that ended at midnight (so Monday's session lights *Monday of last week's* row and the
"3 workouts this week" number keeps last week's total), while the "next up" day has already
rolled to Monday's slot. Nothing recovers until the screen is left for >5 s.

**Fix:** Move the anchor inside the `combine` (`mondayStartMs(clock.nowMs())` per emission), or
drive the flow off a midnight tick.

---

## [MEDIUM] Rest timer is anchored to the wall clock and guards only *backward* jumps; a forward jump ends the rest instantly

**File:** `/home/user/Avex/forge-android/shared/src/main/kotlin/com/forge/app/domain/timer/RestTimerController.kt:114-127`
(clock = `System.currentTimeMillis()` via `SystemClock` at `app/.../core/time/Clock.kt:11`)

**What:** `remainingNow` computes `endAtMs − clock.nowMs()` and explicitly re-anchors when the
result exceeds `MAX_REST_SECONDS` — i.e. it handles a *backward* NTP/manual correction. There is
no symmetric guard for a **forward** jump: any positive clock correction is indistinguishable
from time actually passing, so it is consumed straight out of the remaining rest.

**Scenario:** A user starts a 150 s rest at 18:00:00. Their phone has been off the network and its
clock is 4 minutes slow; at 18:00:20 it associates with the gym Wi-Fi and NTP corrects
`System.currentTimeMillis()` **forward by 240 s**. `endAtMs − now` is now negative, the tick job
takes the `remaining <= 0` branch (line 136-138), the finished-buzz fires, and the user is told
their 2:30 rest is over after 20 real seconds. The same happens on a manual "set date & time"
change, and on boot before the RTC is corrected.

Cross-device variant: `endAtMs` is published to the watch as an absolute phone wall-clock instant
(`shared/src/main/kotlin/com/forge/shared/protocol/WearDtos.kt:47-57`,
`app/.../service/wear/WearStatePublisher.kt:131`) and the watch renders
`timer.endAtMs − System.currentTimeMillis()` against **its own** clock
(`wear/.../ui/TimerView.kt:56`, `wear/.../glance/WearComplications.kt:73`). Any skew between the
two clocks is a direct error in the wrist countdown, and the DTO carries no `publishedAtMs` for
the watch to correct against.

**Fix:** Anchor the countdown on `SystemClock.elapsedRealtime()` (as `AppLockManager.kt:87,94`
already does) and keep the wall-clock `endAtMs` only as the *wire* representation; add a
`publishedAtMs` to `TimerStateDto` so the watch can compute `remaining = endAtMs − publishedAtMs −
(watchNow − watchReceivedAt)` and cancel skew.

---

## [MEDIUM] Timed-hold stopwatch uses the wall clock and survives process death — a killed app returns with a 3600-second plank pre-filled

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/ui/gym/train/components/SetInputRow.kt:138-152`
(ceiling from `domain/units/HoldFormatter.kt:14`, persisted via `WorkoutRepository.kt:619`)

**What:** `swAnchorMs` / `swRunning` / `swBaseSec` are `rememberSaveable`, so they survive process
death; the tick is `((System.currentTimeMillis() - swAnchorMs) / 1000L)`. Nothing bounds the gap
between the anchor and the resumed read except `coerceIn(0, MAX_HOLD_SECONDS)` = 3600.

**Scenario:** A user starts the stopwatch for a plank, gets a call, and Android kills the process.
Two hours later they reopen Avex on the same set: `swRunning` restores as `true`,
`elapsed = 7200 s`, and the field shows **60:00** — clamped to the one-hour ceiling. One tap on
"Log set" writes a 3600-second hold into `logged_set.duration_seconds`, permanently, and it
becomes the all-time best hold for that exercise. A forward clock correction mid-hold does the
same thing at smaller magnitude.

**Fix:** Use `SystemClock.elapsedRealtime()` for the anchor and persist `elapsedRealtimeNanos`'s
boot id alongside it — or simply stop the stopwatch (`swRunning = false`) when the saved anchor is
older than a sanity bound on restore.

---

## [MEDIUM] Daily reminder is a 24-hour periodic that never re-anchors — it drifts an hour at every DST transition and never recovers

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/service/TrainingReminderWorker.kt:117-138`
(`ReminderScheduler.kt:24-27` uses `KEEP` on boot)

**What:** `initialDelayMinutes` is correctly zone-aware (`ZonedDateTime`, with a comment saying
why), but the repeat is `PeriodicWorkRequestBuilder<...>(1, TimeUnit.DAYS)` — a fixed 24 h of
*elapsed* time. `ensureScheduled` on app start uses `ExistingPeriodicWorkPolicy.KEEP`, so the
existing (correct-at-arming-time) schedule is preserved and never recomputed. Only the user
toggling the setting (`apply` → `REPLACE`) re-anchors it.

**Scenario:** A user in America/New_York sets the reminder for **18:00** in January. On
**2026-03-08** the US springs forward. The worker's next fire is 24 h of elapsed time after the
last one, which is now **17:00 EDT**. It stays at 17:00 for the next seven months, then
**2026-11-01** falls back and it becomes 18:00 again — unless it drifted further, because
WorkManager's deferral under Doze compounds in one direction only. A reminder the user set for
after work arrives while they are still at their desk, half the year.

**Fix:** Use a `OneTimeWorkRequest` that re-schedules itself at the end of each run with a freshly
computed `ZonedDateTime` delay (self-rescheduling chain), or keep the periodic but re-`REPLACE` it
on `ACTION_TIMEZONE_CHANGED` / `ACTION_TIME_CHANGED` / boot.

---

## [MEDIUM] `TodayDirective` treats an elapsed-hours count as a calendar-day count — "Rest today" depends on what time of day you trained

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/TodayDirective.kt:140-159,186`

**What:**
```kotlin
val daysSinceLast = finished.maxOfOrNull { it.startedAt }?.let { ((s.nowMs - it) / DAY_MS).toInt() } ?: Int.MAX_VALUE
...
if (daysSinceLast < 1 && readinessLow) { ...Kind.CARDIO, "Take it easy today"... }
if (weeklyTarget != null && sessionsThisWeek >= weeklyTarget && daysSinceLast < 1) { ...Kind.REST, "Rest today"... }
```
`daysSinceLast < 1` is read by the surrounding copy as "trained today" but actually means "within
the last 24 hours".

**Scenario:** Two users have both hit their 4-session weekly target and both last trained
*yesterday*. User A trained **Monday 06:30**; opening the app **Tuesday 09:00** gives
`daysSinceLast = 1`, so the rest rule does not fire and the coach says "train what's next".
User B trained **Monday 20:00**; opening at **Tuesday 09:00** gives `daysSinceLast = 0`, so the
coach says **"Rest today — you've hit 4 sessions this week"**. Identical situations, opposite
directives, decided by the clock time of yesterday's session. The line at 186 has the same shape:
`"It's been $daysSinceLast days since your last session."` reports 2 for a gap the calendar calls
3 whenever the earlier session was late in the day.

**Fix:** `ChronoUnit.DAYS.between(lastSessionLocalDate, todayLocalDate)` using `s.zoneId` (which
the snapshot already carries — `AdaptationRepository.kt:123,185,216`).

---

## [MEDIUM] `LifeEvents.suppressesVerdict` voids *every* closing watch window whenever the athlete is currently flagged sick

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/LifeEvents.kt:179-186`
(called from `OutcomeWatcher.kt:66-73`, fed by `CoachRepository.kt:359-365`)

**What:** `if (state.sick) return true` ignores both `appliedAtMs` and `windowEndMs`. `state.sick`
is a *current* condition — a check-in within `SICK_WINDOW_DAYS = 3` (`LifeEvents.kt:98-104`) — but
it is applied as if it covered the decision's 14-day window. The layoff branch below it *is*
correctly window-scoped (`appliedAtMs <= gapEnd && windowEndMs >= layoff.gapStartMs`), which shows
the intent.

**Scenario:** A user applies three coach changes on **Mon 2026-05-04**. They train through the
whole fortnight and the changes clearly work. On **Sun 2026-05-17** they catch a cold and tick
"sick" in the morning check-in. On **Mon 2026-05-18** the weekly pass runs; all three windows have
just closed, `state.sick` is true, and every one is written to the durable `outcome` column as
**`not_followed`** — "you were away or unwell for this window, so it isn't judged". Fourteen days
of genuine evidence are discarded, and because `TrustLedger`/the Phase-4 promotion gate read that
column, the coach's trust in those change types does not advance. A user who is sick on any Monday
never accumulates trust.

**Fix:** Scope the sick check to the decision's window — pass the check-in timestamps through and
test `checkins.any { it.sick && it.recordedAt in appliedAtMs..windowEndMs }`, mirroring the
layoff-overlap test already directly below it.

---

## [MEDIUM] Nothing in the app reacts to a timezone or clock change, and at least one zone is captured at singleton construction

**Files:** `/home/user/Avex/forge-android/app/src/main/AndroidManifest.xml` (no
`ACTION_TIMEZONE_CHANGED` / `ACTION_TIME_CHANGED` / `ACTION_DATE_CHANGED` receiver anywhere);
`/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:46`
(`private val zone = ZoneId.systemDefault()` on an `@Singleton`)

**What:** Every day/week bucket in the app is derived at read time from
`ZoneId.systemDefault()`, which is the right storage model — but no flow, worker, widget or
ViewModel is invalidated when the zone actually changes, and `BackupRepository` snapshots the zone
once for the process lifetime.

**Scenario:** A user flies **Auckland (UTC+13) → London (UTC+1)** on Friday. The phone changes zone
mid-flight; Avex's process survives. On landing, the Overview's `weekDaysTrained` dots, the streak
and the "next up" day are all still bucketed from flows that were built with the old zone's
`LocalDate.now(zone)` captures (`StatsRepository.kt:104`, `OverviewViewModel.kt:172`), so the app
shows a day-shifted week until the process is killed. Any CSV/JSON export taken in that session
stamps its `date` column with **New Zealand** calendar days
(`BackupRepository.kt:338,89,279`) while the UI shows London days — the exported file and the app
disagree about which day each session happened on.

**Fix:** Register a receiver for `ACTION_TIMEZONE_CHANGED`/`ACTION_TIME_CHANGED` that invalidates
the stats/overview flows and refreshes the widget; make `BackupRepository.zone` a `get()` rather
than a `val`.

---

## [MEDIUM] `Clock` is injected but bypassed in several repositories and one pure domain object

**Files:**
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/StatsRepository.kt:104,119,176` — injects `clock` (line 79) but uses `LocalDate.now(zone)` for the week anchor, "today", and the whole streak computation
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/BodyweightRepository.kt:36,38,41` — injects `clock` (line 21) but the default `date` parameter and the `date == today` test use `LocalDate.now()` / `LocalTime.now()`
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/BodyMeasurementRepository.kt:30` — injects `clock` (line 20), stamps `recordedAt = clock.nowMs()` but `dateKey = LocalDate.now().toString()`
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/prefs/SettingsRepository.kt:1068` — injects `clock` (line 70) but `isQuietNow()` uses `LocalDateTime.now()`
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/trophy/TrophyEvaluator.kt:17` — a **pure** domain object calling `System.currentTimeMillis()` directly
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:74,77` — no `Clock` injected at all

**What:** The `Clock` docstring (`shared/.../core/time/Clock.kt:3-8`) says "Use this instead of
`System.currentTimeMillis()` everywhere so time-dependent logic (PR detection, weekly windows,
streaks, the rest timer) can be unit-tested with a FakeClock". Streaks — named explicitly — are the
biggest bypass.

**Scenario (not just testability):** `BodyMeasurementRepository.log` can write a row whose
`date_key` and `recorded_at` describe **different days**. The two `now()` calls are ~microseconds
apart in practice, but they are also read from two different sources; under a `FakeClock` (tests,
and any future "backfill" path) they diverge arbitrarily, and the entity's contract — "one entry
per type per day, upserted by date" — is enforced on `date_key` while every chart sorts by
`recorded_at`. The consequence at runtime is that `computeStreak` cannot be unit-tested at all:
there is no way to assert "a session at 23:59 on the 3rd and one at 00:01 on the 4th are a 2-day
streak" without changing the device clock.

**Fix:** Route these through `clock.nowMs()` and take a `ZoneId` parameter with a
`systemDefault()` default, as `CheckinRepository.todayKey(zone)` (line 32-33) already does — it is
the model to copy.

---

## [LOW] Widget refreshes at most hourly and never on a date/timezone change

**File:** `/home/user/Avex/forge-android/app/src/main/res/xml/forge_widget_info.xml`
(`android:updatePeriodMillis="3600000"`); content computed in
`/home/user/Avex/forge-android/app/src/main/java/com/forge/app/widget/ForgeWidget.kt:86-142`;
only other refresh is `ProgramRepository.kt:307`

**What:** The widget's content is correctly calendar-derived (`LocalDate.now(zone)`,
`today.with(DayOfWeek.MONDAY)`), but nothing re-renders it at midnight. `updatePeriodMillis` does
not wake the device, so the real staleness on a sleeping phone is "until the next time the device
is awake", not one hour.

**Scenario:** A 06:00 Monday gym-goer's home screen still shows Sunday's next-up day and **last
week's** Mon–Sun dot row (`○ ○ ○ ○ ○ ● ●`) when they leave the house, because the phone was
asleep from 23:00 to 06:00 and the widget last updated Sunday evening.

**Fix:** Schedule a `OneTimeWorkRequest` for the next local midnight that calls
`ForgeWidget().updateAll(context)` and re-arms itself; also update on `ACTION_TIMEZONE_CHANGED`.

---

## [LOW] `computeStreak` reads only the 120 most recent sessions, silently capping long streaks

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/StatsRepository.kt:171-193,203-205`
(`sessionDao.observeRecent(120)`)

**What:** The streak walk terminates when it runs out of loaded sessions, not when it finds a real
gap — but there is no distinction in the return value between "the streak ended here" and "the
window ended here".

**Scenario:** A user training twice a day (or a user with a genuinely long daily streak) hits 120
loaded sessions inside ~60 calendar days; their streak silently plateaus and can never exceed the
number of distinct days present in the last 120 rows. The trophy `MaxStreakAtLeast` reads a
different, unbounded source (`TrophyRepository.computeMaxStreak` over all sessions), so the Profile
can show a max streak larger than the current one even mid-streak.

**Fix:** Query distinct finished *dates* (a `SELECT DISTINCT date(...)`-shaped DAO method) rather
than a fixed row window, or raise the limit and document the bound.

---

## [LOW] Anniversary / cadence numbers computed by truncating elapsed milliseconds

**Files:**
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/trophy/TrophyEvaluator.kt:17` — `((now - firstSessionMs) / DAY_MS)` for the 365-day anniversary trophy (`program/Trophies.kt:67`)
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/repo/StatsStrengthAggregations.kt:263` — `avgDays = (avgMs / (24*60*60*1000))`
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/LifeEvents.kt:141,154` — `daysSinceLast` / `gapDays` for the 14-day layoff test
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/CoachOutcome.kt:20` — "~N days left" countdown

**What:** All measure a duration, so elapsed-ms is defensible; the failure mode is that they are
*labelled* in calendar days and compared against calendar-day thresholds.

**Scenario:** A user's first session was **2025-06-15 at 21:00**. On **2026-06-15 at 09:00** — their
one-year anniversary by any calendar — `trainingDaysElapsed` is 364, so the "One year" trophy does
not unlock until 21:00 that evening. `LifeEvents.layoff`'s `LAYOFF_MIN_DAYS = 14` has the same
off-by-a-few-hours edge: a gap the calendar calls 14 days reads as 13 whenever the earlier session
was later in the day than the current moment, so the "ease back in" ramp does not engage.

**Fix:** `ChronoUnit.DAYS.between(LocalDate, LocalDate)` for anything the user sees as a day count;
keep elapsed-ms only for true durations (the 14-day watch window in `OutcomeWatcher` is a genuine
duration and is fine as-is).

---

## [LOW] `OutcomeWatcher`'s 14-day window can only close on a pass that happens later in the day than the apply — DST adds an hour to that

**File:** `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/domain/coach/OutcomeWatcher.kt:63`

**What:** `windowClosed = s.nowMs - appliedAt >= WINDOW_DAYS * DAY_MS`. Because passes run at most
once per ISO week, a window that is a few minutes short of closing slips a **full extra week**.

**Scenario:** A user applies a change on **Mon 2026-03-09 at 20:00** (evening brief). Two weeks
later, **Mon 2026-03-23**, they open the app at 09:00 to plan the week: only 13 d 13 h have
elapsed, so no verdict. The verdict does not land until **Mon 2026-03-30** — 21 days after apply,
and `CoachOutcome.label` has been showing "still watching · ~0 days left" for a week. A DST spring
forward inside the window (e.g. Europe/London on 2026-03-29) removes another hour and widens the
band of apply-times that slip.

**Fix:** Close the window on calendar days (`ChronoUnit.DAYS.between(appliedDate, todayDate) >= 14`)
rather than on 14 × 86,400,000 ms.

---

## [LOW] `Locale`-derived and format-derived state captured at class initialisation

**Files:**
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/data/importer/GymImporter.kt:47-49` — `monthFirstLocale` is a `private val` on an `object`, evaluated once per process
- `/home/user/Avex/forge-android/app/src/main/java/com/forge/app/ui/gym/train/components/DayCardComponents.kt:129` — top-level `private val dateFormat = SimpleDateFormat("MMM d", Locale.getDefault())`

**What:** Both freeze `Locale.getDefault()` at first class load. A locale change (in-app or system)
does not update them until the process restarts.

**Scenario:** A user switches their phone from `en-US` to `en-GB` and, without rebooting, imports a
CSV containing `04/05/2024`. `monthFirstLocale` still reflects `US`, so `MM/dd/yyyy` is tried first
and the row is stored as **4 May** when the user (now on a day-first locale, and looking at a
day-first export) meant **5 April**. Silent, permanent, and affects every ambiguous date in the file.

**Fix:** Make both computed properties (`get()`), or pass the resolved `Locale` in from the call site.

---

## Verified-correct (checked, no finding)

Recording these so a later pass doesn't re-litigate them:

- `WeekMath.mondayStartMs` / `monthStartMs` (`core/time/WeekMath.kt:12-22`) — `.with(DayOfWeek.MONDAY)` is ISO previous-or-same Monday; `atStartOfDay(zone)` correctly resolves DST midnight gaps.
- `CoachRepository.weekId` (`:792-794`) uses `IsoFields.WEEK_BASED_YEAR` + `WEEK_OF_WEEK_BASED_YEAR` — week 53 and the year-end rollover are handled; no `YYYY`-vs-`yyyy` pattern bug anywhere in the repo.
- `ProfileRepository.currentStreakWeeks` (`:272-291`) walks with `minusWeeks(1)` and re-derives the ISO key each step — correct across year boundaries.
- `CardioWeekAggregate` / `CardioWeekSeries` — `LocalDate`-based day and week bucketing with explicit DST comments; `cardioWeeksOnTarget` correctly drops the in-progress week.
- `QuietHoursSchedule.isQuietAt` (`domain/notify/QuietHoursSchedule.kt:45-57`) — midnight-crossing windows, the previous-day spill-over, and the `start == end` "off" case all check out; storage is canonically Monday-first and independent of display preference.
- `VacationCalendar` + `SettingsVacationPage` + `VacationRepository` — UTC-consistent from picker (`toKey` at `:180-181`) through storage to the inclusive `!isBefore && !isAfter` test; ranges normalised both at write and at read.
- `TrophyRepository.checkComebackKid` (`:212-221`) — explicitly uses `ChronoUnit.DAYS` with a comment about the millisecond-truncation bug it replaced.
- `Session.durationMinutes()` (`data/db/entities/Session.kt:56-64`) and `BackupRepository.activeSecondsOf` — negative durations coerced to 0.
- `TrainingReminderWorker.initialDelayMinutes` (`:132-138`) — correctly `ZonedDateTime`-based, with a comment explaining why `LocalDateTime` would drift.
- `GoalsComponents.periodDaysLeft` (`:107-118`) — inclusive day count via `ChronoUnit.DAYS.between(...) + 1`, matching the `mondayStartMs`/month aggregate windows.
- `HealthConnectManager.readDailyStepTotals` (`:439-450`) — zone-aware `atStartOfDay` day buckets.
- `BodyweightLogSheet` date picker (`:329,346-347`) — correct UTC-midnight round trip in both directions.
- `AppLockManager` (`:87,94`) — uses `SystemClock.elapsedRealtime()`, immune to wall-clock changes.
- `WeeklySchedule` (`domain/schedule/WeeklySchedule.kt`) — pure index arithmetic, Monday-indexed `0..6`, consistently fed `dayOfWeek.value - 1` by all four call sites (`StatsRepository:141`, `TrainingReminderWorker:92`, `ForgeWidget:93`, `DirectiveRepository:70,84-86`).

---

## Verification note (independently re-checked)

**The cardio DatePicker CRITICAL is confirmed by direct comparison of the two sites.**

`CardioLogSheetSections.kt:242` seeds the picker with raw local-time millis:
```kotlin
val dpState = rememberDatePickerState(
    initialSelectedDateMillis = dateMs,          // local-time millis
```

`BodyweightLogSheet.kt:329` — the same dialog, written correctly:
```kotlin
val dpState = rememberDatePickerState(
    initialSelectedDateMillis = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(),
```

Material3's `DatePickerState` canonicalises its selection to UTC midnight, which is why the
bodyweight sheet converts explicitly. The cardio sheet does not, so for any user west of UTC the
seeded instant lands on the previous calendar day. Because `combineDay` converts the *result*
back correctly, opening the picker and tapping OK without touching anything rewrites the entry
one day earlier — a silent edit from a no-op interaction.

The existence of a correct sibling implementation in the same codebase is what makes this a
defect rather than a debatable convention.

**Cross-confirmation:** `GymImporter.kt:36` (the `'Z'`-as-literal timestamp bug) was found
*independently* by both the backup/import scanner and the date/time scanner, working from
different briefs and different entry points. Two unrelated derivations reaching the same
file:line materially raises confidence in that finding.
