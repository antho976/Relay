package com.forge.app.ui.overview

import com.forge.app.domain.adapt.HealthSnap
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * What Home's BODY section shows (2026-10-16). The watch used to surface on Home only as a steps
 * line that vanished without a grant, so a user with a watch had nothing telling them it was
 * connected, and a user without one had nothing telling them it could be.
 */
sealed interface HomeBody {
    /** Health Connect is on the phone but nothing is granted yet: Home offers to connect. */
    data object Invite : HomeBody

    /** Connected. Each reading is null when the watch has not reported it. */
    data class Readings(
        val sleep: Reading?,
        val restingHr: Reading?,
        val hrv: Reading?,
        val steps: Reading?
    ) : HomeBody {
        val isEmpty: Boolean get() = sleep == null && restingHr == null && hrv == null && steps == null
    }

    /** One tile: the figure and a short line comparing it to the user's own usual. */
    data class Reading(val value: String, val compare: String?, val description: String)
}

/**
 * Builds the readings from a 15-day [snap]. "Usual" is the median of the days before today, needing
 * three of them; with fewer there is no comparison line rather than a made-up one.
 */
fun homeBodyReadings(snap: HealthSnap, todayStartMs: Long): HomeBody.Readings {
    // Last night: the newest sleep that ended today (a nap after noon still counts as today's).
    val nights = snap.sleepNights.sortedBy { it.endedAtMs }
    val lastNight = nights.lastOrNull { it.endedAtMs >= todayStartMs }
    val usualSleep = median(nights.filter { it.endedAtMs < todayStartMs }.map { it.durationMin })
    val sleep = lastNight?.let { night ->
        val compare = usualSleep?.let { usual ->
            val delta = night.durationMin - usual
            when {
                abs(delta) < 15 -> "About your usual"
                delta < 0 -> "${formatMinutes(-delta)} under usual"
                else -> "${formatMinutes(delta)} over usual"
            }
        }
        HomeBody.Reading(
            value = formatMinutes(night.durationMin),
            compare = compare,
            description = "Slept ${formatMinutes(night.durationMin)}" + (compare?.let { ", ${it.lowercase()}" } ?: "")
        )
    }

    val latestHr = snap.restingHr.maxByOrNull { it.timeMs }?.takeIf { it.timeMs >= todayStartMs - DAY_MS }
    val usualHr = median(snap.restingHr.filter { it.timeMs < todayStartMs }.map { it.bpm })
    val restingHr = latestHr?.let { s ->
        val compare = usualHr?.let { usual ->
            val delta = s.bpm - usual
            when {
                abs(delta) <= 2 -> "Normal for you"
                delta > 0 -> "$delta over usual"
                else -> "${-delta} under usual"
            }
        }
        HomeBody.Reading(
            value = "${s.bpm} bpm",
            compare = compare,
            description = "Resting heart rate ${s.bpm} beats per minute" + (compare?.let { ", ${it.lowercase()}" } ?: "")
        )
    }

    val latestHrv = snap.hrv.maxByOrNull { it.timeMs }?.takeIf { it.timeMs >= todayStartMs - DAY_MS }
    val usualHrv = median(snap.hrv.filter { it.timeMs < todayStartMs }.map { it.rmssdMs.roundToInt() })
    val hrv = latestHrv?.let { s ->
        val ms = s.rmssdMs.roundToInt()
        val compare = usualHrv?.takeIf { it > 0 }?.let { usual ->
            val pct = ((ms - usual) * 100.0 / usual).roundToInt()
            when {
                abs(pct) < 10 -> "Normal for you"
                pct < 0 -> "${-pct}% under usual"
                else -> "$pct% over usual"
            }
        }
        HomeBody.Reading(
            value = "$ms ms",
            compare = compare,
            description = "Heart rate variability $ms milliseconds" + (compare?.let { ", ${it.lowercase()}" } ?: "")
        )
    }

    val todaySteps = snap.dailySteps.lastOrNull { it.dayStartMs == todayStartMs }
    val typicalSteps = median(snap.dailySteps.filter { it.dayStartMs < todayStartMs && it.steps > 0 }.map { it.steps })
    val steps = if (todaySteps == null && snap.dailySteps.isEmpty()) null else {
        val count = todaySteps?.steps ?: 0
        val compare = typicalSteps?.let { "Typical ${formatCount(it)}" }
        HomeBody.Reading(
            value = "%,d".format(count),
            compare = compare,
            description = "$count steps today" + (typicalSteps?.let { ", typical $it" } ?: "")
        )
    }

    return HomeBody.Readings(sleep = sleep, restingHr = restingHr, hrv = hrv, steps = steps)
}

private fun median(values: List<Int>): Int? {
    if (values.size < 3) return null
    val sorted = values.sorted()
    return sorted[sorted.size / 2]
}

/** 434 → "7h 14m", 45 → "45m". */
internal fun formatMinutes(min: Int): String {
    val h = min / 60
    val m = min % 60
    return when {
        h == 0 -> "${m}m"
        m == 0 -> "${h}h"
        else -> "${h}h ${m}m"
    }
}

/** 8_140 → "8.1k", 950 → "950". */
private fun formatCount(n: Int): String =
    if (n >= 1_000) "%.1fk".format(n / 1_000.0) else n.toString()

private const val DAY_MS = 24L * 60 * 60 * 1000
