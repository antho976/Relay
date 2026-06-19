package com.forge.app.data.health

import android.content.Context
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.ActiveCaloriesBurnedRecord
import androidx.health.connect.client.records.RestingHeartRateRecord
import androidx.health.connect.client.records.SleepSessionRecord
import androidx.health.connect.client.records.WeightRecord
import androidx.health.connect.client.records.metadata.Metadata
import androidx.health.connect.client.request.ReadRecordsRequest
import androidx.health.connect.client.time.TimeRangeFilter
import androidx.health.connect.client.units.Energy
import androidx.health.connect.client.units.Mass
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.RestingHrSample
import com.forge.app.domain.adapt.SleepNight
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.time.Duration
import java.time.Instant
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Forge's single touchpoint with Health Connect — the only external data source the app reads.
 * Everything here is gated and fail-soft: if HC isn't installed, the user hasn't granted access,
 * or a read throws, we return an empty [HealthSnap] and the coach behaves exactly as it does with
 * no health data at all (the recovery drivers are purely additive).
 *
 * No INTERNET permission is involved: [HealthConnectClient] talks to the on-device Health Connect
 * app over IPC, so connecting it does NOT change Forge's "nothing leaves the device" stance.
 */
@Singleton
class HealthConnectManager @Inject constructor(
    @ApplicationContext private val context: Context
) {
    /** The read-only recovery permissions Forge requests (sleep + resting heart rate). */
    val permissions: Set<String> = setOf(
        HealthPermission.getReadPermission(SleepSessionRecord::class),
        HealthPermission.getReadPermission(RestingHeartRateRecord::class)
    )

    /**
     * Bodyweight permissions (HC-2/HC-3) — read so a smart-scale value can flow INTO Forge, write so
     * a Forge weigh-in can flow BACK to Health Connect. Kept as a SEPARATE set from [permissions] so
     * each integration is independently opt-in: connecting recovery never silently asks for weight,
     * and vice-versa.
     */
    val weightPermissions: Set<String> = setOf(
        HealthPermission.getReadPermission(WeightRecord::class),
        HealthPermission.getWritePermission(WeightRecord::class)
    )

    /**
     * Active-calorie permission (HC-4) — write only, so a Forge session can flow OUT to Health
     * Connect's daily energy total. Its own set, like [weightPermissions]: enabling calorie sync
     * never silently asks for sleep/HR or weight, and vice-versa.
     */
    val caloriePermissions: Set<String> = setOf(
        HealthPermission.getWritePermission(ActiveCaloriesBurnedRecord::class)
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

    /** True when Forge may READ bodyweight from Health Connect (HC-2). */
    suspend fun canReadWeight(): Boolean =
        grantedPermissions().contains(HealthPermission.getReadPermission(WeightRecord::class))

    /** True when Forge may WRITE bodyweight back to Health Connect (HC-3). */
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

    /** True when Forge may WRITE active calories to Health Connect (HC-4). */
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

    private companion object {
        /** Cap a single sleep record at 16h so a stuck/forgotten wearable session can't read as great rest. */
        const val MAX_SLEEP_MIN = 16L * 60
        const val MIN_BPM = 20
        const val MAX_BPM = 240
    }
}
