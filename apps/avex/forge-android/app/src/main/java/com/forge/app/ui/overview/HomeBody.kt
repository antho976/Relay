package com.forge.app.ui.overview

import com.forge.app.domain.adapt.HealthSnap
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The readings Home can put in its two slots (2026-10-21 redesign). The user picks one per slot from
 * the slot sheet; [id] is what [com.forge.app.data.prefs.SettingsRepository.homeSlots] stores, so it
 * never changes once shipped.
 */
enum class HomeSlot(val id: String, val label: String, val pickerTitle: String, val needsWatch: Boolean) {
    SLEEP("sleep", "Sleep", "Sleep", true),
    RESTING_HR("resting_hr", "Resting HR", "Resting heart rate", true),
    HRV("hrv", "HRV", "HRV", true),
    STEPS("steps", "Steps", "Steps today", true),
    BODY_WEIGHT("body_weight", "Body weight", "Body weight", false),
    LAST_SESSION("last_session", "Last session", "Last session", false),
    WEEK_VOLUME("week_volume", "This week", "Volume this week", false);

    companion object {
        val DEFAULT_LEFT = SLEEP
        val DEFAULT_RIGHT = LAST_SESSION
        fun of(id: String?): HomeSlot? = entries.firstOrNull { it.id == id }

        /** What stands in for a watch reading while no watch is connected, in order of preference. */
        val NO_WATCH_STAND_INS = listOf(WEEK_VOLUME, LAST_SESSION, BODY_WEIGHT)
    }
}

/**
 * One reading as the redesign draws it: mono label → serif [figure] with its [unit] at half size →
 * a mark ([spark] line or [bars]) → mono [change] line.
 */
data class SlotReading(
    val figure: String,
    /** Muted, half size, straight after the figure: "h", " bpm", "×5 squat". */
    val unit: String = "",
    /** Oldest → newest, drawn as the red sparkline. Fewer than two points draws nothing. */
    val spark: List<Float> = emptyList(),
    /** "Each lift vs last time" bars, for the last-session reading. */
    val bars: List<Float?>? = null,
    val change: String? = null,
    /** True when the change line says something moved, so it reads in ink rather than muted. */
    val changeMoved: Boolean = false,
    /** Overrides the slot's own label: "Yesterday" or "Mon" for the last session. */
    val label: String? = null,
    /** The short value the slot sheet prints beside the option ("7.4h"). */
    val picker: String = figure + unit,
    val description: String,
)

/**
 * What the watch can feed Home. [Invite] when Health Connect is on the phone but nothing is granted;
 * [Readings] once connected, with a null reading where the watch has not reported it.
 */
sealed interface HomeBody {
    data object Invite : HomeBody

    data class Readings(
        val sleep: SlotReading?,
        val restingHr: SlotReading?,
        val hrv: SlotReading?,
        val steps: SlotReading?,
    ) : HomeBody {
        val isEmpty: Boolean get() = sleep == null && restingHr == null && hrv == null && steps == null
    }
}

/** How many days the sparklines span. */
private const val SPARK_DAYS = 7

/**
 * Builds the watch readings from a 15-day [snap]. "Usual" is the median of the days before today,
 * needing three of them; with fewer there is no comparison line rather than a made-up one. A reading
 * older than yesterday is not shown as today's.
 */
