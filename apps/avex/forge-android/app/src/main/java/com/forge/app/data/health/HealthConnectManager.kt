package com.forge.app.data.health

import android.content.Context
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.RestingHeartRateRecord
import androidx.health.connect.client.records.SleepSessionRecord
import androidx.health.connect.client.request.ReadRecordsRequest
import androidx.health.connect.client.time.TimeRangeFilter
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

    /** Permissions the user has already granted (empty when HC is absent or unreadable). */
    suspend fun grantedPermissions(): Set<String> =
        runCatching { clientOrNull()?.permissionController?.getGrantedPermissions() }.getOrNull().orEmpty()

    /** True only when every recovery permission is granted. */
    suspend fun hasAllPermissions(): Boolean = grantedPermissions().containsAll(permissions)

    /**
     * Read sleep + resting-HR records in `[startMs, nowMs]` into a pure [HealthSnap]. Returns empty
     * on any failure (unavailable / not granted / provider error) — recovery signals are additive,
     * never load-bearing, so a miss just means the coach leans on its on-app signals instead.
     */
    suspend fun readRecovery(startMs: Long, nowMs: Long): HealthSnap = withContext(Dispatchers.IO) {
        val client = clientOrNull() ?: return@withContext HealthSnap()
        if (!hasAllPermissions()) return@withContext HealthSnap()
        runCatching {
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
        }.getOrElse { HealthSnap() }
    }

    private companion object {
        /** Cap a single sleep record at 16h so a stuck/forgotten wearable session can't read as great rest. */
        const val MAX_SLEEP_MIN = 16L * 60
        const val MIN_BPM = 20
        const val MAX_BPM = 240
    }
}
