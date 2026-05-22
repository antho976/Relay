package com.forge.app.domain.trophy

/**
 * A point-in-time read of every counter / max the trophy rules care about. The
 * evaluator operates only on this snapshot — no DAO access — so it stays a pure
 * function and is trivial to unit-test with a hand-built snapshot.
 *
 * Built once in [com.forge.app.data.repo.TrophyRepository.snapshot] from a fan-out
 * of suspend DAO calls. Cheap: each underlying query is a single-row aggregate.
 */
data class TrophyStatsSnapshot(
    val totalLoggedExercises: Int,
    val totalPrs: Int,
    val brutalRatings: Int,
    val swapsUsed: Int,
    val fullTargetHits: Int,
    val finishedSessions: Int,
    val distinctDayKeysTrained: Int,
    val maxBenchLb: Double,
    val maxSquatLb: Double,
    val maxSessionVolumeLb: Double
)
