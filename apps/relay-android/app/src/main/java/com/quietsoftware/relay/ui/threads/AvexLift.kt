package com.quietsoftware.relay.ui.threads

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.LocalDate
import java.time.temporal.ChronoUnit
import kotlin.math.pow

/** What "est. max" is, wherever it shows (avex_pages.rs `EST_MAX`). */
private const val EST_MAX = "Est. max is an estimated one-rep max: weight × (1 + reps ÷ 30), from sets of 1 to 12 reps"

/** Days without a lift, with no change to read, before it counts as slipping (avex_pages.rs `DROPPED_DAYS`). */
private const val DROPPED_DAYS = 42L

internal enum class LiftTrend { Up, Holding, Slipping }

/** Whole days from one `YYYY-MM-DD` to another; null when either does not read. */
private fun daysFrom(from: String?, to: String?): Long? {
    val a = dayOf(from) ?: return null
    val b = dayOf(to) ?: return null
    return ChronoUnit.DAYS.between(a, b)
}

/**
 * Up or down more than 1% on the four weeks before, else holding; a lift with nothing to compare
 * that has not been done in six weeks is slipping too (avex_pages.rs `trend`). [asOf] is the day
 * the copy is read to.
 */
internal fun liftTrend(l: JsonObject, asOf: String): LiftTrend {
    val change = l.d("change_pct")
    return when {
        change != null && change > 1 -> LiftTrend.Up
        change != null && change < -1 -> LiftTrend.Slipping
        change != null -> LiftTrend.Holding
        (daysFrom(l.s("last_day"), asOf) ?: 0L) > DROPPED_DAYS -> LiftTrend.Slipping
        else -> LiftTrend.Holding
    }
}

/** A point of a lift's estimated max: its day, the figure, and whether it was a record. */
private class LiftPoint(val day: String, val e1rm: Double, val record: Boolean)

/**
 * The least-squares slope of the estimated max over the eight weeks before the last point, a week
 * at a time; null with fewer than three points there (avex_pages.rs `weekly_slope`).
 */
private fun weeklySlope(points: List<LiftPoint>): Double? {
    val last = points.lastOrNull()?.day ?: return null
    val xy = points.mapNotNull { p -> daysFrom(p.day, last)?.let { -it.toDouble() to p.e1rm } }.filter { it.first >= -56.0 }
    if (xy.size < 3) return null
    val n = xy.size.toDouble()
    val mx = xy.sumOf { it.first } / n
    val my = xy.sumOf { it.second } / n
    val sxx = xy.sumOf { (it.first - mx).pow(2) }
    val sxy = xy.sumOf { (it.first - mx) * (it.second - my) }
    return if (sxx > 0) sxy / sxx * 7 else null
}

/** `+10 lb`, `−2.5 kg`. */
private fun signedWeight(d: Double, unit: String) = if (d < 0) "−${weight(-d)} $unit" else "+${weight(d)} $unit"

/**
 * What the lift has done, in a sentence or two (avex_pages.rs `trend_sentence`): "Up 4.9% in four
 * weeks, about +2 lb a week. Last record Mon 5 Oct."
 */
private fun trendSentence(row: JsonObject, points: List<LiftPoint>, unit: String, asOf: String): String {
    val lastDay = row.s("last_day")
    val change = row.d("change_pct")
    val first = when {
        change != null && change > 1 -> "Up ${String.format(java.util.Locale.ROOT, "%.1f", change)}% in four weeks"
        change != null && change < -1 -> "Down ${String.format(java.util.Locale.ROOT, "%.1f", -change)}% in four weeks"
        change != null -> "Holding steady over four weeks"
        row.o("best") == null -> return row.l("best_reps")?.let { "Done without weight: at best ${plural(maxOf(it, 0L), "rep")} in one set." }
            ?: "No set to read an estimated max from yet."
        (daysFrom(lastDay, asOf) ?: 0L) > 28 -> "Not done in the last four weeks; last ${humanDate(lastDay)}"
        else -> "Not enough weeks yet to compare"
    }
    val out = StringBuilder(first)
    weeklySlope(points)?.let { Math.round(it * 2) / 2.0 }?.takeIf { it != 0.0 }?.let { out.append(", about ${signedWeight(it, unit)} a week") }
    out.append('.')
    val record = points.lastOrNull { it.record }
    val newest = points.lastOrNull()
    if (record != null && newest != null) {
        val gap = daysFrom(record.day, newest.day)
        if (gap != null && gap > 35) out.append(" No new record in ${gap / 7} weeks; the last was ${humanDate(record.day)}.")
        else out.append(" Last record ${humanDate(record.day)}.")
    }
    return out.toString()
}

/**
 * One lift (avex_pages.rs `lift_page`): what it has done in words, its best set and its four
 * weeks, its estimated max over time, and every workout it was in, each opening to its sets. A
 * copy from before this week is read as of its export's day, as the Lifts tab reads it.
 */
