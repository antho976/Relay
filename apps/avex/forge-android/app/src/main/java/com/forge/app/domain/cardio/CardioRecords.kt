package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry

/**
 * One activity type's all-time bests (GYMAP-34) — the longest distance and the fastest pace ever
 * logged for that type, and how many sessions back them. Pure, computed off the in-memory entry list
 * (no extra DB reads). Both record entries are non-null: a type only produces a record once it has a
 * real distance session (which also gives it a pace), so rest days and duration-only activities
 * (yoga, HIIT with no distance) never appear.
 */
data class CardioActivityRecord(
    val typeCode: String,
    val sessions: Int,
    /** The session with the longest distance for this type — the row's headline record. */
    val longestEntry: CardioEntry,
    /** The session with the fastest pace for this type (lowest sec/km). */
    val fastestEntry: CardioEntry
)

/**
 * All-time per-activity records, most-logged type first (ties broken by type code for a stable order).
 * A type qualifies once it has at least one session carrying a real distance and duration — which is
 * exactly the pair that also defines a pace — so every returned record has a longest and a fastest
 * entry. [CardioType.REST] and distance-less sessions are filtered out before grouping.
 */
fun cardioActivityRecords(entries: List<CardioEntry>): List<CardioActivityRecord> =
    entries
        .filter { it.type != CardioType.REST.code && (it.distanceKm ?: 0.0) > 0.0 && it.durationMin > 0 }
        .groupBy { it.type }
        .mapNotNull { (code, group) ->
            val longest = group.maxByOrNull { it.distanceKm ?: 0.0 } ?: return@mapNotNull null
            val fastest = group.minByOrNull { paceSecPerKm(it.durationMin, it.distanceKm) ?: Int.MAX_VALUE }
                ?: return@mapNotNull null
            CardioActivityRecord(typeCode = code, sessions = group.size, longestEntry = longest, fastestEntry = fastest)
        }
        .sortedWith(compareByDescending<CardioActivityRecord> { it.sessions }.thenBy { it.typeCode })
