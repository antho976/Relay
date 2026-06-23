package com.forge.app.ui.gym.history

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.domain.cardio.CardioType
import com.forge.app.program.Program

enum class SessionHistoryFilter { SHORT, LONG, HIGH_VOLUME }

/** A session counts as "high volume" at/above this many lb. Shared by the filter predicate and the
 *  chip label (which converts it to the display unit) so the two can never drift apart. */
const val HIGH_VOLUME_LB = 3000.0

/** One entry in the unified history — either a finished gym workout or a logged cardio session. */
sealed interface HistoryItem {
    val dateMs: Long

    /** Globally-unique LazyColumn key — gym & cardio ids come from different tables and can collide. */
    val key: String

    data class Workout(val session: Session) : HistoryItem {
        override val dateMs get() = session.startedAt
        override val key get() = "w${session.id}"
    }

    data class Cardio(val entry: CardioEntry) : HistoryItem {
        override val dateMs get() = entry.date
        override val key get() = "c${entry.id}"
    }
}

/** The user-controlled filter inputs, held off the session list so a filter change doesn't re-query. */
data class HistoryFilters(
    val query: String = "",
    val tag: String? = null,
    val duration: SessionHistoryFilter? = null,
    val volume: SessionHistoryFilter? = null
)

/**
 * Pure: merge gym workouts + cardio, apply the [filters], and return them newest-first. Computed
 * once per data/filter emission in the ViewModel (NOT a recomposition-time getter), so the History
 * list reads a ready-made list. [exerciseNamesBySession] is the search index for exercise matches.
 */
internal fun buildFilteredHistory(
    workouts: List<Session>,
    cardio: List<CardioEntry>,
    exerciseNamesBySession: Map<Long, List<String>>,
    filters: HistoryFilters
): List<HistoryItem> {
    val needle = filters.query.trim().lowercase()
    val gym = workouts.filter { matchesWorkout(it, needle, filters, exerciseNamesBySession) }
        .map { HistoryItem.Workout(it) }
    val card = cardio.filter { matchesCardio(it, needle, filters) }
        .map { HistoryItem.Cardio(it) }
    return (gym + card).sortedByDescending { it.dateMs }
}

/** Distinct quick tags across all workouts, alphabetised — the source for the filter chips. */
internal fun availableTagsOf(workouts: List<Session>): List<String> =
    workouts.flatMap { it.tags.split(",") }
        .map { it.trim() }
        .filter { it.isNotEmpty() }
        .distinct()
        .sorted()

private fun matchesWorkout(
    s: Session,
    needle: String,
    f: HistoryFilters,
    names: Map<Long, List<String>>
): Boolean {
    if (needle.isNotEmpty() && !workoutMatchesQuery(s, needle, names)) return false
    f.tag?.let { tag -> if (s.tags.split(",").none { it.trim().equals(tag, ignoreCase = true) }) return false }
    f.duration?.let { d ->
        val min = s.durationMinutes()
        when (d) {
            SessionHistoryFilter.SHORT -> if ((min ?: Int.MAX_VALUE) >= 45) return false
            SessionHistoryFilter.LONG -> if ((min ?: 0) <= 60) return false
            else -> {}
        }
    }
    f.volume?.let { v ->
        if (v == SessionHistoryFilter.HIGH_VOLUME && !(s.totalVolumeLb != null && s.totalVolumeLb > HIGH_VOLUME_LB)) return false
    }
    return true
}

private fun matchesCardio(e: CardioEntry, needle: String, f: HistoryFilters): Boolean {
    // Cardio carries no tags or volume → it drops out when a tag or high-volume filter is on.
    if (f.tag != null) return false
    if (f.volume == SessionHistoryFilter.HIGH_VOLUME) return false
    if (needle.isNotEmpty()) {
        val hay = (CardioType.fromCode(e.type).displayName + " " + (e.note ?: "") + " cardio").lowercase()
        if (!hay.contains(needle)) return false
    }
    f.duration?.let { d ->
        when (d) {
            SessionHistoryFilter.SHORT -> if (e.durationMin >= 45) return false
            SessionHistoryFilter.LONG -> if (e.durationMin <= 60) return false
            else -> {}
        }
    }
    return true
}

/** Case-insensitive match against the day name, the session journal, or any logged exercise name. */
private fun workoutMatchesQuery(s: Session, needle: String, names: Map<Long, List<String>>): Boolean {
    if (Program.dayDisplayName(s.dayKey).lowercase().contains(needle)) return true
    if (s.journal.lowercase().contains(needle)) return true
    return names[s.id].orEmpty().any { it.lowercase().contains(needle) }
}
