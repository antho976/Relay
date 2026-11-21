package com.forge.app.debug

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.records.HeartRateVariabilityRmssdRecord
import androidx.health.connect.client.records.Record
import androidx.health.connect.client.records.RestingHeartRateRecord
import androidx.health.connect.client.records.SleepSessionRecord
import androidx.health.connect.client.records.StepsRecord
import androidx.health.connect.client.records.metadata.Device
import androidx.health.connect.client.records.metadata.Metadata
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.time.ZoneId
import kotlin.random.Random

/**
 * Debug-only: writes two weeks of watch-like sleep, resting HR, HRV and steps into Health Connect,
 * so the BODY surfaces can be reviewed on an emulator with no watch. Grant the debug build the
 * WRITE_ and READ_ health permissions first (see the debug manifest), then:
 *
 * `adb shell am broadcast -a com.forge.app.debug.SEED_HEALTH -n com.quietsoftware.avex.debug/com.forge.app.debug.SeedHealthDataReceiver`
 */
class SeedHealthDataReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACTION) return
        val pending = goAsync()
        CoroutineScope(Dispatchers.IO).launch {
            try {
                HealthConnectClient.getOrCreate(context).insertRecords(records())
                Log.i(TAG, "Health data seeded")
            } catch (t: Throwable) {
                Log.e(TAG, "Health seeding failed", t)
            } finally {
                pending.finish()
            }
        }
    }

    private fun records(): List<Record> {
        val zone = ZoneId.systemDefault()
        val now = java.time.Instant.now()
        val today = LocalDate.now(zone)
        val rng = Random(7)
        val watch = Device(type = Device.TYPE_WATCH, manufacturer = "Debug", model = "Seed watch")
        fun meta() = Metadata.autoRecorded(device = watch)
        val out = mutableListOf<Record>()
        for (daysAgo in 14 downTo 0) {
            val day = today.minusDays(daysAgo.toLong())
            val offset = zone.rules.getOffset(day.atStartOfDay(zone).toInstant())
            // The night that ends on this morning; last night runs short so the compare line shows.
            val sleepMin = if (daysAgo == 0) 395 else 430 + rng.nextInt(-25, 30)
            val wake = day.atTime(6, 45).atZone(zone).toInstant()
            out += SleepSessionRecord(
                startTime = wake.minusSeconds(sleepMin * 60L), startZoneOffset = offset,
                endTime = wake, endZoneOffset = offset, metadata = meta()
            )
            val morning = day.atTime(7, 0).atZone(zone).toInstant()
            out += RestingHeartRateRecord(
                time = morning, zoneOffset = offset,
                beatsPerMinute = if (daysAgo == 0) 57 else 52L + rng.nextInt(-2, 3), metadata = meta()
            )
            out += HeartRateVariabilityRmssdRecord(
                time = morning, zoneOffset = offset,
                heartRateVariabilityMillis = if (daysAgo == 0) 49.0 else 62.0 + rng.nextInt(-5, 6),
                metadata = meta()
            )
            // Steps in hourly blocks from 8:00, up to now on today.
            val total = if (daysAgo == 0) 6_240 else 8_000 + rng.nextInt(-2_000, 2_500)
            val hours = 12
            for (h in 0 until hours) {
                val start = day.atTime(8 + h, 0).atZone(zone).toInstant()
                val end = start.plusSeconds(3_600)
                if (end.isAfter(now)) break
                out += StepsRecord(
                    startTime = start, startZoneOffset = offset, endTime = end, endZoneOffset = offset,
                    count = (total / hours).toLong(), metadata = meta()
                )
            }
        }
        return out
    }

    private companion object {
        const val ACTION = "com.forge.app.debug.SEED_HEALTH"
        const val TAG = "AvexSeed"
    }
}
