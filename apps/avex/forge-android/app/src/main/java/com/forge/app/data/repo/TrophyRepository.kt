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
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.trophy.TrophyEvaluator
import com.forge.app.domain.trophy.TrophyExercises
import com.forge.app.domain.trophy.TrophyStatsSnapshot
import com.forge.app.program.Program
import com.forge.app.program.Trophies
import com.forge.app.program.Trophy
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
    private val clock: Clock
) {
    fun observeAll(): Flow<List<UnlockedTrophy>> = unlockedDao.observeAll()
    fun observeUnlockedIds(): Flow<List<String>> = unlockedDao.observeUnlockedIds()
    suspend fun unlockedIds(): Set<String> = unlockedDao.unlockedIds().toSet()

    suspend fun unlock(trophyId: String) {
        unlockedDao.unlock(UnlockedTrophy(trophyId = trophyId, unlockedAt = clock.nowMs()))
    }

    suspend fun unlockMany(trophyIds: Collection<String>) {
        val now = clock.nowMs()
        trophyIds.forEach { id ->
            unlockedDao.unlock(UnlockedTrophy(trophyId = id, unlockedAt = now))
        }
    }

    suspend fun snapshot(): TrophyStatsSnapshot {
        val allSessions = sessionDao.allFinished()
        val zone = ZoneId.systemDefault()
        val onVacation = com.forge.app.domain.vacation.VacationCalendar.onVacation(vacationDao.all())
        return TrophyStatsSnapshot(
            totalLoggedExercises = loggedExerciseDao.totalLogged(),
            totalPrs = loggedExerciseDao.prCount(),
            brutalRatings = loggedExerciseDao.countWithRating(EffortRating.BRUTAL),
            swapsUsed = loggedExerciseDao.swapCount(),
            fullTargetHits = loggedExerciseDao.fullTargetCount(),
            finishedSessions = sessionDao.finishedCount(),
            distinctDayKeysTrained = sessionDao.distinctDayKeysTrained().size,
            maxBenchLb = loggedSetDao.maxWeightAcrossExercises(TrophyExercises.BENCH_EXERCISE_IDS) ?: 0.0,
            maxSquatLb = loggedSetDao.maxWeightAcrossExercises(TrophyExercises.SQUAT_EXERCISE_IDS) ?: 0.0,
            maxSessionVolumeLb = loggedSetDao.maxSessionVolume() ?: 0.0,
            maxStreakEver = computeMaxStreak(allSessions, zone, onVacation),
            earlyBirdSessions = countSessionsBefore(allSessions, zone, hour = 7),
            nightOwlSessions = countSessionsAfter(allSessions, zone, hour = 21),
            sundaysTrainedCount = countSundays(allSessions, zone),
            maxSessionDurationMinutes = maxDurationMinutes(allSessions),
            minFinishedSessionDurationMinutes = minDurationMinutes(allSessions),
            maxSingleExerciseReps = loggedSetDao.maxRepsSummedPerExercise() ?: 0,
            comebackKidEarned = checkComebackKid(allSessions, zone),
            consistencyKingEarned = checkConsistencyKing(allSessions, zone),
            varietyPackEarned = checkVarietyPack(allSessions, zone)
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
        if (newlyUnlockedIds.isEmpty()) return emptyList()
        unlockMany(newlyUnlockedIds)
        return Trophies.all.filter { it.id in newlyUnlockedIds }
    }

    fun observeNearMisses() = nearMissDao.observeRecent()

    private suspend fun recordNearMisses(snapshot: TrophyStatsSnapshot, lockedIds: Set<String>) {
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
        sessions.maxOfOrNull { s ->
            val fin = s.finishedAt ?: return@maxOfOrNull 0
            ((fin - s.startedAt) / 60_000).toInt()
        } ?: 0

    private fun minDurationMinutes(sessions: List<Session>): Int {
        val filtered = sessions.filter { s ->
            val fin = s.finishedAt ?: return@filter false
            val mins = ((fin - s.startedAt) / 60_000).toInt()
            s.setCount > 0 && mins >= 5
        }
        return filtered.minOfOrNull { s ->
            ((s.finishedAt!! - s.startedAt) / 60_000).toInt()
        } ?: Int.MAX_VALUE
    }

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
