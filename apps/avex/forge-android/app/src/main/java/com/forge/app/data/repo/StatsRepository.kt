package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.RestDayDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.program.Program
import com.forge.app.data.db.entities.Session
import com.forge.app.ui.gym.stats.state.DayTypeVolumeStats
import com.forge.app.ui.gym.stats.state.WeekActivityRow
import com.forge.app.ui.gym.stats.state.ExerciseFrequency
import com.forge.app.ui.gym.stats.state.ExerciseYoY
import com.forge.app.ui.gym.stats.state.HeatmapCell
import com.forge.app.ui.gym.stats.state.PeriodComparison
import com.forge.app.ui.gym.stats.state.PeriodStats
import com.forge.app.ui.gym.stats.state.HistoryPoint
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.RepMaxSet
import com.forge.app.ui.gym.stats.state.MuscleSetCount
import com.forge.app.ui.gym.stats.state.RepRangeDist
import com.forge.app.ui.gym.stats.state.RpeBucket
import com.forge.app.ui.gym.stats.state.DayTypeBreakdown
import com.forge.app.ui.gym.stats.state.InsightFlag
import com.forge.app.ui.gym.stats.state.LifetimeMetrics
import com.forge.app.ui.gym.stats.state.VolumePoint
import com.forge.app.ui.gym.stats.state.MonthCalendarData
import com.forge.app.ui.gym.stats.state.MuscleVolume
import com.forge.app.ui.gym.stats.state.PrEntry
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.gym.stats.state.SessionDaySummary
import com.forge.app.ui.gym.stats.state.StrengthCurve
import com.forge.app.ui.gym.stats.state.TimeToPrEntry
import com.forge.app.ui.gym.stats.state.Totals
import com.forge.app.ui.gym.stats.state.VolumeDeloadPoint
import com.forge.app.ui.gym.stats.state.WeeklyEffortCounts
import com.forge.app.ui.overview.state.OnThisDayMemory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Aggregates the rolling-window stats that feed the Overview screen.
 *
 * The "since" parameter for each weekly query is computed once at flow subscription
 * time. That means if the user keeps the app open for a week the window won't slide,
 * but that's a non-issue for a personal app — they'll close and reopen.
 *
 * The pure (DAO-free) aggregation helpers used by [observeGymStats] live in sibling
 * files: StatsStrengthAggregations.kt, StatsVolumeAggregations.kt, and
 * StatsEffortAggregations.kt. The two comparison helpers below stay here because they
 * query the DAO.
 */
