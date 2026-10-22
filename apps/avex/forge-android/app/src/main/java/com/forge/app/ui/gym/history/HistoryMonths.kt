package com.forge.app.ui.gym.history

import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.TextStyle
import java.util.Locale
import kotlin.math.abs

/**
 * The History page's type filter (2026-10-21 redesign): every entry, gym only, cardio only, or only
 * the sessions that set a personal record. One pick of four.
 */
enum class HistoryKind(val label: String) {
    ALL("All"),
    GYM("Gym"),
    CARDIO("Cardio"),
    PRS("PRs")
}

/** True when [item] passes the type filter [kind]. */
internal fun HistoryKind.admits(item: HistoryItem): Boolean = when (this) {
    HistoryKind.ALL -> true
    HistoryKind.GYM -> item is HistoryItem.Workout
    HistoryKind.CARDIO -> item is HistoryItem.Cardio
    HistoryKind.PRS -> item is HistoryItem.Workout && item.session.prCount > 0
}

/**
 * One calendar month of History: its name ("October", or "October 2025" outside this year), how
 * many sessions and PRs it holds, and its entries newest first.
 */
data class HistoryMonth(
    val key: String,
    val label: String,
    val sessions: Int,
    val prs: Int,
    val items: List<HistoryItem>
) {
    /** "13 sessions · 1 PR"; the PR clause only when there was one. */
    val meta: String
        get() = buildString {
            append("$sessions ${if (sessions == 1) "session" else "sessions"}")
            if (prs > 0) append(" · $prs ${if (prs == 1) "PR" else "PRs"}")
        }
}

/** Group an already-filtered, newest-first list into calendar months, order preserved. */
internal fun groupByMonth(
    items: List<HistoryItem>,
    today: LocalDate = LocalDate.now(ZoneId.systemDefault()),
    zone: ZoneId = ZoneId.systemDefault(),
    locale: Locale = Locale.getDefault()
): List<HistoryMonth> =
    items.groupBy { YearMonth.from(Instant.ofEpochMilli(it.dateMs).atZone(zone)) }
        .map { (month, rows) ->
            val name = month.month.getDisplayName(TextStyle.FULL_STANDALONE, locale)
                .replaceFirstChar { it.titlecase(locale) }
            HistoryMonth(
                key = month.toString(),
                label = if (month.year == today.year) name else "$name ${month.year}",
                sessions = rows.size,
                prs = rows.sumOf { (it as? HistoryItem.Workout)?.session?.prCount ?: 0 },
                items = rows
            )
        }

/**
 * The line under the title: "212 workouts and 64 cardio sessions since Mar 2025" for what the
 * filters leave. Zero reads as zero ("0 workouts"), never as a dash.
 */
internal fun historyMeta(
    items: List<HistoryItem>,
    zone: ZoneId = ZoneId.systemDefault(),
    locale: Locale = Locale.getDefault()
): String {
    val gym = items.count { it is HistoryItem.Workout }
    val cardio = items.size - gym
    val first = items.minOfOrNull { it.dateMs }
    val since = first?.let {
        val d = Instant.ofEpochMilli(it).atZone(zone).toLocalDate()
        " since ${d.month.getDisplayName(TextStyle.SHORT, locale)} ${d.year}"
    }.orEmpty()
    val w = "$gym ${if (gym == 1) "workout" else "workouts"}"
    val c = "$cardio cardio ${if (cardio == 1) "session" else "sessions"}"
    return "$w and $c$since"
}

/**
 * What a gym row says beyond its name: its heaviest set ("Squat 275×5") and each lift against its
 * own last time ([deltas], in percent, null = nothing to compare). Loaded per row, once.
 */
data class HistoryRowDetail(
    val lead: String?,
    val deltas: List<Float?>
)

/** A move under half a percent reads as "matched": a stub, not a bar. */
internal fun deltaBars(pcts: List<Float?>): List<Float?> =
    pcts.map { p -> p?.let { if (abs(it) < 0.5f) 0f else it } }
