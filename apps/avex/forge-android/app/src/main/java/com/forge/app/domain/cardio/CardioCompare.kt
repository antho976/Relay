package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry

/**
 * How one active session stands against the rest of its activity type — the fastest pace and the
 * longest distance ever logged for that type, plus the session logged just before this one. Pure and
 * computed off the in-memory entry list (no extra DB reads); drives the compare reads on the
 * session-detail screen. Rest entries never compare, and sessions only compare within their own type
 * (a ride's pace against a run's pace means nothing).
 */
data class CardioSessionCompare(
    /** The OTHER same-type session with the fastest pace, or null when none has a pace. Carried as the
     *  entry (not a pre-rounded figure) so its pace formats through the same one-rounding path as its
     *  own detail row and the two screens can't disagree. */
    val bestPaceEntry: CardioEntry?,
    /** True when this session's pace beats (or matches) every other same-type session. */
    val isPaceBest: Boolean,
    /** Longest same-type distance among OTHER sessions (km), or null when none has a distance. */
    val bestOtherDistanceKm: Double?,
    /** True when this session's distance beats (or matches) every other same-type session. */
    val isDistanceBest: Boolean,
    /** The same-type session logged most recently before this one, if any. */
    val previous: CardioEntry?
)

/** Average pace in seconds per kilometre, or null when duration or distance is missing. */
fun paceSecPerKm(durationMin: Int, distanceKm: Double?): Int? =
    paceSecPerUnit(durationMin, distanceKm, useMiles = false)

/**
 * Compare [entry] against every other logged session of the same activity type. Returns null when
 * there is nothing to say — a rest entry, or the very first session of its type.
 */
fun compareCardioSession(entry: CardioEntry, all: List<CardioEntry>): CardioSessionCompare? {
    if (CardioType.fromCode(entry.type).isRest) return null
    val others = all.filter { it.type == entry.type && it.id != entry.id }
    if (others.isEmpty()) return null

    val myPace = paceSecPerKm(entry.durationMin, entry.distanceKm)
    // Rank in sec/km (unit-independent); keep the winning entry so the UI formats its pace from raw.
    val bestPace = others
        .mapNotNull { e -> paceSecPerKm(e.durationMin, e.distanceKm)?.let { e to it } }
        .minByOrNull { it.second }
    val myDistance = entry.distanceKm?.takeIf { it > 0.0 }
    val bestOtherDistance = others.mapNotNull { it.distanceKm?.takeIf { d -> d > 0.0 } }.maxOrNull()

    return CardioSessionCompare(
        bestPaceEntry = bestPace?.first,
        isPaceBest = myPace != null && (bestPace == null || myPace <= bestPace.second),
        bestOtherDistanceKm = bestOtherDistance,
        isDistanceBest = myDistance != null && (bestOtherDistance == null || myDistance >= bestOtherDistance),
        previous = others.filter { it.date < entry.date }.maxByOrNull { it.date }
    )
}