@Singleton
class StatsRepository @Inject constructor(
    private val sessionDao: SessionDao,
    private val cardioDao: CardioDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val restDayDao: RestDayDao,
    private val bodyweightRepo: BodyweightRepository,
    private val adaptationRepo: AdaptationRepository,
    private val clock: Clock
) {

    data class WeeklyStats(
        val workouts: Int = 0,
        val volumeLb: Double = 0.0,
        val cardioMinutes: Int = 0,
        val totalFinishedSessions: Int = 0,
        val streakDays: Int = 0,
        val daysSinceLastSession: Int? = null,
        val firstFinishedSessionMs: Long? = null,
        /** 0=Mon..6=Sun indices that had a finished gym session in the current ISO week. */
        val weekDaysTrained: Set<Int> = emptySet(),
        /** Next gym day key in the rotation (Upper A → Lower A → Upper B → Lower B). */
        val nextUpDayKey: String = Program.UPPER_A,
        /** Last 5 finished gym sessions for the overview RECENT section. */
        val recentGymSessions: List<com.forge.app.data.db.entities.Session> = emptyList()
    )

    fun observeWeeklyStats(): Flow<WeeklyStats> {
        val weekStartMs = clock.nowMs() - WEEK_MS
        val baseFlow = combine(
            sessionDao.observeFinishedCountSince(weekStartMs),
            sessionDao.observeVolumeSince(weekStartMs),
            cardioDao.observeMinutesSince(weekStartMs, excludeType = "rest"),
            sessionDao.observeFinishedCount(),
            sessionDao.observeRecent(120)
        ) { workouts, volume, cardio, totalFinished, recentSessions ->
            val zone = ZoneId.systemDefault()
            val todayDate = LocalDate.now(zone)
            val isoWeekStart = todayDate.minusDays(todayDate.dayOfWeek.value.toLong() - 1)
            val isoWeekStartMs = isoWeekStart.atStartOfDay(zone).toInstant().toEpochMilli()
            val finishedAts = recentSessions.mapNotNull { it.finishedAt }
            val weekDaysTrained = recentSessions
                .filter { it.finishedAt != null && it.finishedAt >= isoWeekStartMs }
                .map {
                    // Bucket by the same timestamp the week filter uses (finishedAt), so a session
                    // that started before midnight but finished this week lands on the right day.
                    val d = Instant.ofEpochMilli(it.finishedAt!!).atZone(zone).toLocalDate()
                    d.dayOfWeek.value - 1 // 0=Mon..6=Sun
                }
                .toSet()
            val lastFinished = recentSessions.filter { it.finishedAt != null }.maxByOrNull { it.finishedAt!! }
            val nextUpDayKey = if (lastFinished == null) (Program.dayKeys.firstOrNull() ?: Program.UPPER_A)
                else { val idx = Program.dayKeys.indexOf(lastFinished.dayKey); Program.dayKeys[(idx + 1) % Program.dayKeys.size] }
            WeeklyStats(
                workouts = workouts,
                volumeLb = volume ?: 0.0,
                cardioMinutes = cardio ?: 0,
                totalFinishedSessions = totalFinished,
                streakDays = computeStreak(finishedAts),
                daysSinceLastSession = computeDaysSinceLast(finishedAts),
                weekDaysTrained = weekDaysTrained,
                nextUpDayKey = nextUpDayKey,
                recentGymSessions = recentSessions.filter { it.finishedAt != null }.take(5)
            )
        }
        return baseFlow.combine(sessionDao.observeFirstFinishedSessionStartedAt()) { stats, firstMs ->
            stats.copy(firstFinishedSessionMs = firstMs)
        }.flowOn(Dispatchers.Default)
    }

    private fun computeStreak(finishedAts: List<Long>): Int {
        if (finishedAts.isEmpty()) return 0
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        val trainingDays = finishedAts.mapTo(mutableSetOf()) {
            Instant.ofEpochMilli(it).atZone(zone).toLocalDate()
        }
        val startDay = when {
            trainingDays.contains(today) -> today
            trainingDays.contains(today.minusDays(1)) -> today.minusDays(1)
            else -> return 0
        }
        var streak = 0
        var day = startDay
        while (trainingDays.contains(day)) {
            streak++
            day = day.minusDays(1)
        }
        return streak
    }

    private fun computeDaysSinceLast(finishedAts: List<Long>): Int? {
        val latest = finishedAts.maxOrNull() ?: return null
        val zone = ZoneId.systemDefault()
        val lastDay = Instant.ofEpochMilli(latest).atZone(zone).toLocalDate()
        return ChronoUnit.DAYS.between(lastDay, LocalDate.now(zone)).toInt()
    }

    // ─── History / comparison helpers ─────────────────────────────────────────

    fun observeAllFinishedSessions(): Flow<List<Session>> = sessionDao.observeAllFinishedSessions()

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    fun observeDayVolumeStats(): Flow<Map<String, SessionDao.DayVolumeStats>> =
        sessionDao.observeFinishedCount()
            .flatMapLatest { flow { emit(sessionDao.avgMaxVolumeByDayKey().associateBy { it.dayKey }) } }

    data class SessionExerciseLine(
        val exerciseName: String,
        val topWeightLb: Double?,
        val topReps: Int?,
        val setCount: Int
    )

    suspend fun getSessionExerciseLines(sessionId: Long): List<SessionExerciseLine> {
        val exercises = loggedExerciseDao.forSession(sessionId)
        val allSets = loggedSetDao.allForSession(sessionId)
        val setsByExId = allSets.groupBy { it.loggedExerciseId }
        return exercises.mapNotNull { ex ->
            if (ex.skipped) return@mapNotNull null
            val sets = setsByExId[ex.id] ?: emptyList()
            if (sets.isEmpty()) return@mapNotNull null
            val topSet = sets.maxByOrNull { it.weightLb ?: 0.0 }
            val name = ex.swappedName
                ?: Program.days.flatMap { it.exercises }.firstOrNull { it.id == ex.exerciseId }?.name
                ?: ex.exerciseId
            SessionExerciseLine(
                exerciseName = name,
                topWeightLb = topSet?.weightLb,
                topReps = topSet?.reps,
                setCount = sets.size
            )
        }
    }

    // ─── Gym stats subtab ──────────────────────────────────────────────────────

    data class GymStats(
        val totals: Totals,
        val heatmap: List<HeatmapCell>,
        val volumeByMuscle: List<MuscleVolume>,
        val strengthCurve: StrengthCurve?,
        val recentPrs: List<PrEntry>,
        val hallOfFame: List<PrRecord>,
        val exerciseHistory: Map<String, List<HistoryPoint>>,
        /** Volume (lb) per session per exercise — for #72 volume over time chart. */
        val exerciseVolumeHistory: Map<String, List<VolumePoint>> = emptyMap(),
        /** Max weight per exercise for the radar chart (#124): exerciseId → max lb. */
        val compoundMaxes: Map<String, Double> = emptyMap(),
        /** PR session timestamps for clustering scatter (#128). */
        val prSessionTimestamps: List<Long> = emptyList(),
        val exerciseFrequency: List<ExerciseFrequency> = emptyList(),
        val timeToPr: List<TimeToPrEntry> = emptyList(),
        val effortDistribution: List<WeeklyEffortCounts> = emptyList(),
        val prsByDayOfWeek: List<Int> = List(7) { 0 },
        val volumeDeloadTrend: List<VolumeDeloadPoint> = emptyList(),
        val dayTypeBestVsAvg: List<DayTypeVolumeStats> = emptyList(),
        val weekComparison: PeriodComparison? = null,
        val monthComparison: PeriodComparison? = null,
        val exerciseYoY: List<ExerciseYoY> = emptyList(),
        val insights: List<InsightFlag> = emptyList(),
        val dayTypeBreakdown: List<DayTypeBreakdown> = emptyList(),
        val lifetimeMetrics: LifetimeMetrics? = null,
        val moodOverTime: List<com.forge.app.data.db.dao.SessionDao.MoodOverTime> = emptyList(),
        /** Raw sessions this ISO week — used internally to build [weekActivity]. */
        val weekSessions: List<Session> = emptyList(),
        val weekActivity: List<WeekActivityRow> = emptyList(),
        val thisWeekCardioMin: Int = 0,
        val e1rmLifts: List<E1rmLift> = emptyList(),
        val repMaxes: RepMaxSet? = null,
        val weeklySetsByMuscle: List<MuscleSetCount> = emptyList(),
        val repRangeDist: RepRangeDist? = null,
        val rpeDistribution: List<RpeBucket> = emptyList(),
        val avgRpe: Double? = null,
        val bodyweightTrend: List<Double> = emptyList(),
        val consistencyStreakWeeks: Int = 0,
        val progressiveOverloadPct: Double? = null,
        val avgRpePerSession: List<Double> = emptyList(),
        val weeklySessionCounts: List<Int> = emptyList()
    )

    /**
     * Single observable feeding the Gym → Stats subtab. Aggregates several DAO flows
     * into the snapshot the UI consumes. Heatmap window and curve exercise are fixed
     * here; ideally configurable later via settings.
     */
    fun observeGymStats(): Flow<GymStats> {
        val heatmapStartMs = clock.nowMs() - HEATMAP_WINDOW_MS
        val volumeStartMs = clock.nowMs() - WEEK_MS
        val zone = ZoneId.systemDefault()
        val isoWeekStart = run {
            val today = LocalDate.now(zone)
            today.minusDays(today.dayOfWeek.value.toLong() - 1)
        }
        val weekStartMs = isoWeekStart.atStartOfDay(zone).toInstant().toEpochMilli()
        val weekEndMs = isoWeekStart.plusWeeks(1).atStartOfDay(zone).toInstant().toEpochMilli()

        val totalsFlow: Flow<Totals> = combine(
            sessionDao.observeFinishedCount(),
            loggedExerciseDao.observeTotalLogged(),
            loggedExerciseDao.observePrCount()
        ) { workouts, exercises, prs ->
            Totals(workouts = workouts, exercisesLogged = exercises, prs = prs)
        }

        val eightWeeksMs = clock.nowMs() - 8L * 7 * 24 * 60 * 60 * 1000

        val moodFlow = sessionDao.observeMoodOverTime()

        return combine(
            totalsFlow,
            loggedExerciseDao.observeHeatmapTimestamps(heatmapStartMs),
            loggedSetDao.observeSetsSinceWithExerciseId(volumeStartMs),
            loggedSetDao.observeAllFinishedSetsWithSession(),
            loggedExerciseDao.observeRecentPrs()
        ) { totals, heatmapRows, volumeSets, allSets, prRows ->
          coroutineScope {
            // The independent aggregate queries below ran one-by-one, so their latencies
            // stacked. Fire them concurrently and await — cuts the Stats-tab open time.
            val freqRowsD = async { loggedExerciseDao.frequencySince(eightWeeksMs) }
            val prDatesD = async { loggedExerciseDao.prDatesPerExercise() }
            val effortRowsD = async { loggedExerciseDao.effortRatingsSince(eightWeeksMs) }
            val prTimesD = async { sessionDao.prSessionStartTimes() }
            val deloadRowsD = async { sessionDao.allFinishedVolumeDeload() }
            val dayStatsD = async { sessionDao.avgMaxVolumeByDayKey() }
            val latestBwLbD = async { bodyweightRepo.latestWeightLb() }
            val weekCompD = async { buildWeekComparison() }
            val monthCompD = async { buildMonthComparison() }
            val lifetimeAggD = async { sessionDao.lifetimeAggregate() }
            val dayTypeRowsD = async { sessionDao.perDayTypeStats() }

            val freqRows = freqRowsD.await()
            val prDates = prDatesD.await()
            val effortRows = effortRowsD.await()
            val prTimes = prTimesD.await()
            val deloadRows = deloadRowsD.await()
            val dayStats = dayStatsD.await()
            val latestBwLb = latestBwLbD.await()
            val weekComp = weekCompD.await()
            val monthComp = monthCompD.await()
            val yoy = buildExerciseYoY(allSets)
            val lifetimeAgg = lifetimeAggD.await()
            val lifetimeMetrics = LifetimeMetrics(
                lifetimeVolumeLb = lifetimeAgg.totalVolume ?: 0.0,
                totalSessions = lifetimeAgg.sessionCount,
                avgSessionVolumeLb = if (lifetimeAgg.sessionCount > 0) (lifetimeAgg.totalVolume ?: 0.0) / lifetimeAgg.sessionCount else 0.0,
                avgSetCount = lifetimeAgg.avgSets ?: 0.0
            )
            val dayTypeRows = dayTypeRowsD.await()
            val dayTypeBreakdown = buildDayTypeBreakdown(dayTypeRows)
            // Insights come from the adaptation engine (System 4) — buildInsights' rules
            // moved there with snapshot-wide gating; the UI keeps rendering InsightFlag rows.
            val insights = adaptationRepo.insights().map { InsightFlag(it.icon, it.title, it.body) }
            val e1lifts = buildE1rmLifts(allSets)
            GymStats(
                totals = totals,
                heatmap = buildHeatmap(heatmapRows.map { it.startedAt }),
                volumeByMuscle = buildVolumeByMuscle(volumeSets),
                strengthCurve = buildStrengthCurveFor(STRENGTH_CURVE_EXERCISE_ID, allSets),
                recentPrs = buildPrEntries(prRows, allSets),
                hallOfFame = buildHallOfFame(allSets, latestBwLb),
                exerciseHistory = buildExerciseHistory(allSets),
                exerciseVolumeHistory = buildExerciseVolumeHistory(allSets),
                compoundMaxes = buildCompoundMaxes(allSets),
                e1rmLifts = e1lifts,
                repMaxes = buildRepMaxes(allSets),
                weeklySetsByMuscle = buildWeeklySetsByMuscle(volumeSets),
                repRangeDist = buildRepRangeDist(allSets),
                rpeDistribution = buildRpeDistribution(allSets),
                avgRpe = allSets.mapNotNull { it.rpe }.takeIf { it.isNotEmpty() }?.average(),
                consistencyStreakWeeks = computeConsistencyStreak(allSets),
                progressiveOverloadPct = computeProgressiveOverload(e1lifts),
                avgRpePerSession = buildAvgRpePerSession(allSets),
                weeklySessionCounts = buildWeeklySessionCounts(allSets),
                prSessionTimestamps = prTimes,
                exerciseFrequency = buildExerciseFrequency(freqRows),
                timeToPr = buildTimeToPr(prDates),
                effortDistribution = buildEffortDistribution(effortRows),
                prsByDayOfWeek = buildPrsByDayOfWeek(prTimes),
                volumeDeloadTrend = buildVolumeDeloadTrend(deloadRows),
                dayTypeBestVsAvg = buildDayTypeBestVsAvg(dayStats),
                weekComparison = weekComp,
                monthComparison = monthComp,
                exerciseYoY = yoy,
                insights = insights,
                dayTypeBreakdown = dayTypeBreakdown,
                lifetimeMetrics = lifetimeMetrics
            )
          }
        }.combine(moodFlow) { stats, moods ->
            stats.copy(moodOverTime = moods)
        }.combine(sessionDao.observeFinishedInRange(weekStartMs, weekEndMs)) { stats, sessions ->
            stats.copy(weekSessions = sessions)
        }.combine(cardioDao.observeSince(weekStartMs)) { stats, cardioEntries ->
            val nonRest = cardioEntries.filter { it.type != "rest" }
            stats.copy(
                weekActivity = buildWeekActivity(stats.weekSessions, nonRest),
                thisWeekCardioMin = nonRest.sumOf { it.durationMin }
            )
        }.combine(bodyweightRepo.observeRecent(90)) { stats, bw ->
            // observeRecent is newest-first; reverse to chronological for the trend chart.
            stats.copy(bodyweightTrend = bw.reversed().map { it.weightLb })
        }.flowOn(Dispatchers.Default)
    }

    /** Monthly calendar: reactive stream of sessions + rest days in the current calendar month (#54 #114 #115). */
    fun observeMonthCalendar(): Flow<MonthCalendarData> {
        val zone = ZoneId.systemDefault()
        val month = YearMonth.now(zone)
        val fromMs = month.atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val toMs = month.plusMonths(1).atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        return sessionDao.observeFinishedInRange(fromMs, toMs)
            .combine(restDayDao.observeAll()) { sessions, restEntries ->
                val monthPrefix = month.toString() // "yyyy-MM"
                MonthCalendarData(
                    yearMonth = month,
                    sessionDays = sessions.associate { session ->
                        val day = Instant.ofEpochMilli(session.startedAt).atZone(zone).dayOfMonth
                        val dayName = Program.days.firstOrNull { it.key == session.dayKey }?.defaultName
                            ?: session.dayKey
                        day to SessionDaySummary(
                            dayName = dayName,
                            totalVolumeLb = session.totalVolumeLb ?: 0.0,
                            prCount = session.prCount
                        )
                    },
                    restDays = restEntries
                        .filter { it.dateKey.startsWith(monthPrefix) }
                        .associate { entry ->
                            entry.dateKey.substringAfterLast("-").toIntOrNull()?.let { it to entry.type } ?: (0 to "")
                        }
                        .filter { it.key > 0 }
                )
            }
    }

    /**
     * Looks for a session from 1, 3, 6, or 12 months ago (±3-day window). Returns the
     * most distant qualifying match so "1 year ago" takes precedence over "1 month ago". (#106)
     */
    suspend fun findOnThisDayMemory(): OnThisDayMemory? {
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        for (months in listOf(12, 6, 3, 1)) {
            val target = today.minusMonths(months.toLong())
            val targetMs = target.atStartOfDay(zone).toInstant().toEpochMilli()
            val windowMs = 3L * 24 * 60 * 60 * 1000
            val session = sessionDao.sessionNearDate(
                targetMs = targetMs + 12 * 60 * 60 * 1000,
                fromMs = targetMs - windowMs, // ±3 days around the target date (was forward-only)
                toMs = targetMs + windowMs
            ) ?: continue
            val dayName = Program.days.firstOrNull { it.key == session.dayKey }?.defaultName ?: session.dayKey
            return OnThisDayMemory(
                monthsAgo = months,
                dayName = dayName,
                totalVolumeLb = session.totalVolumeLb ?: 0.0,
                prCount = session.prCount,
                sessionDate = session.startedAt
            )
        }
        return null
    }

    private suspend fun buildWeekComparison(): PeriodComparison? {
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        val thisWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
        val lastWeekStart = thisWeekStart.minusWeeks(1)
        val toMs = { d: LocalDate -> d.atStartOfDay(zone).toInstant().toEpochMilli() }
        val thisW = sessionDao.aggregateInRange(toMs(thisWeekStart), toMs(thisWeekStart.plusWeeks(1)))
        val lastW = sessionDao.aggregateInRange(toMs(lastWeekStart), toMs(thisWeekStart))
        if (lastW.sessionCount == 0 && thisW.sessionCount == 0) return null
        return PeriodComparison(
            label = "WEEK",
            current = PeriodStats(thisW.sessionCount, thisW.totalVolume ?: 0.0, thisW.totalPrs, thisW.totalSets),
            previous = PeriodStats(lastW.sessionCount, lastW.totalVolume ?: 0.0, lastW.totalPrs, lastW.totalSets)
        )
    }

    private suspend fun buildMonthComparison(): PeriodComparison? {
        val zone = ZoneId.systemDefault()
        val now = YearMonth.now(zone)
        val thisMonthStart = now.atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val nextMonthStart = now.plusMonths(1).atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val lastMonthStart = now.minusMonths(1).atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        // Exact month boundary (was thisMonthStart + 32 days, which always bled 1-2 days into next month).
        val thisM = sessionDao.aggregateInRange(thisMonthStart, nextMonthStart)
        val lastM = sessionDao.aggregateInRange(lastMonthStart, thisMonthStart)
        if (lastM.sessionCount == 0 && thisM.sessionCount == 0) return null
        return PeriodComparison(
            label = "MONTH",
            current = PeriodStats(thisM.sessionCount, thisM.totalVolume ?: 0.0, thisM.totalPrs, thisM.totalSets),
            previous = PeriodStats(lastM.sessionCount, lastM.totalVolume ?: 0.0, lastM.totalPrs, lastM.totalSets)
        )
    }

    companion object {
        private const val WEEK_MS: Long = 7L * 24 * 60 * 60 * 1000
        /** DB Bench Press — Antho's main upper-body lift, shown as the default curve. */
        private const val STRENGTH_CURVE_EXERCISE_ID = "ua1"
    }
}
