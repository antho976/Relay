package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import java.util.Locale
import kotlin.math.roundToInt

/**
 * How one active session stands against the rest of its activity type — the fastest pace and the
 * longest distance ever logged for that type, plus the session logged just before this one. Pure and
 * computed off the in-memory entry list (no extra DB reads); drives the compare reads on the
 * session-detail screen. Rest entries never compare, and sessions only compare within their own type
 * (a ride's pace against a run's pace means nothing).
 */
data class CardioSessionCompare(
    /** Fastest same-type pace among OTHER sessions (sec per km), or null when none has a pace. */
    val bestOtherPaceSecPerKm: Int?,
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
fun paceSecPerKm(durationMin: Int, distanceKm: Double?): Int? {
    if (distanceKm == null || distanceKm <= 0.0 || durationMin <= 0) return null
    return (durationMin * 60.0 / distanceKm).roundToInt()
}

/** "M:SS" for a pace reading or gap in seconds. Locale.US — a stopwatch reading never localises. */
fun formatPaceSec(sec: Int): String = String.format(Locale.US, "%d:%02d", sec / 60, sec % 60)

/** A sec-per-km pace reading converted to seconds per display unit (km or mile). */
fun paceSecPerUnit(secPerKm: Int, useMiles: Boolean): Int =
    if (useMiles) (secPerKm * 1.609344).roundToInt() else secPerKm

/**
 * Compare [entry] against every other logged session of the same activity type. Returns null when
 * there is nothing to say — a rest entry, or the very first session of its type.
 */
fun compareCardioSession(entry: CardioEntry, all: List<CardioEntry>): CardioSessionCompare? {
    if (CardioType.fromCode(entry.type).isRest) return null
    val others = all.filter { it.type == entry.type && it.id != entry.id }
    if (others.isEmpty()) return null

    val myPace = paceSecPerKm(entry.durationMin, entry.distanceKm)
    val bestOtherPace = others.mapNotNull { paceSecPerKm(it.durationMin, it.distanceKm) }.minOrNull()
    val myDistance = entry.distanceKm?.takeIf { it > 0.0 }
    val bestOtherDistance = others.mapNotNull { it.distanceKm?.takeIf { d -> d > 0.0 } }.maxOrNull()

    return CardioSessionCompare(
        bestOtherPaceSecPerKm = bestOtherPace,
        isPaceBest = myPace != null && (bestOtherPace == null || myPace <= bestOtherPace),
        bestOtherDistanceKm = bestOtherDistance,
        isDistanceBest = myDistance != null && (bestOtherDistance == null || myDistance >= bestOtherDistance),
        previous = others.filter { it.date < entry.date }.maxByOrNull { it.date }
    )
}
