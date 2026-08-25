package com.forge.app.data.health

import android.content.Context
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.ActiveCaloriesBurnedRecord
import androidx.health.connect.client.records.BodyFatRecord
import androidx.health.connect.client.records.DistanceRecord
import androidx.health.connect.client.records.ExerciseRoute
import androidx.health.connect.client.records.ExerciseRouteResult
import androidx.health.connect.client.records.ExerciseSessionRecord
import androidx.health.connect.client.records.HeartRateRecord
import androidx.health.connect.client.records.HeartRateVariabilityRmssdRecord
import androidx.health.connect.client.records.LeanBodyMassRecord
import androidx.health.connect.client.records.Record
import androidx.health.connect.client.records.RestingHeartRateRecord
import androidx.health.connect.client.records.SleepSessionRecord
import androidx.health.connect.client.records.StepsRecord
import androidx.health.connect.client.records.TotalCaloriesBurnedRecord
import androidx.health.connect.client.records.WeightRecord
import androidx.health.connect.client.records.metadata.Metadata
import androidx.health.connect.client.request.AggregateRequest
import androidx.health.connect.client.request.ReadRecordsRequest
import androidx.health.connect.client.time.TimeRangeFilter
import androidx.health.connect.client.units.Energy
import androidx.health.connect.client.units.Mass
import androidx.health.connect.client.units.Percentage
import com.forge.app.domain.adapt.DailySteps
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.HrvSample
import com.forge.app.domain.adapt.RestingHrSample
import com.forge.app.domain.adapt.SleepNight
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.RoutePoint
import com.forge.app.domain.health.HrPoint
import com.forge.app.domain.health.SessionWindow
import com.forge.app.domain.health.StepSample
import com.forge.app.domain.health.WatchWorkout
import com.forge.app.domain.health.bestSessionMatch
import com.forge.app.domain.health.bucketStepsByHour
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withContext
import java.time.Duration
import java.time.Instant
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.reflect.KClass

/**
 * Avex's single touchpoint with Health Connect — the only external data source the app reads.
 * Everything here is gated and fail-soft: if HC isn't installed, the user hasn't granted access,
 * or a read throws, we return an empty [HealthSnap] and the coach behaves exactly as it does with
 * no health data at all (the recovery drivers are purely additive).
 *
 * No INTERNET permission is involved: [HealthConnectClient] talks to the on-device Health Connect
 * app over IPC, so connecting it does NOT change Avex's "nothing leaves the device" stance.
 */
