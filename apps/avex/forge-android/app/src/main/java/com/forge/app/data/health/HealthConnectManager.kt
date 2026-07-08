package com.forge.app.data.health

import android.content.Context
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.ActiveCaloriesBurnedRecord
import androidx.health.connect.client.records.ExerciseRoute
import androidx.health.connect.client.records.ExerciseRouteResult
import androidx.health.connect.client.records.ExerciseSessionRecord
import androidx.health.connect.client.records.RestingHeartRateRecord
import androidx.health.connect.client.records.SleepSessionRecord
import androidx.health.connect.client.records.StepsRecord
import androidx.health.connect.client.records.WeightRecord
import androidx.health.connect.client.records.metadata.Metadata
import androidx.health.connect.client.request.ReadRecordsRequest
import androidx.health.connect.client.time.TimeRangeFilter
import androidx.health.connect.client.units.Energy
import androidx.health.connect.client.units.Mass
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.RestingHrSample
import com.forge.app.domain.adapt.SleepNight
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.RoutePoint
import com.forge.app.domain.health.SessionWindow
import com.forge.app.domain.health.StepSample
import com.forge.app.domain.health.bestSessionMatch
import com.forge.app.domain.health.bucketStepsByHour
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.time.Duration
import java.time.Instant
import javax.inject.Inject
import javax.inject.Singleton

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

    /** True when Avex may WRITE active calories to Health Connect (HC-4). */
    suspend fun canWriteActiveCalories(): Boolean =
        grantedPermissions().contains(HealthPermission.getWritePermission(ActiveCaloriesBurnedRecord::class))

    /**
     * Write one finished session's estimated active calories to Health Connect over `[startMs, endMs]`,
     * returning whether it landed. Best-effort and fail-soft like [writeWeight]: no provider / no write
     * permission / a provider error all return false without throwing, so a failed mirror never breaks
     * the local finish. The span is clamped to be strictly positive — HC rejects a zero/negative range.
     */
    suspend fun writeActiveCalories(kcal: Double, startMs: Long, endMs: Long): Boolean = withContext(Dispatchers.IO) {
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
                        metadata = Metadata.manualEntry()
                    )
                )
            )
            true
        } ?: false
    }

    /**
     * Read sleep + resting-HR records in `[startMs, nowMs]` into a pure [HealthSnap]. Returns empty
     * on any failure (unavailable / not granted / provider error) — recovery signals are additive,
     * never load-bearing, so a miss just means the coach leans on its on-app signals instead.
     */
    suspend fun readRecovery(startMs: Long, nowMs: Long): HealthSnap = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext HealthSnap()
        if (!hasAllPermissions()) return@withContext HealthSnap()
        hcCatching {
            val range = TimeRangeFilter.between(Instant.ofEpochMilli(startMs), Instant.ofEpochMilli(nowMs))
            val sleep = client.readRecords(ReadRecordsRequest(SleepSessionRecord::class, timeRangeFilter = range))
                .records.mapNotNull {
                    // Drop corrupt records (a third-party app can write endTime <= startTime) and cap
                    // absurd spans (a forgotten wearable can log a multi-day "night") so one bad row
                    // can't skew DeloadAdvisor's sleep average up or down.
                    val min = Duration.between(it.startTime, it.endTime).toMinutes()
                    if (min <= 0) null else SleepNight(
                        endedAtMs = it.endTime.toEpochMilli(),
                        durationMin = min.coerceAtMost(MAX_SLEEP_MIN).toInt()
                    )
                }
            val hr = client.readRecords(ReadRecordsRequest(RestingHeartRateRecord::class, timeRangeFilter = range))
                .records.mapNotNull {
                    // Ignore physiologically impossible readings (0 bpm corrupt rows would distort the baseline).
                    val bpm = it.beatsPerMinute.toInt()
                    if (bpm in MIN_BPM..MAX_BPM) RestingHrSample(timeMs = it.time.toEpochMilli(), bpm = bpm) else null
                }
            HealthSnap(sleepNights = sleep, restingHr = hr)
        } ?: HealthSnap()
    }

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
            val samples = client.readRecords(ReadRecordsRequest(StepsRecord::class, timeRangeFilter = range))
                .records.map { StepSample(startMs = it.startTime.toEpochMilli(), count = it.count) }
            CardioWearableDay(hourlySteps = bucketStepsByHour(samples, java.time.ZoneId.systemDefault()))
        } ?: CardioWearableDay()
    }

    /** True when Avex may READ exercise sessions from Health Connect. */
    suspend fun canReadExercise(): Boolean =
        grantedPermissions().contains(HealthPermission.getReadPermission(ExerciseSessionRecord::class))

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
            val records = client.readRecords(ReadRecordsRequest(ExerciseSessionRecord::class, timeRangeFilter = range)).records
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
    }
}
