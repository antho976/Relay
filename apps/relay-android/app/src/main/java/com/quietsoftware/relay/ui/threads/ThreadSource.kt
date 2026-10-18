package com.quietsoftware.relay.ui.threads

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.kit.plural
import kotlinx.serialization.json.JsonObject
import java.time.LocalDate
import java.time.temporal.ChronoUnit

// One source switch (docs/THREADS.md, "One source switch"; threads_view.rs `set_source`): Tally,
// Arbiter or Avex is chosen once, and what follows it is a new thread's line and questions, the
// message box's line on what the agent may do, and an open thread's placeholder. The agent itself
// is the same whichever shows.

/** The sources, as the desktop's switch names them. */
internal val THREAD_SOURCES = listOf("tally" to "Tally", "arbiter" to "Arbiter", "avex" to "Avex")

/** The source showing, chosen on a new thread; kept while the app runs. */
internal object ThreadSource {
    var key by mutableStateOf("tally")
}

/** What the agent may do alone with [source] showing, in the line under the message box (threads_view.rs `show_access`). */
internal fun sourceLimits(source: String) = when (source) {
    "arbiter" -> "Proposes, you approve"
    "avex" -> "Reads only"
    else -> DATA_LIMITS
}

/** An open thread's placeholder with [source] showing (threads_view.rs `show_hint`). */
internal fun sourceHint(source: String) = when (source) {
    "avex" -> "Ask about your training"
    "arbiter" -> "Ask about your strategies and orders"
    else -> "Ask about your money, or tell it what to change"
}

/** Questions a new thread offers with Avex showing, when its export suggests fewer (threads_view.rs `AVEX_SUGGESTIONS`). */
private val AVEX_SUGGESTIONS = listOf(
    "How is my week going?" to "Workouts and volume against last week",
    "Which lifts have stalled?" to "Flat or down over four weeks",
    "What are my latest records?" to "Personal bests and the sets behind them",
    "Am I training consistently?" to "Weeks in a row and days a week",
)

/** The day Avex's export was written, `YYYY-MM-DD`; empty before one was imported. */
private fun exportDay(s: JsonObject): String = s.o("imported")?.s("exported_at")?.take(10).orEmpty()

/** A week of `gym.summary`'s in words: "3 of 4 workouts, 46,330 lb lifted", or "no workouts". */
private fun weekWords(s: JsonObject, week: JsonObject): String {
    val sessions = week.l("sessions") ?: 0L
    if (sessions <= 0) return "no workouts"
    val target = s.o("imported")?.l("days_per_week") ?: 0L
    val count = if (target > 0) "$sessions of ${plural(target, "workout")}" else plural(sessions, "workout")
    return "$count, ${group(Math.round(week.d("volume") ?: 0.0))} ${s.s("unit") ?: "lb"} lifted"
}

/**
 * A new thread's line from Avex (`gym.summary`; threads_view.rs `week_line`): the week its export
 * reaches, and how old the copy is once that week is over, so a stale copy never reads as a week
 * without training.
 */
internal fun avexLine(s: JsonObject, today: LocalDate = LocalDate.now()): String {
    if (s.o("imported") == null) return "Nothing from Avex yet. Export your training history in Avex, then import it on the PC."
    val day = exportDay(s)
    val shown = humanDate(day)
    val thisWeek = s.o("this_week")
    if (day.isEmpty() || day >= thisWeek?.s("start").orEmpty()) {
        val words = thisWeek?.takeIf { (it.l("sessions") ?: 0L) > 0 }?.let { weekWords(s, it) } ?: "no workouts yet"
        return when {
            day.isEmpty() || day == today.toString() -> "This week so far: $words."
            shown == "Yesterday" -> "This week, to yesterday's export: $words."
            else -> "This week, to your export on $shown: $words."
        }
    }
    val exported = dayOf(day)
    val age = exported?.let { ChronoUnit.DAYS.between(it, today) }?.takeIf { it >= 2 }?.let { ", ${plural(it, "day")} ago" }.orEmpty()
    val from = if (shown == "Yesterday") "yesterday" else shown
    // The week the export falls in, among the twelve the summary keeps.
    val week = s.objs("weeks").firstOrNull { w ->
        val start = dayOf(w.s("start"))
        start != null && exported != null && !start.isAfter(exported) && exported.isBefore(start.plusDays(7))
    }
    return if (week != null) "Your Avex copy is from $from$age. That week: ${weekWords(s, week)}."
    else "Your Avex copy is from $from$age. Import a newer export to see the weeks since."
}

/**
 * The questions a new thread offers with Avex showing, from its export (threads_view.rs
 * `avex_suggestions`): the week it reaches, the lifts that stalled, the lift done most and the
 * workout done last. At most four.
 */
internal fun avexSuggestions(s: JsonObject?): List<Pair<String, String>> {
    val out = ArrayList<Pair<String, String>>()
    var stale = false
    if (s != null) {
        if (s.o("imported") == null) return listOf("How do I bring my training in from Avex?" to "Export it in Avex, then import it on the PC")
        val day = exportDay(s)
        stale = day.isNotEmpty() && day < s.o("this_week")?.s("start").orEmpty()
        out += if (stale) ("How did my last week of training go?" to "Your copy ends ${humanDate(day)}") else AVEX_SUGGESTIONS[0]
        out += AVEX_SUGGESTIONS[1]
        s.objs("lifts").firstOrNull()?.s("name")?.takeIf { it.isNotEmpty() }?.let { out += "How is my $it trending?" to "Its estimated max, week by week" }
        s.objs("recent").mapNotNull { it.s("title") }.firstOrNull { it.isNotEmpty() && it != "Freestyle" }?.let { out += "Plan my next $it" to "Sets and weights from your last one" }
    }
    for (pair in AVEX_SUGGESTIONS) {
        if (out.size >= 4) break
        // A stale copy has no "this week" to ask about.
        if ((stale && pair == AVEX_SUGGESTIONS[0]) || out.any { it.first == pair.first }) continue
        out += pair
    }
    return out.take(4)
}