@Composable
fun AvexLiftScreen(name: String, nav: Nav) {
    val summary = rememberLive("gym.summary")
    val asOf = (summary.result as? JsonObject)?.let(::staleExport)
    // The day to read to is known once the summary has answered, or could not.
    val ready = summary.result != null || summary.error != null
    val live = if (ready) rememberLive("gym.lift.get", remember(name, asOf) { buildJsonObject { put("name", name); put("limit", 400); asOf?.let { put("today", it) } } }) else null
    val row = (live?.result as? JsonObject)?.o("row")
    DetailPage(
        nav,
        row?.s("name") ?: name,
        row?.let { "${plural(it.l("sessions") ?: 0L, "workout")} · ${plural(it.l("sets") ?: 0L, "set")} · last ${inlineDate(it.s("last_day"))}" },
    ) {
        if (live == null) Quiet("Reading Avex…")
        else Reading(live, "This engine cannot read one lift yet. Update Relay's engine to see it here.") { LiftPage(it, asOf) }
    }
}

@Composable
private fun LiftPage(d: JsonObject, asOf: String?) {
    val c = Relay.colors
    val unit = d.s("unit") ?: "lb"
    val row = d.o("row") ?: return
    val name = row.str("name")
    val history = d.objs("history")
    // Each workout's best set as an estimated max, oldest first.
    val points = history.mapNotNull { p -> p.d("e1rm")?.let { LiftPoint(p.str("day"), it, p.b("record") == true) } }.reversed()
    val day = asOf ?: LocalDate.now().toString()
    val words = trendSentence(row, points, unit, day)
    T(words, style = Relay.type.ui, color = c.ink2)
    if (points.size >= 2) AskKey(liftQuestion(name, liftTrend(row, day) == LiftTrend.Slipping || words.contains("No new record")))
    val best = row.o("best")
    if (best != null) {
        val change = row.d("change_pct")
        val recent = row.d("recent_e1rm")
        StatGrid(listOf(
            { m -> Stat("Best set", setWords(best.d("weight"), best.l("reps") ?: 0L, unit), "", m) },
            { m ->
                val e = best.d("e1rm")
                if (e != null) Stat("Best est. max", "${weight(e)} $unit", humanDate(best.s("day")), m)
                else Stat("Best est. max", "—", "No set of 1 to 12 reps", m)
            },
            { m ->
                val sub = when {
                    change != null && recent != null -> "From est. max ${weight(recent / (1 + change / 100))} $unit the four weeks before"
                    recent != null -> "Nothing in the four weeks before"
                    else -> "Not done in the last four weeks"
                }
                Stat("Four-week change", change?.let(::pct) ?: "—", sub, m)
            },
        ))
    } else {
        StatGrid(listOf({ m -> Stat("Best set", row.l("best_reps")?.let { plural(maxOf(it, 0L), "rep") } ?: "—", "Without weight", m) }))
    }
    if (points.size >= 2) {
        ChartCard(ChartSpec(ChartKind.Line, "Estimated max over time", gym = remember(name) { buildJsonObject { put("measure", "e1rm"); put("by", "session"); put("lift", name); put("periods", 260) } }))
        T("Each workout's best set. $EST_MAX.", style = Relay.type.caption, color = c.ink3)
    }
    // The newest first; years of them wait behind one key.
    var all by rememberSaveable { mutableStateOf(false) }
    Card("Every workout", aside = if (history.size > WORKOUTS_SHOWN && !all) "$WORKOUTS_SHOWN of ${history.size}" else null) {
        if (history.isEmpty()) Quiet("No workout with this lift in the export.")
        history.take(if (all) history.size else WORKOUTS_SHOWN).forEachIndexed { i, p -> LiftWorkoutRow(p, unit, divider = i > 0) }
        if (history.size > WORKOUTS_SHOWN) Key(if (all) "Show fewer" else "Show all ${history.size}", { all = !all }, Modifier.padding(bottom = 4.dp), kind = KeyKind.Quiet)
    }
}

/** How many of a lift's workouts its page shows before "Show all". */
private const val WORKOUTS_SHOWN = 30

/** A workout the lift was in: its day, its sets, volume and estimated max, its top set; it opens to the workout's sets. */
@Composable
private fun LiftWorkoutRow(p: JsonObject, unit: String, divider: Boolean) {
    val c = Relay.colors
    val id = p.l("session_id")
    var open by rememberSaveable(id) { mutableStateOf(false) }
    val record = p.b("record") == true
    val detail = listOfNotNull(
        plural(p.l("sets") ?: 0L, "set"),
        // Sets without weight move none.
        p.d("volume")?.takeIf { it > 0 }?.let { volume(it, unit) },
        p.d("e1rm")?.let { "est. max ${weight(it)} $unit" },
    ).joinToString(" · ")
    val top = p.d("top_weight")?.let { setWords(it, p.l("top_reps") ?: 0L, unit) } ?: plural(p.l("top_reps") ?: 0L, "rep")
    FigureRow(humanDate(p.s("day")) + if (record) " · record" else "", detail, top, figureColor = if (record) c.live else c.ink2, divider = divider, onClick = if (id != null) ({ open = !open }) else null)
    AnimatedVisibility(open) { id?.let { Workout(it, unit) } }
}