fun homeBodyReadings(snap: HealthSnap, todayStartMs: Long, zone: ZoneId = ZoneId.systemDefault()): HomeBody.Readings {
    fun dayOf(ms: Long): LocalDate = Instant.ofEpochMilli(ms).atZone(zone).toLocalDate()

    // Sleep, one total per morning: a night split in two by the watch is still one night's sleep.
    val nights = snap.sleepNights.groupBy { dayOf(it.endedAtMs) }
        .mapValues { (_, n) -> n.sumOf { it.durationMin } }
        .toSortedMap()
    val todayDate = dayOf(todayStartMs)
    val lastNight = nights[todayDate]
    val usualSleep = median(nights.filterKeys { it < todayDate }.values.toList())
    val sleep = lastNight?.let { min ->
        val hours = min / 60.0
        val delta = usualSleep?.let { (min - it) / 60.0 }
        val change = delta?.let {
            if (abs(it) < 0.25) "About your usual"
            else "${arrow(it)} ${"%.1f".format(abs(it))}h vs usual"
        }
        SlotReading(
            figure = "%.1f".format(hours),
            unit = "h",
            spark = nights.values.toList().takeLast(SPARK_DAYS).map { it / 60f },
            change = change,
            changeMoved = delta != null && abs(delta) >= 0.25,
            description = "Slept ${formatMinutes(min)}" + (change?.let { ", ${it.lowercase()}" } ?: ""),
        )
    }

    val hrByDay = snap.restingHr.groupBy { dayOf(it.timeMs) }.mapValues { (_, s) -> s.maxBy { it.timeMs }.bpm }.toSortedMap()
    val latestHr = snap.restingHr.maxByOrNull { it.timeMs }?.takeIf { it.timeMs >= todayStartMs - DAY_MS }
    val usualHr = median(snap.restingHr.filter { it.timeMs < todayStartMs }.map { it.bpm })
    val restingHr = latestHr?.let { s ->
        val delta = usualHr?.let { s.bpm - it }
        val change = delta?.let { if (abs(it) <= 2) "Normal for you" else "${arrow(it.toDouble())} ${abs(it)} vs usual" }
        SlotReading(
            figure = "${s.bpm}",
            unit = " bpm",
            spark = hrByDay.values.toList().takeLast(SPARK_DAYS).map { it.toFloat() },
            change = change,
            changeMoved = delta != null && abs(delta) > 2,
            description = "Resting heart rate ${s.bpm} beats per minute" + (change?.let { ", ${it.lowercase()}" } ?: ""),
        )
    }

    val hrvByDay = snap.hrv.groupBy { dayOf(it.timeMs) }.mapValues { (_, s) -> s.maxBy { it.timeMs }.rmssdMs }.toSortedMap()
    val latestHrv = snap.hrv.maxByOrNull { it.timeMs }?.takeIf { it.timeMs >= todayStartMs - DAY_MS }
    val usualHrv = median(snap.hrv.filter { it.timeMs < todayStartMs }.map { it.rmssdMs.roundToInt() })
    val hrv = latestHrv?.let { s ->
        val ms = s.rmssdMs.roundToInt()
        val pct = usualHrv?.takeIf { it > 0 }?.let { ((ms - it) * 100.0 / it).roundToInt() }
        val change = pct?.let { if (abs(it) < 10) "Normal for you" else "${arrow(it.toDouble())} ${abs(it)}% vs usual" }
        SlotReading(
            figure = "$ms",
            unit = " ms",
            spark = hrvByDay.values.toList().takeLast(SPARK_DAYS).map { it.toFloat() },
            change = change,
            changeMoved = pct != null && abs(pct) >= 10,
            description = "Heart rate variability $ms milliseconds" + (change?.let { ", ${it.lowercase()}" } ?: ""),
        )
    }

    val todaySteps = snap.dailySteps.lastOrNull { it.dayStartMs == todayStartMs }
    val typicalSteps = median(snap.dailySteps.filter { it.dayStartMs < todayStartMs && it.steps > 0 }.map { it.steps })
    val steps = if (todaySteps == null && snap.dailySteps.isEmpty()) null else {
        val count = todaySteps?.steps ?: 0
        val change = typicalSteps?.let { "Typical ${formatCount(it)}" }
        SlotReading(
            figure = "%,d".format(count),
            spark = snap.dailySteps.sortedBy { it.dayStartMs }.takeLast(SPARK_DAYS).map { it.steps.toFloat() },
            change = change,
            description = "$count steps today" + (typicalSteps?.let { ", typical $it" } ?: ""),
        )
    }

    return HomeBody.Readings(sleep = sleep, restingHr = restingHr, hrv = hrv, steps = steps)
}

private fun arrow(delta: Double) = if (delta < 0) "↓" else "↑"

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
