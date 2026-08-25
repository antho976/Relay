package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.TrophyNearMissDao
import com.forge.app.data.db.dao.UnlockedTrophyDao
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.TrophyNearMiss
import com.forge.app.data.db.entities.UnlockedTrophy
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.trophy.TrophyEvaluator
import com.forge.app.domain.trophy.TrophyExercises
import com.forge.app.domain.trophy.TrophyStatsSnapshot
import com.forge.app.program.Program
import com.forge.app.program.Trophies
import com.forge.app.program.Trophy
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.Flow
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class TrophyRepository @Inject constructor(
    private val unlockedDao: UnlockedTrophyDao,
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val nearMissDao: TrophyNearMissDao,
    private val vacationDao: com.forge.app.data.db.dao.VacationDao,
    private val cardioDao: com.forge.app.data.db.dao.CardioDao,
    private val goalRepository: GoalRepository,
    private val clock: Clock
) {
    fun observeAll(): Flow<List<UnlockedTrophy>> = unlockedDao.observeAll()
    fun observeUnlockedIds(): Flow<List<String>> = unlockedDao.observeUnlockedIds()
    suspend fun unlockedIds(): Set<String> = unlockedDao.unlockedIds().toSet()

    /** trophyId → when it was unlocked (epoch ms), for surfacing the earned date. */
    suspend fun unlockedDatesById(): Map<String, Long> = unlockedDao.all().associate { it.trophyId to it.unlockedAt }

    suspend fun unlock(trophyId: String) {
        unlockedDao.unlock(UnlockedTrophy(trophyId = trophyId, unlockedAt = clock.nowMs()))
    }

    suspend fun unlockMany(trophyIds: Collection<String>) {
        val now = clock.nowMs()
        trophyIds.forEach { id ->
            unlockedDao.unlock(UnlockedTrophy(trophyId = id, unlockedAt = now))
        }
    }

    suspend fun snapshot(): TrophyStatsSnapshot = coroutineScope {
        // Fire the ~14 independent DAO reads concurrently — run serially these were the long pole on
        // profile open (#8). Room serves concurrent reads and none of these depend on each other; the
        // CPU passes over `allSessions` (streak/early-bird/etc.) run after it resolves.
        val allSessionsD = async { sessionDao.allFinished() }
        val vacationD = async { vacationDao.all() }
        val totalLoggedD = async { loggedExerciseDao.totalLogged() }
        val totalPrsD = async { loggedExerciseDao.prCount() }
        val brutalD = async { loggedExerciseDao.countWithRating(EffortRating.BRUTAL) }
        val swapsD = async { loggedExerciseDao.swapCount() }
        val fullTargetD = async { loggedExerciseDao.fullTargetCount() }
        // finishedSessions + distinctDayKeysTrained are derived from `allSessions` below — both DAO
        // queries scan the same `finished_at IS NOT NULL` rows we already load, so no extra read.
        val maxBenchD = async { loggedSetDao.maxWeightAcrossExercises(TrophyExercises.BENCH_EXERCISE_IDS) }
        val maxSquatD = async { loggedSetDao.maxWeightAcrossExercises(TrophyExercises.SQUAT_EXERCISE_IDS) }
        val maxSessVolD = async { loggedSetDao.maxSessionVolume() }
        val maxRepsD = async { loggedSetDao.maxRepsSummedPerExercise() }
        val goalsD = async { goalRepository.achievedCount() }
        val cardioSessionsD = async { cardioDao.totalSessions() }
        val cardioDistanceD = async { cardioDao.totalDistanceKm() }

        val allSessions = allSessionsD.await()
        val zone = ZoneId.systemDefault()
        val onVacation = com.forge.app.domain.vacation.VacationCalendar.onVacation(vacationD.await())
        // Lifetime tonnage + first session derived from the already-loaded session list — no extra query.
        val trackedSessions = allSessions.filter { !it.isUntracked }
        val lifetimeTonnageLb = trackedSessions.sumOf { it.totalVolumeLb ?: 0.0 }
        val firstSessionMs = trackedSessions.minOfOrNull { it.startedAt }
        TrophyStatsSnapshot(
            totalLoggedExercises = totalLoggedD.await(),
            totalPrs = totalPrsD.await(),
            brutalRatings = brutalD.await(),
            swapsUsed = swapsD.await(),
            fullTargetHits = fullTargetD.await(),
            finishedSessions = allSessions.size,
            distinctDayKeysTrained = allSessions.mapTo(mutableSetOf()) { it.dayKey }.size,
            maxBenchLb = maxBenchD.await() ?: 0.0,
            maxSquatLb = maxSquatD.await() ?: 0.0,
            maxSessionVolumeLb = maxSessVolD.await() ?: 0.0,
            maxStreakEver = computeMaxStreak(allSessions, zone, onVacation),
            earlyBirdSessions = countSessionsBefore(allSessions, zone, hour = 7),
            nightOwlSessions = countSessionsAfter(allSessions, zone, hour = 21),
            sundaysTrainedCount = countSundays(allSessions, zone),
            maxSessionDurationMinutes = maxDurationMinutes(allSessions),
            minFinishedSessionDurationMinutes = minDurationMinutes(allSessions),
            maxSingleExerciseReps = maxRepsD.await() ?: 0,
            comebackKidEarned = checkComebackKid(allSessions, zone),
            consistencyKingEarned = checkConsistencyKing(allSessions, zone),
            varietyPackEarned = checkVarietyPack(allSessions, zone),
            exerciseGoalsAchieved = goalsD.await(),
            lifetimeTonnageLb = lifetimeTonnageLb,
            firstSessionMs = firstSessionMs,
            cardioSessions = cardioSessionsD.await(),
            cardioDistanceKm = cardioDistanceD.await() ?: 0.0,
            nowMs = clock.nowMs()
        )
    }

    suspend fun evaluateAndUnlockNew(): List<Trophy> {
        val snapshot = snapshot()
        val satisfied = TrophyEvaluator.unlockedByRule(snapshot)
        val already = unlockedIds()
        val newlyUnlockedIds = satisfied - already
        // Near-misses are LOCKED trophies (not yet unlocked and not yet satisfied). The old
        // `already - satisfied` was the unlocked-but-unsatisfied set, which is virtually always empty.
        val lockedIds = Trophies.all.mapTo(mutableSetOf()) { it.id } - already - satisfied
        recordNearMisses(snapshot, lockedIds)
        // Prune stale near-miss rows for trophies that are now unlocked — they aren't auto-removed on
        // unlock, so without this an already-earned trophy can resurface as a near-miss / "Up next".
        runCatching { nearMissDao.deleteForTrophies(already + newlyUnlockedIds) }
        if (newlyUnlockedIds.isEmpty()) return emptyList()
        unlockMany(newlyUnlockedIds)
        return Trophies.all.filter { it.id in newlyUnlockedIds }
    }

    fun observeNearMisses() = nearMissDao.observeRecent()

    /**
     * Rewrites the near-miss list for every trophy this pass evaluated: insert the ones currently in
     * the 80–99 % band, then drop everything recorded for those trophies earlier than this pass.
     * One live row per trophy, so the `LIMIT 50` read window means "the 50 closest trophies" rather
     * than "whatever a single recent pass happened to write" — and a trophy that has since fallen
     * back below 80 % leaves with its stale row instead of lingering as a near-miss.
     *
     * Insert-then-prune (rather than prune-then-insert) keeps the observed list non-empty
     * throughout; rows are stamped with a single `now` so the `< now` prune can't touch them.
     */
    private suspend fun recordNearMisses(snapshot: TrophyStatsSnapshot, lockedIds: Set<String>) {
        if (lockedIds.isEmpty()) return
        val now = clock.nowMs()
        Trophies.all
            .filter { it.id in lockedIds }
            .forEach { trophy ->
                val (progress, target) = TrophyEvaluator.progressFor(trophy, snapshot)
                if (target > 0 && progress > 0 && progress.toFloat() / target >= 0.8f) {
                    nearMissDao.insert(
                        TrophyNearMiss(
                            trophyId = trophy.id,
                            trophyName = trophy.name,
                            progress = progress,
                            target = target,
                            recordedAt = now
                        )
                    )
                }
            }
        runCatching { nearMissDao.deleteForTrophiesBefore(lockedIds, now) }
    }

    // ─── Snapshot helpers ─────────────────────────────────────────────────────

    private fun computeMaxStreak(sessions: List<Session>, zone: ZoneId, onVacation: (LocalDate) -> Boolean): Int {
        if (sessions.isEmpty()) return 0
        val days = sessions.mapTo(sortedSetOf()) {
            Instant.ofEpochMilli(it.startedAt).atZone(zone).toLocalDate()
        }
        var maxStreak = 1
        var streak = 1
        var prev = days.first()
        for (day in days.drop(1)) {
            // A holiday between two sessions doesn't break the streak: bridge if every day
            // strictly between is a vacation day (a 1-day gap has none, so it bridges trivially).
            streak = if (allDaysBetweenAreVacation(prev, day, onVacation)) streak + 1 else 1
            if (streak > maxStreak) maxStreak = streak
            prev = day
        }
        return maxStreak
    }

    private fun allDaysBetweenAreVacation(prev: LocalDate, next: LocalDate, onVacation: (LocalDate) -> Boolean): Boolean {
        var d = prev.plusDays(1)
        while (d.isBefore(next)) {
            if (!onVacation(d)) return false
            d = d.plusDays(1)
        }
        return true
    }

    private fun countSessionsBefore(sessions: List<Session>, zone: ZoneId, hour: Int): Int =
        sessions.count { s ->
            val time = Instant.ofEpochMilli(s.startedAt).atZone(zone).toLocalTime()
            time.isBefore(LocalTime.of(hour, 0))
        }

    private fun countSessionsAfter(sessions: List<Session>, zone: ZoneId, hour: Int): Int =
        sessions.count { s ->
            val time = Instant.ofEpochMilli(s.startedAt).atZone(zone).toLocalTime()
            !time.isBefore(LocalTime.of(hour, 0))
        }

    private fun countSundays(sessions: List<Session>, zone: ZoneId): Int =
        sessions.count { s ->
            Instant.ofEpochMilli(s.startedAt).atZone(zone).dayOfWeek == DayOfWeek.SUNDAY
        }

    private fun maxDurationMinutes(sessions: List<Session>): Int =
        sessions.maxOfOrNull { it.durationMinutes() ?: 0 } ?: 0

    private fun minDurationMinutes(sessions: List<Session>): Int =
        sessions
            .mapNotNull { s -> s.durationMinutes()?.takeIf { s.setCount > 0 && it >= 5 } }
            .minOrNull() ?: Int.MAX_VALUE

    private fun checkComebackKid(sessions: List<Session>, zone: ZoneId): Boolean {
        if (sessions.size < 2) return false
        for (i in 1 until sessions.size) {
            val prevDate = Instant.ofEpochMilli(sessions[i - 1].startedAt).atZone(zone).toLocalDate()
            val currDate = Instant.ofEpochMilli(sessions[i].startedAt).atZone(zone).toLocalDate()
            // Calendar-day gap, not raw-millisecond truncation (which rounded e.g. 4d23h down to 4).
            val gapDays = ChronoUnit.DAYS.between(prevDate, currDate)
            if (gapDays in 5..14 && sessions[i].prCount > 0) return true
        }
        return false
    }

    private fun checkConsistencyKing(sessions: List<Session>, zone: ZoneId): Boolean {
        if (sessions.isEmpty()) return false
        val today = LocalDate.now(zone)
        val threeMonthsAgo = today.minusMonths(3)
        val trainingDays = sessions.mapTo(mutableSetOf()) {
            Instant.ofEpochMilli(it.startedAt).atZone(zone).toLocalDate()
        }
        var week = threeMonthsAgo
        while (!week.isAfter(today.minusDays(7))) {
            val weekEnd = week.plusDays(6)
            val hasSession = (0..6).any { offset -> trainingDays.contains(week.plusDays(offset.toLong())) }
            if (!hasSession) return false
            week = weekEnd.plusDays(1)
        }
        return true
    }

    private fun checkVarietyPack(sessions: List<Session>, zone: ZoneId): Boolean {
        // "Train every day of the plan in a week" is meaningless with no plan (build-your-own /
        // freestyle): an empty dayKeys set makes the size check pass and the `all {}` below vacuously
        // true, falsely awarding the trophy on the first session. Require a real plan.
        if (Program.dayKeys.isEmpty()) return false
        if (sessions.size < Program.dayKeys.size) return false
        // Slide a 7-day window; check if all 4 dayKeys appear within it
        val byDate = sessions.groupBy { s ->
            Instant.ofEpochMilli(s.startedAt).atZone(zone).toLocalDate()
        }
        val dates = byDate.keys.sorted()
        for (i in dates.indices) {
            val windowEnd = dates[i].plusDays(6)
            val dayKeysInWindow = dates
                .filter { !it.isBefore(dates[i]) && !it.isAfter(windowEnd) }
                .flatMap { byDate[it]!! }
                .map { it.dayKey }
                .toSet()
            if (Program.dayKeys.all { it in dayKeysInWindow }) return true
        }
        return false
    }
}