@Singleton
class HealthConnectManager @Inject constructor(
    @ApplicationContext private val context: Context
) {
    /** The read-only recovery permissions Avex requests (sleep + resting heart rate). */
    val permissions: Set<String> = setOf(
        HealthPermission.getReadPermission(SleepSessionRecord::class),
        HealthPermission.getReadPermission(RestingHeartRateRecord::class)
    )

    /**
     * Bodyweight permissions (HC-2/HC-3) — read so a smart-scale value can flow INTO Avex, write so
     * a Avex weigh-in can flow BACK to Health Connect. Kept as a SEPARATE set from [permissions] so
     * each integration is independently opt-in: connecting recovery never silently asks for weight,
     * and vice-versa.
     */
    val weightPermissions: Set<String> = setOf(
        HealthPermission.getReadPermission(WeightRecord::class),
        HealthPermission.getWritePermission(WeightRecord::class)
    )

    /**
     * Body-fat-% permissions (GYMAP-62) — read so a smart-scale reading can flow INTO Avex, write so
     * a Avex figure can flow BACK to Health Connect. Its own set like [weightPermissions], so enabling
     * body-fat sync never silently asks for weight/recovery, and vice-versa. HC stores body fat as its
     * own [BodyFatRecord] (a percentage), separate from [WeightRecord], so it needs its own grant.
     */
    val bodyFatPermissions: Set<String> = setOf(
        HealthPermission.getReadPermission(BodyFatRecord::class),
        HealthPermission.getWritePermission(BodyFatRecord::class)
    )

    /**
     * Active-calorie permission (HC-4) — write only, so a Avex session can flow OUT to Health
     * Connect's daily energy total. Its own set, like [weightPermissions]: enabling calorie sync
     * never silently asks for sleep/HR or weight, and vice-versa.
     */
    val caloriePermissions: Set<String> = setOf(
        HealthPermission.getWritePermission(ActiveCaloriesBurnedRecord::class)
    )

    /**
     * Steps permission — read only, so a watch/ring's daily step counts can flow INTO the cardio
     * screen's hourly-steps graph. Its own set, like [weightPermissions] / [caloriePermissions], so
     * enabling steps never silently asks for sleep/HR, weight, or calories, and vice-versa.
     */
    val stepsPermissions: Set<String> = setOf(
        HealthPermission.getReadPermission(StepsRecord::class)
    )

    /**
     * Exercise-session read permission — lets Avex find the watch session that matches a cardio
     * entry so it can offer that session's GPS route. The route DATA itself isn't covered by any
     * blanket permission in connect-client 1.1.0; it's released one session at a time through Health
     * Connect's own consent screen ([androidx.health.connect.client.contracts.ExerciseRouteRequestContract]).
     * Its own set, so enabling routes never silently asks for steps/sleep/HR/weight/calories.
     */
    val exercisePermissions: Set<String> = setOf(
        HealthPermission.getReadPermission(ExerciseSessionRecord::class)
    )

    /**
     * HRV read permission (W6) — a watch's overnight heart-rate variability (RMSSD), the strongest
     * recovery signal it produces. Requested ALONGSIDE the recovery set by the same Sleep & heart
     * rate row (one concept, one row), but kept out of [permissions] so existing grants stay valid:
     * the row's connected state still keys on sleep + resting HR alone, and HRV is opportunistic.
     */
    val hrvPermissions: Set<String> = setOf(
        HealthPermission.getReadPermission(HeartRateVariabilityRmssdRecord::class)
    )

    /**
     * Lean-body-mass read permission (W6) — the watch's BIA skeletal-muscle reading. Read-only and
     * its own set like [stepsPermissions]: the watch is the sole author, Avex only imports.
     */
    val leanMassPermissions: Set<String> = setOf(
        HealthPermission.getReadPermission(LeanBodyMassRecord::class)
    )

    /**
     * Watch-workout READ permissions (W5) — everything the "your watch workouts, understood" layer
     * consumes: the workout's HR series for the cardio HR graph, its measured distance and calories
     * for stat enrichment / import prefill, and the session list itself. One set, one Recovery row —
     * these reads only ever serve that one feature, so they opt in together. ExerciseSessionRecord
     * read may already be granted via the GPS-routes row; re-requesting a granted permission is a
     * no-op in the HC flow.
     */
    val watchWorkoutPermissions: Set<String> = setOf(
        HealthPermission.getReadPermission(HeartRateRecord::class),
        HealthPermission.getReadPermission(DistanceRecord::class),
        HealthPermission.getReadPermission(TotalCaloriesBurnedRecord::class),
        HealthPermission.getReadPermission(ExerciseSessionRecord::class)
    )

    /**
     * Exercise-session WRITE permission (W0) — lets a finished Avex gym or cardio session flow OUT
     * to Health Connect as an [ExerciseSessionRecord], which is what makes it appear in Samsung
     * Health / Google Fit. Write-only and its own set like [caloriePermissions]: enabling
     * session write-back never silently asks for any read.
     */
    val sessionWritePermissions: Set<String> = setOf(
        HealthPermission.getWritePermission(ExerciseSessionRecord::class),
        // W3: the watch's live HR trace rides the written session as a HeartRateRecord series.
        HealthPermission.getWritePermission(HeartRateRecord::class)
    )

    /** SDK_AVAILABLE / SDK_UNAVAILABLE / SDK_UNAVAILABLE_PROVIDER_UPDATE_REQUIRED. */
    fun sdkStatus(): Int = HealthConnectClient.getSdkStatus(context)

    /** True when a usable Health Connect provider is installed on this device. */
    val isAvailable: Boolean get() = sdkStatus() == HealthConnectClient.SDK_AVAILABLE

    /** True when the provider exists but needs a Play-store update before it can be used. */
    val needsUpdate: Boolean get() = sdkStatus() == HealthConnectClient.SDK_UNAVAILABLE_PROVIDER_UPDATE_REQUIRED

    // The client is created once per process and reused — getOrCreate (the costly part) then doesn't
    // re-run on every snapshot()/coach pass. sdkStatus stays live for the Settings page's install/
    // connect state; only the client handle is memoized.
    @Volatile private var cachedClient: HealthConnectClient? = null

    private fun clientOrNull(): HealthConnectClient? {
        cachedClient?.let { return it }
        if (!isAvailable) return null
        return runCatching { HealthConnectClient.getOrCreate(context) }.getOrNull()?.also { cachedClient = it }
    }

    /** runCatching that still honours cooperative cancellation: a cancelled coroutine rethrows instead
     *  of being swallowed as a soft "null/false" result, while genuine provider errors → null. Inline so
     *  the suspend calls inside the block run in the caller's coroutine context. */
    private inline fun <T> hcCatching(block: () -> T): T? =
        try { block() }
        catch (c: kotlinx.coroutines.CancellationException) { throw c }
        catch (t: Throwable) { null }

    /** Permissions the user has already granted (empty when HC is absent or unreadable). The
     *  permission-controller call is a binder IPC, so it runs off the main thread. */
    suspend fun grantedPermissions(): Set<String> = withContext(Dispatchers.IO) {
        hcCatching { clientOrNull()?.permissionController?.getGrantedPermissions() }.orEmpty()
    }

    /** True only when every recovery permission is granted. */
    suspend fun hasAllPermissions(): Boolean = grantedPermissions().containsAll(permissions)

    /** True when Avex may READ bodyweight from Health Connect (HC-2). */
    suspend fun canReadWeight(): Boolean =
        grantedPermissions().contains(HealthPermission.getReadPermission(WeightRecord::class))

    /** True when Avex may WRITE bodyweight back to Health Connect (HC-3). */
    suspend fun canWriteWeight(): Boolean =
        grantedPermissions().contains(HealthPermission.getWritePermission(WeightRecord::class))

    /** A bodyweight reading mirrored out of Health Connect as plain Kotlin (lb + when it was taken). */
    data class HcWeight(val weightLb: Double, val timeMs: Long)

    /**
     * The most recent [WeightRecord] before [nowMs], in lb — or null if HC is absent, the read
     * permission isn't granted, there are no records, or the read throws. Reads a single
     * descending-ordered row so a long weight history never pulls more than one record.
     */
    suspend fun latestWeight(nowMs: Long): HcWeight? = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext null
        if (!canReadWeight()) return@withContext null
        hcCatching {
            client.readRecords(
                ReadRecordsRequest(
                    WeightRecord::class,
                    timeRangeFilter = TimeRangeFilter.before(Instant.ofEpochMilli(nowMs)),
                    ascendingOrder = false,
                    pageSize = 1
                )
            ).records.firstOrNull()?.let { HcWeight(it.weight.inPounds, it.time.toEpochMilli()) }
        }
    }

    /**
     * The FULL weight history in `[sinceMs, untilMs]`, oldest first, in lb (GYMAP-63 first-connect
     * backfill). Pages through the provider (unlike [latestWeight]'s single row) and caps at
     * [HISTORY_MAX_RECORDS] so a pathological history can't pull unbounded records.
     *
     * Returns **null** when the read couldn't happen (no provider / not granted / a read error) — as
     * opposed to an empty list, which means the read SUCCEEDED and HC simply has no records. The caller
     * needs that distinction: a transient failure must not be mistaken for "nothing to import" and
     * latch the one-time backfill flag (GYMAP-63).
     */
    suspend fun readWeightHistory(sinceMs: Long, untilMs: Long): List<HcWeight>? = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext null
        if (!canReadWeight()) return@withContext null
        hcCatching {
            val out = ArrayList<HcWeight>()
            val range = TimeRangeFilter.between(Instant.ofEpochMilli(sinceMs), Instant.ofEpochMilli(untilMs))
            var token: String? = null
            do {
                val resp = client.readRecords(
                    ReadRecordsRequest(
                        WeightRecord::class,
                        timeRangeFilter = range,
                        ascendingOrder = true,
                        pageSize = HISTORY_PAGE_SIZE,
                        pageToken = token
                    )
                )
                resp.records.forEach { out.add(HcWeight(it.weight.inPounds, it.time.toEpochMilli())) }
                token = resp.pageToken
                // Stop on an empty page even if a token lingers: without this, a provider that returns
                // a non-null continuation token but no records would loop forever (out.size never grows).
            } while (token != null && resp.records.isNotEmpty() && out.size < HISTORY_MAX_RECORDS)
            out
        } // null when the read threw — distinct from a successful empty read.
    }

    /**
     * Write a single bodyweight reading to Health Connect, returning whether it landed. Best-effort
     * and fail-soft: no provider / no write permission / a provider error all return false without
     * throwing, so a failed mirror never breaks the local log (the source of truth stays the DB).
     */
    suspend fun writeWeight(weightLb: Double, atMs: Long): Boolean = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext false
        if (!canWriteWeight()) return@withContext false
        hcCatching {
            client.insertRecords(
                listOf(
                    WeightRecord(
                        time = Instant.ofEpochMilli(atMs),
                        zoneOffset = null,
                        weight = Mass.pounds(weightLb),
                        metadata = Metadata.manualEntry()
                    )
                )
            )
            true
        } ?: false
    }

    /** True when Avex may READ body fat % from Health Connect (GYMAP-62). */
    suspend fun canReadBodyFat(): Boolean =
        grantedPermissions().contains(HealthPermission.getReadPermission(BodyFatRecord::class))

    /** True when Avex may WRITE body fat % back to Health Connect (GYMAP-62). */
    suspend fun canWriteBodyFat(): Boolean =
        grantedPermissions().contains(HealthPermission.getWritePermission(BodyFatRecord::class))

    /** A body-fat reading mirrored out of Health Connect as plain Kotlin (percent + when it was taken). */
    data class HcBodyFat(val percent: Double, val timeMs: Long)

    /**
     * The most recent [BodyFatRecord] before [nowMs], as a percentage — or null if HC is absent, the
     * read permission isn't granted, there are no records, or the read throws. Reads a single
     * descending-ordered row so a long history never pulls more than one record.
     */
    suspend fun latestBodyFat(nowMs: Long): HcBodyFat? = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext null
        if (!canReadBodyFat()) return@withContext null
        hcCatching {
            client.readRecords(
                ReadRecordsRequest(
                    BodyFatRecord::class,
                    timeRangeFilter = TimeRangeFilter.before(Instant.ofEpochMilli(nowMs)),
                    ascendingOrder = false,
                    pageSize = 1
                )
            ).records.firstOrNull()?.let { HcBodyFat(it.percentage.value, it.time.toEpochMilli()) }
        }
    }

    /**
     * Write a single body-fat reading (percentage) to Health Connect, returning whether it landed.
     * Best-effort and fail-soft like [writeWeight]: no provider / no write permission / a provider
     * error all return false without throwing, so a failed mirror never breaks the local log.
     */
    suspend fun writeBodyFat(percent: Double, atMs: Long): Boolean = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext false
        if (!canWriteBodyFat()) return@withContext false
        hcCatching {
            client.insertRecords(
                listOf(
                    BodyFatRecord(
                        time = Instant.ofEpochMilli(atMs),
                        zoneOffset = null,
                        percentage = Percentage(percent),
                        metadata = Metadata.manualEntry()
                    )
                )
            )
            true
        } ?: false
    }

    /** True when Avex may WRITE active calories to Health Connect (HC-4). */
    suspend fun canWriteActiveCalories(): Boolean =
        grantedPermissions().contains(HealthPermission.getWritePermission(ActiveCaloriesBurnedRecord::class))

    /**
     * Write one finished session's estimated active calories to Health Connect over `[startMs, endMs]`,
     * returning whether it landed. Best-effort and fail-soft like [writeWeight]: no provider / no write
     * permission / a provider error all return false without throwing, so a failed mirror never breaks
     * the local finish. The span is clamped to be strictly positive — HC rejects a zero/negative range.
     */
    suspend fun writeActiveCalories(
        kcal: Double,
        startMs: Long,
        endMs: Long,
        clientRecordId: String,
        clientRecordVersion: Long
    ): Boolean = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext false
        if (!canWriteActiveCalories()) return@withContext false
        val safeEnd = maxOf(endMs, startMs + 1)
        hcCatching {
            client.insertRecords(
                listOf(
                    ActiveCaloriesBurnedRecord(
                        startTime = Instant.ofEpochMilli(startMs),
                        startZoneOffset = null,
                        endTime = Instant.ofEpochMilli(safeEnd),
                        endZoneOffset = null,
                        energy = Energy.kilocalories(kcal),
                        // Keyed like the session and HR mirrors, which are upserts on (our package,
                        // clientRecordId). This one carried no key, so every pass INSERTED: a
                        // re-finish or an orphan-session recovery added a second calorie record for
                        // the same workout, inflating the day's burn in Samsung Health or Fit with
                        // no local trace to find it by and no way to repair it.
                        metadata = Metadata.manualEntry(
                            clientRecordId = clientRecordId,
                            clientRecordVersion = clientRecordVersion
                        )
                    )
                )
            )
            true
        } ?: false
    }

    /**
     * Read the recovery signals in `[startMs, nowMs]` into a pure [HealthSnap]: sleep (with stages
     * when the provider reports them, W6) + resting HR behind the recovery grant, and — each behind
     * its OWN grant, independently fail-soft — HRV (W6) and per-day steps (W6). Any missing grant
     * or read error just leaves that field empty: recovery signals are additive, never load-bearing.
     */
    suspend fun readRecovery(startMs: Long, nowMs: Long): HealthSnap = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext HealthSnap()
        val granted = grantedPermissions()
        val range = TimeRangeFilter.between(Instant.ofEpochMilli(startMs), Instant.ofEpochMilli(nowMs))

        val recoveryGranted = granted.containsAll(permissions)
        val sleep = if (!recoveryGranted) emptyList() else hcCatching {
            client.readRecords(ReadRecordsRequest(SleepSessionRecord::class, timeRangeFilter = range))
                .records.mapNotNull { rec ->
                    // Drop corrupt records (a third-party app can write endTime <= startTime) and cap
                    // absurd spans (a forgotten wearable can log a multi-day "night") so one bad row
                    // can't skew DeloadAdvisor's sleep average up or down.
                    val min = Duration.between(rec.startTime, rec.endTime).toMinutes()
                    if (min <= 0) null else SleepNight(
                        endedAtMs = rec.endTime.toEpochMilli(),
                        durationMin = min.coerceAtMost(MAX_SLEEP_MIN).toInt(),
                        deepMin = rec.stageMinutes(SleepSessionRecord.STAGE_TYPE_DEEP),
                        remMin = rec.stageMinutes(SleepSessionRecord.STAGE_TYPE_REM)
                    )
                }
        }.orEmpty()
        val hr = if (!recoveryGranted) emptyList() else hcCatching {
            client.readRecords(ReadRecordsRequest(RestingHeartRateRecord::class, timeRangeFilter = range))
                .records.mapNotNull {
                    // Ignore physiologically impossible readings (0 bpm corrupt rows would distort the baseline).
                    val bpm = it.beatsPerMinute.toInt()
                    if (bpm in MIN_BPM..MAX_BPM) RestingHrSample(timeMs = it.time.toEpochMilli(), bpm = bpm) else null
                }
        }.orEmpty()

        val hrvGranted = HealthPermission.getReadPermission(HeartRateVariabilityRmssdRecord::class) in granted
        val hrv = if (!hrvGranted) emptyList() else hcCatching {
            client.readRecords(ReadRecordsRequest(HeartRateVariabilityRmssdRecord::class, timeRangeFilter = range))
                .records.mapNotNull {
                    val rmssd = it.heartRateVariabilityMillis
                    if (rmssd > 0.0 && rmssd < MAX_RMSSD_MS) HrvSample(timeMs = it.time.toEpochMilli(), rmssdMs = rmssd) else null
                }
        }.orEmpty()

        HealthSnap(
            sleepNights = sleep, restingHr = hr, hrv = hrv,
            dailySteps = readDailyStepTotals(startMs, nowMs)
        )
    }

    /**
     * Per-day step totals in `[startMs, endMs]`, bucketed on the local calendar day (W6). One read,
     * grouped client-side; fail-soft to empty. Feeds the coach's daily-movement input and the
     * Home movement line's typical-day compare.
     */
    suspend fun readDailyStepTotals(startMs: Long, endMs: Long): List<DailySteps> = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext emptyList()
        if (!canReadSteps()) return@withContext emptyList()
        hcCatching {
            val zone = java.time.ZoneId.systemDefault()
            val range = TimeRangeFilter.between(Instant.ofEpochMilli(startMs), Instant.ofEpochMilli(endMs))
            client.readAllPages(StepsRecord::class, range)
                .groupBy { Instant.ofEpochMilli(it.startTime.toEpochMilli()).atZone(zone).toLocalDate() }
                .map { (day, recs) ->
                    DailySteps(
                        dayStartMs = day.atStartOfDay(zone).toInstant().toEpochMilli(),
                        steps = recs.sumOf { it.count }.toInt()
                    )
                }
                .sortedBy { it.dayStartMs }
        }.orEmpty()
    }

    /**
     * Read EVERY record of [type] in [range], following Health Connect's page tokens.
     *
     * `readRecords(...).records` returns only the FIRST page — 1000 records by default, ordered
     * ascending. High-frequency record types blow past that easily: a watch writing steps or heart
     * rate every few minutes fills 1000 rows well inside a two-week window, so the reads that
     * matter were silently truncated to their OLDEST page. Today's data was the part that fell off
     * the end, which is why a step read over a fortnight could report zero steps today and hand the
     * coach a phantom sedentary athlete.
     *
     * Bounded by [HISTORY_MAX_RECORDS], and it stops on an empty page even if a token lingers —
     * without that a provider returning a non-null continuation token with no records loops
     * forever. Both guards are lifted from readWeightHistory, which already got this right.
     */
    private suspend fun <T : Record> HealthConnectClient.readAllPages(
        type: KClass<T>,
        range: TimeRangeFilter
    ): List<T> {
        val out = mutableListOf<T>()
        var token: String? = null
        do {
            val resp = readRecords(
                ReadRecordsRequest(type, timeRangeFilter = range, pageSize = HISTORY_PAGE_SIZE, pageToken = token)
            )
            out += resp.records
            token = resp.pageToken
        } while (token != null && resp.records.isNotEmpty() && out.size < HISTORY_MAX_RECORDS)
        return out
    }

    /** Minutes this sleep session spent in [stageType], per the provider's stage list (0 = absent). */
    private fun SleepSessionRecord.stageMinutes(stageType: Int): Int =
        stages.filter { it.stage == stageType }
            .sumOf { Duration.between(it.startTime, it.endTime).toMinutes().coerceAtLeast(0) }
            .toInt()

    /** True when Avex may READ steps from Health Connect. */
    suspend fun canReadSteps(): Boolean =
        grantedPermissions().contains(HealthPermission.getReadPermission(StepsRecord::class))

    /**
     * Read every [StepsRecord] in `[dayStartMs, dayEndMs)` and bucket it into an hourly breakdown for
     * the cardio screen. Fail-soft like the rest: no provider / no read permission / a provider error
     * all return an empty [CardioWearableDay], which the UI renders as "no wearable data" (the steps
     * graph simply doesn't appear). Routes stay empty — GPS is a separate, deferred read.
     */
    suspend fun readStepsDay(dayStartMs: Long, dayEndMs: Long): CardioWearableDay = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext CardioWearableDay()
        if (!canReadSteps()) return@withContext CardioWearableDay()
        hcCatching {
            val range = TimeRangeFilter.between(Instant.ofEpochMilli(dayStartMs), Instant.ofEpochMilli(dayEndMs))
            val samples = client.readAllPages(StepsRecord::class, range)
                .map { StepSample(startMs = it.startTime.toEpochMilli(), count = it.count) }
            CardioWearableDay(hourlySteps = bucketStepsByHour(samples, java.time.ZoneId.systemDefault()))
        } ?: CardioWearableDay()
    }

    /** True when Avex may READ exercise sessions from Health Connect. */
    suspend fun canReadExercise(): Boolean =
        grantedPermissions().contains(HealthPermission.getReadPermission(ExerciseSessionRecord::class))

    /** True when Avex may WRITE exercise sessions to Health Connect (W0). */
    suspend fun canWriteExerciseSessions(): Boolean =
        grantedPermissions().contains(HealthPermission.getWritePermission(ExerciseSessionRecord::class))

    /**
     * Write one finished session to Health Connect as an [ExerciseSessionRecord] (W0), returning
     * whether it landed. Fail-soft like every write here. [clientRecordId] makes the write an
     * UPSERT keyed on (our package, clientRecordId): re-finishing the same session or editing a
     * cardio entry UPDATES the HC record instead of duplicating it, and [deleteExerciseSession]
     * can remove it when the local entry is deleted. [clientRecordVersion] must not decrease for
     * an update to land, so callers pass a stamp that grows with each edit.
     */
    suspend fun writeExerciseSession(
        clientRecordId: String,
        clientRecordVersion: Long,
        exerciseType: Int,
        title: String,
        startMs: Long,
        endMs: Long
    ): Boolean = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext false
        if (!canWriteExerciseSessions()) return@withContext false
        val safeEnd = maxOf(endMs, startMs + 1)
        hcCatching {
            client.insertRecords(
                listOf(
                    ExerciseSessionRecord(
                        startTime = Instant.ofEpochMilli(startMs),
                        startZoneOffset = null,
                        endTime = Instant.ofEpochMilli(safeEnd),
                        endZoneOffset = null,
                        exerciseType = exerciseType,
                        title = title,
                        metadata = Metadata.manualEntry(
                            clientRecordId = clientRecordId,
                            clientRecordVersion = clientRecordVersion
                        )
                    )
                )
            )
            true
        } ?: false
    }

    /** True when Avex may WRITE heart-rate series to Health Connect (W3). */
    suspend fun canWriteHeartRate(): Boolean =
        grantedPermissions().contains(HealthPermission.getWritePermission(HeartRateRecord::class))

    /**
     * Write the watch's HR trace for a finished session as one [HeartRateRecord] series (W3),
     * keyed like the session record so a re-finish updates rather than duplicates. Fail-soft.
     */
    suspend fun writeHrSeries(
        clientRecordId: String,
        clientRecordVersion: Long,
        startMs: Long,
        endMs: Long,
        samples: List<HrPoint>
    ): Boolean = withContext(Dispatchers.IO) {
        if (samples.isEmpty()) return@withContext false
        val client = clientOrNull() ?: return@withContext false
        if (!canWriteHeartRate()) return@withContext false
        val safeEnd = maxOf(endMs, startMs + 1)
        hcCatching {
            client.insertRecords(
                listOf(
                    HeartRateRecord(
                        startTime = Instant.ofEpochMilli(startMs),
                        startZoneOffset = null,
                        endTime = Instant.ofEpochMilli(safeEnd),
                        endZoneOffset = null,
                        samples = samples
                            .filter { it.timeMs in startMs..safeEnd }
                            .map { HeartRateRecord.Sample(Instant.ofEpochMilli(it.timeMs), it.bpm.toLong()) },
                        metadata = Metadata.manualEntry(
                            clientRecordId = clientRecordId,
                            clientRecordVersion = clientRecordVersion
                        )
                    )
                )
            )
            true
        } ?: false
    }

    /**
     * Delete the Health Connect session Avex wrote under [clientRecordId] (a deleted cardio entry
     * takes its mirror with it). Fail-soft; deleting an id that was never written is a no-op.
     */
    suspend fun deleteExerciseSession(clientRecordId: String): Boolean = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext false
        if (!canWriteExerciseSessions()) return@withContext false
        hcCatching {
            client.deleteRecords(
                ExerciseSessionRecord::class,
                recordIdsList = emptyList(),
                clientRecordIdsList = listOf(clientRecordId)
            )
            true
        } ?: false
    }

    /**
     * True when [record] was written by Avex itself (W0 write-back). Session READS must skip
     * self-authored records: our own (routeless) sessions would otherwise time-match a cardio
     * entry better than the user's real watch session and shadow its GPS route / HR series.
     */
    private fun isSelfWritten(record: Record): Boolean =
        record.metadata.dataOrigin.packageName == context.packageName

    // ─── Watch workouts (W5): HR series + measured stats + import candidates ──

    /** True when Avex may READ heart-rate series from Health Connect (W5). */
    suspend fun canReadHeartRate(): Boolean =
        grantedPermissions().contains(HealthPermission.getReadPermission(HeartRateRecord::class))

    /** True when Avex may READ lean body mass from Health Connect (W6). */
    suspend fun canReadLeanMass(): Boolean =
        grantedPermissions().contains(HealthPermission.getReadPermission(LeanBodyMassRecord::class))

    /** A lean-mass reading mirrored out of Health Connect as plain Kotlin (lb + when it was taken). */
    data class HcLeanMass(val weightLb: Double, val timeMs: Long)

    /**
     * The most recent [LeanBodyMassRecord] before [nowMs], in lb — or null if HC is absent, the read
     * permission isn't granted, there are no records, or the read throws. Single-row read like
     * [latestWeight]/[latestBodyFat].
     */
    suspend fun latestLeanMass(nowMs: Long): HcLeanMass? = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext null
        if (!canReadLeanMass()) return@withContext null
        hcCatching {
            client.readRecords(
                ReadRecordsRequest(
                    LeanBodyMassRecord::class,
                    timeRangeFilter = TimeRangeFilter.before(Instant.ofEpochMilli(nowMs)),
                    ascendingOrder = false,
                    pageSize = 1
                )
            ).records.firstOrNull()?.let { HcLeanMass(it.mass.inPounds, it.time.toEpochMilli()) }
        }
    }

    /**
     * Every heart-rate sample in `[startMs, endMs]`, flattened out of HC's series records, ordered
     * by time, physiologically bounded, and capped so a runaway provider can't hand back an
     * unbounded list. Fail-soft to empty like every read here.
     */
    suspend fun readHrSeries(startMs: Long, endMs: Long): List<HrPoint> = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext emptyList()
        if (!canReadHeartRate()) return@withContext emptyList()
        hcCatching {
            val range = TimeRangeFilter.between(Instant.ofEpochMilli(startMs), Instant.ofEpochMilli(endMs))
            client.readAllPages(HeartRateRecord::class, range)
                .asSequence()
                .flatMap { it.samples }
                .mapNotNull { s ->
                    val bpm = s.beatsPerMinute.toInt()
                    if (bpm in MIN_BPM..MAX_BPM) HrPoint(timeMs = s.time.toEpochMilli(), bpm = bpm) else null
                }
                .sortedBy { it.timeMs }
                .take(HR_SERIES_MAX_SAMPLES)
                .toList()
        }.orEmpty()
    }

    /**
     * The watch session that best matches a cardio entry on its day, summarised with its measured
     * stats (W5) — the same time-window match as [matchSessionRoute], but for the HR graph and the
     * "watch measured" reading rather than the GPS track. Self-written sessions are excluded.
     * Distance/calories aggregate over the session's span, each independently fail-soft to null.
     */
    suspend fun matchWatchSession(
        entryStartMs: Long,
        entryDurationMin: Int,
        dayStartMs: Long,
        dayEndMs: Long
    ): WatchWorkout? = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext null
        if (!canReadExercise()) return@withContext null
        hcCatching {
            val range = TimeRangeFilter.between(Instant.ofEpochMilli(dayStartMs), Instant.ofEpochMilli(dayEndMs))
            val records = client.readRecords(ReadRecordsRequest(ExerciseSessionRecord::class, timeRangeFilter = range))
                .records.filterNot(::isSelfWritten)
            val windows = records.mapIndexed { i, r ->
                SessionWindow(index = i, startMs = r.startTime.toEpochMilli(), endMs = r.endTime.toEpochMilli())
            }
            val idx = bestSessionMatch(entryStartMs, entryDurationMin, windows) ?: return@hcCatching null
            records[idx].toWatchWorkout(client)
        }
    }

    /**
     * The most recent NON-Avex exercise sessions in `[sinceMs, nowMs]` — the candidate pool for
     * "recorded with your watch — import?" suggestions (W5). Bounded at [limit]; the caller filters
     * out sessions that already have a matching cardio entry and ones the user dismissed.
     */
    suspend fun recentWatchWorkouts(sinceMs: Long, nowMs: Long, limit: Int = 10): List<WatchWorkout> =
        withContext(Dispatchers.IO) {
            val client = clientOrNull() ?: return@withContext emptyList()
            if (!canReadExercise()) return@withContext emptyList()
            hcCatching {
                val range = TimeRangeFilter.between(Instant.ofEpochMilli(sinceMs), Instant.ofEpochMilli(nowMs))
                // PAGE until [limit] non-self records are found, rather than filtering one fixed
                // page. The self-written filter runs client-side, and Avex writes an
                // ExerciseSessionRecord for every finished gym session AND every non-rest cardio
                // entry — so it is usually the most prolific writer in its own candidate pool. With
                // a single `limit * 2` page, an active user's nine Avex records out of twelve left
                // three candidates, and the watch runs this feature exists to surface sat at
                // positions 13, 15 and 18, never seen. "Recorded with your watch — import?" stopped
                // appearing for exactly the users who train most.
                val out = ArrayList<ExerciseSessionRecord>(limit)
                var token: String? = null
                var pages = 0
                do {
                    val resp = client.readRecords(
                        ReadRecordsRequest(
                            ExerciseSessionRecord::class, timeRangeFilter = range,
                            ascendingOrder = false, pageSize = limit * 2, pageToken = token
                        )
                    )
                    resp.records.filterNot(::isSelfWritten).forEach { if (out.size < limit) out += it }
                    token = resp.pageToken
                    pages++
                } while (token != null && resp.records.isNotEmpty() && out.size < limit && pages < WATCH_WORKOUT_MAX_PAGES)
                out.map { it.toWatchWorkout(client) }
            }.orEmpty()
        }

    /** Summarise one HC session with its measured distance/calories (each gated + fail-soft). */
    private suspend fun ExerciseSessionRecord.toWatchWorkout(client: HealthConnectClient): WatchWorkout {
        val granted = grantedPermissions()
        val start = startTime.toEpochMilli()
        val end = endTime.toEpochMilli()
        val metrics = buildSet {
            if (HealthPermission.getReadPermission(DistanceRecord::class) in granted) add(DistanceRecord.DISTANCE_TOTAL)
            if (HealthPermission.getReadPermission(TotalCaloriesBurnedRecord::class) in granted) add(TotalCaloriesBurnedRecord.ENERGY_TOTAL)
        }
        val aggregate = if (metrics.isEmpty()) null else hcCatching {
            client.aggregate(
                AggregateRequest(
                    metrics = metrics,
                    timeRangeFilter = TimeRangeFilter.between(startTime, endTime),
                    // Only the app that wrote THIS session. Without the filter the aggregate sums
                    // every provider that covered the window, so a user running Samsung Health and
                    // Google Fit side by side saw one 5 km run reported as nearly 10 km — and that
                    // inflated figure is what the import card offers and then writes back.
                    dataOriginFilter = setOf(metadata.dataOrigin)
                )
            )
        }
        return WatchWorkout(
            recordId = metadata.id,
            startMs = start,
            endMs = end,
            exerciseType = exerciseType,
            title = title,
            distanceKm = aggregate?.get(DistanceRecord.DISTANCE_TOTAL)?.inKilometers?.takeIf { it > 0.0 },
            kcal = aggregate?.get(TotalCaloriesBurnedRecord.ENERGY_TOTAL)?.inKilocalories?.takeIf { it > 0.0 }
        )
    }

    /**
     * The watch session that best matches a cardio entry on its day, resolved for route display.
     *
     * - [route] non-null → that session already exposes its GPS track (≥2 points) and it can be drawn
     *   straight away.
     * - [route] null but [recordId] set → a matching session exists with a route, but Health Connect
     *   requires per-session consent first; the UI launches [ExerciseRouteRequestContract] with this id.
     *
     * Returns null when there's no provider / no exercise permission / no close-enough session / the
     * matched session has no route at all (NoData) — in every case the caller shows no route UI.
     */
    data class SessionRouteMatch(val recordId: String, val route: List<RoutePoint>?)

    suspend fun matchSessionRoute(
        entryStartMs: Long,
        entryDurationMin: Int,
        dayStartMs: Long,
        dayEndMs: Long
    ): SessionRouteMatch? = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext null
        if (!canReadExercise()) return@withContext null
        hcCatching {
            val range = TimeRangeFilter.between(Instant.ofEpochMilli(dayStartMs), Instant.ofEpochMilli(dayEndMs))
            val records = client.readRecords(ReadRecordsRequest(ExerciseSessionRecord::class, timeRangeFilter = range))
                .records.filterNot(::isSelfWritten)
            val windows = records.mapIndexed { i, r ->
                SessionWindow(index = i, startMs = r.startTime.toEpochMilli(), endMs = r.endTime.toEpochMilli())
            }
            val idx = bestSessionMatch(entryStartMs, entryDurationMin, windows) ?: return@hcCatching null
            val record = records[idx]
            when (val result = record.exerciseRouteResult) {
                is ExerciseRouteResult.Data ->
                    routePoints(result.exerciseRoute).takeIf { it.size >= 2 }
                        ?.let { SessionRouteMatch(record.metadata.id, it) }
                is ExerciseRouteResult.ConsentRequired -> SessionRouteMatch(record.metadata.id, null)
                else -> null // NoData (or any future result) → no route to offer.
            }
        }
    }

    /** Flatten a Health Connect [ExerciseRoute] to Avex's Android-free [RoutePoint]s (lat/lng only). */
    fun routePoints(route: ExerciseRoute?): List<RoutePoint> =
        route?.route?.map { RoutePoint(lat = it.latitude, lng = it.longitude) }.orEmpty()

    /**
     * Per-signal "is data actually arriving" flags — the honest check the Recovery page renders as
     * "receiving" vs "nothing yet". A signal that's GRANTED but has no record in the recent window
     * is almost always the companion app's Health Connect sharing being off, not the app being
     * broken — so this is what lets the UI tell those apart. Only the READ signals appear; the
     * calorie WRITE has no inbound data to probe.
     */
    data class SignalFlow(
        val sleepOrHr: Boolean,
        val weight: Boolean,
        val steps: Boolean,
        /** A recent watch session actually carried a GPS route — not merely that sessions exist, so the
         *  "GPS routes" row's reading reflects routes rather than any workout syncing. */
        val route: Boolean
    ) {
        companion object { val NONE = SignalFlow(false, false, false, false) }
    }

    /**
     * Probe whether each granted READ signal has any record in the last [FLOW_WINDOW_DAYS] — the
     * "data is reaching Avex" reading for the Recovery rows. Reads a single row per type (cheap) and
     * only for types the user has granted (an ungranted read would throw). Fail-soft: no provider /
     * a read error → all false.
     *
     * A `false` is deliberately AMBIGUOUS — the watch hasn't synced yet, the activity wasn't done
     * this month, or the companion app isn't sharing — so the caller renders it as "nothing yet",
     * NEVER "unsupported". We can only observe presence through Health Connect, never capability.
     */
    suspend fun probeSignalFlow(nowMs: Long): SignalFlow = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext SignalFlow.NONE
        val granted = grantedPermissions()
        val startMs = nowMs - FLOW_WINDOW_DAYS * 24 * 60 * 60 * 1000L
        val range = TimeRangeFilter.between(Instant.ofEpochMilli(startMs), Instant.ofEpochMilli(nowMs))
        // Existence check for one granted type: read a single row in the window, true if any exists.
        suspend fun <T : Record> hasAny(type: KClass<T>): Boolean {
            if (HealthPermission.getReadPermission(type) !in granted) return false
            return hcCatching {
                client.readRecords(ReadRecordsRequest(type, timeRangeFilter = range, pageSize = 1)).records.isNotEmpty()
            } ?: false
        }
        // The GPS-routes row asks whether ROUTES are arriving, not merely sessions — an indoor-only
        // logger has sessions but no routes. Scan the most recent sessions for one that carries a track
        // (Data = points in hand, ConsentRequired = a route exists behind a per-session consent prompt).
        suspend fun hasAnyRoute(): Boolean {
            if (HealthPermission.getReadPermission(ExerciseSessionRecord::class) !in granted) return false
            return hcCatching {
                client.readRecords(
                    ReadRecordsRequest(
                        ExerciseSessionRecord::class, timeRangeFilter = range,
                        ascendingOrder = false, pageSize = FLOW_ROUTE_PROBE_SESSIONS
                    )
                ).records.filterNot(::isSelfWritten).any {
                    it.exerciseRouteResult is ExerciseRouteResult.Data ||
                        it.exerciseRouteResult is ExerciseRouteResult.ConsentRequired
                }
            } ?: false
        }
        // The probes are independent — fan them out so the Recovery page's reading costs one round-trip,
        // not five in series. The Sleep & heart-rate row bundles two types ("receiving" if either arrives).
        coroutineScope {
            val sleep = async { hasAny(SleepSessionRecord::class) }
            val hr = async { hasAny(RestingHeartRateRecord::class) }
            val weight = async { hasAny(WeightRecord::class) }
            val steps = async { hasAny(StepsRecord::class) }
            val route = async { hasAnyRoute() }
            SignalFlow(
                sleepOrHr = sleep.await() || hr.await(),
                weight = weight.await(),
                steps = steps.await(),
                route = route.await()
            )
        }
    }

    /**
     * The most recent valid sleep session before [nowMs] as a standalone data point — gated on the
     * sleep read permission alone (independent of resting-HR), fail-soft to null. Distinct from
     * [readRecovery], which bundles a window of nights for the coach; this exposes just the latest.
     */
    suspend fun latestSleep(nowMs: Long): SleepNight? = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext null
        if (!grantedPermissions().contains(HealthPermission.getReadPermission(SleepSessionRecord::class))) return@withContext null
        hcCatching {
            client.readRecords(
                ReadRecordsRequest(
                    SleepSessionRecord::class,
                    timeRangeFilter = TimeRangeFilter.before(Instant.ofEpochMilli(nowMs)),
                    ascendingOrder = false,
                    pageSize = 1
                )
            ).records.firstOrNull()?.let {
                val min = Duration.between(it.startTime, it.endTime).toMinutes()
                if (min <= 0) null else SleepNight(
                    endedAtMs = it.endTime.toEpochMilli(),
                    durationMin = min.coerceAtMost(MAX_SLEEP_MIN).toInt()
                )
            }
        }
    }

    /**
     * The most recent physiologically valid resting-HR reading before [nowMs] as a standalone data
     * point — gated on the resting-HR read permission alone, fail-soft to null.
     */
    suspend fun latestRestingHr(nowMs: Long): RestingHrSample? = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext null
        if (!grantedPermissions().contains(HealthPermission.getReadPermission(RestingHeartRateRecord::class))) return@withContext null
        hcCatching {
            client.readRecords(
                ReadRecordsRequest(
                    RestingHeartRateRecord::class,
                    timeRangeFilter = TimeRangeFilter.before(Instant.ofEpochMilli(nowMs)),
                    ascendingOrder = false,
                    pageSize = 1
                )
            ).records.firstOrNull()?.let {
                val bpm = it.beatsPerMinute.toInt()
                if (bpm in MIN_BPM..MAX_BPM) RestingHrSample(timeMs = it.time.toEpochMilli(), bpm = bpm) else null
            }
        }
    }

    private companion object {
        /** Cap a single sleep record at 16h so a stuck/forgotten wearable session can't read as great rest. */
        const val MAX_SLEEP_MIN = 16L * 60
        const val MIN_BPM = 20
        const val MAX_BPM = 240
        /** How far back the [probeSignalFlow] "is data arriving" check looks. Wide enough that a user
         *  who synced anytime this month reads as "receiving"; a granted-but-silent signal (sharing
         *  off) reads as "nothing yet". */
        const val FLOW_WINDOW_DAYS = 30L
        /** How many recent exercise sessions [probeSignalFlow] scans for a GPS route before giving up —
         *  bounded so the route reading stays a cheap read even for a heavy logger. */
        const val FLOW_ROUTE_PROBE_SESSIONS = 50
        /** Page size + hard cap for the [readWeightHistory] backfill. One-per-day weigh-ins over many
         *  years stay well under the cap; the cap just guards against a pathological history. */
        const val HISTORY_PAGE_SIZE = 1000
        const val HISTORY_MAX_RECORDS = 5000
        /** Pages [recentWatchWorkouts] will walk looking for non-self records before giving up. */
        const val WATCH_WORKOUT_MAX_PAGES = 5
        /** Cap on flattened HR samples per read (W5) — ~3h at 1Hz; charts downsample far below this. */
        const val HR_SERIES_MAX_SAMPLES = 12_000
        /** RMSSD sanity ceiling (W6) — real overnight values sit ~15–150ms; 500+ is a corrupt row. */
        const val MAX_RMSSD_MS = 500.0
    }
}
