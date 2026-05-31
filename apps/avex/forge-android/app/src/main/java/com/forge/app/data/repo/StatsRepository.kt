package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.RestDayDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.program.Program
import com.forge.app.data.db.entities.CardioEntry
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
import com.forge.app.ui.gym.stats.state.RepMaxEntry
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
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
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
 */
@Singleton
class StatsRepository @Inject constructor(
    private val sessionDao: SessionDao,
    private val cardioDao: CardioDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val restDayDao: RestDayDao,
    private val bodyweightRepo: BodyweightRepository,
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
                    val d = Instant.ofEpochMilli(it.startedAt).atZone(zone).toLocalDate()
                    d.dayOfWeek.value - 1 // 0=Mon..6=Sun
                }
                .toSet()
            val lastFinished = recentSessions.filter { it.finishedAt != null }.maxByOrNull { it.finishedAt!! }
            val nextUpDayKey = if (lastFinished == null) Program.UPPER_A
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
        }
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
            val freqRows = loggedExerciseDao.frequencySince(eightWeeksMs)
            val prDates = loggedExerciseDao.prDatesPerExercise()
            val effortRows = loggedExerciseDao.effortRatingsSince(eightWeeksMs)
            val prTimes = sessionDao.prSessionStartTimes()
            val deloadRows = sessionDao.allFinishedVolumeDeload()
            val dayStats = sessionDao.avgMaxVolumeByDayKey()
            val latestBwLb = bodyweightRepo.latestWeightLb()
            val weekComp = buildWeekComparison()
            val monthComp = buildMonthComparison()
            val yoy = buildExerciseYoY(allSets)
            val lifetimeAgg = sessionDao.lifetimeAggregate()
            val lifetimeMetrics = LifetimeMetrics(
                lifetimeVolumeLb = lifetimeAgg.totalVolume ?: 0.0,
                totalSessions = lifetimeAgg.sessionCount,
                avgSessionVolumeLb = if (lifetimeAgg.sessionCount > 0) (lifetimeAgg.totalVolume ?: 0.0) / lifetimeAgg.sessionCount else 0.0,
                avgSetCount = lifetimeAgg.avgSets ?: 0.0
            )
            val dayTypeRows = sessionDao.perDayTypeStats()
            val dayTypeBreakdown = buildDayTypeBreakdown(dayTypeRows)
            val insights = buildInsights(allSets, volumeSets, dayTypeRows)
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
        }
    }

    // ─── Aggregation helpers ──────────────────────────────────────────────────

    private fun buildHeatmap(timestamps: List<Long>): List<HeatmapCell> {
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        val countsByDate: Map<LocalDate, Int> = timestamps
            .groupingBy { Instant.ofEpochMilli(it).atZone(zone).toLocalDate() }
            .eachCount()
        // Order oldest → newest so the UI can lay it out as 7 rows × 7 cols
        return (HEATMAP_DAYS - 1 downTo 0).map { offset ->
            val date = today.minusDays(offset.toLong())
            HeatmapCell(date = date, count = countsByDate[date] ?: 0)
        }
    }

    private fun buildVolumeByMuscle(
        sets: List<com.forge.app.data.db.projections.SetWithExerciseId>
    ): List<MuscleVolume> {
        val byMuscle = mutableMapOf<com.forge.app.program.MuscleGroup, Double>()
        sets.forEach { s ->
            val plan = Program.exercise(s.exerciseId) ?: return@forEach
            val volume = (s.weightLb ?: 0.0) * s.reps
            byMuscle.merge(plan.muscle, volume, Double::plus)
        }
        return byMuscle
            .map { (muscle, volume) -> MuscleVolume(muscle = muscle, volumeLb = volume) }
            .sortedByDescending { it.volumeLb }
    }

    private fun buildStrengthCurveFor(
        exerciseId: String,
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): StrengthCurve? {
        val plan = Program.exercise(exerciseId) ?: return null
        val maxPerSession = allSets
            .filter { it.exerciseId == exerciseId && it.weightLb != null }
            .groupBy { it.sessionStartedAt }
            .map { (_, sessionSets) -> sessionSets.maxOf { it.weightLb!! } }
            .takeLast(STRENGTH_CURVE_MAX_POINTS)
        if (maxPerSession.isEmpty()) return null
        return StrengthCurve(plan = plan, points = maxPerSession)
    }

    private fun buildPrEntries(
        rows: List<com.forge.app.data.db.projections.RecentPrRow>,
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): List<PrEntry> {
        // Group sets by sessionStartedAt + exerciseId so we can look up the PR set per row.
        // (LoggedExercise row uniquely identifies session+exercise; we approximate via
        // session date which is good enough for display purposes here.)
        return rows.map { row ->
            val candidateSets = allSets.filter {
                it.exerciseId == row.exerciseId && it.sessionStartedAt == row.sessionStartedAt
            }
            val prSet = candidateSets.maxByOrNull { it.weightLb ?: 0.0 }
            val name = row.swappedName
                ?: Program.exercise(row.exerciseId)?.name
                ?: row.exerciseId
            PrEntry(
                date = row.sessionStartedAt,
                exerciseName = name,
                weightText = prSet?.weightLb?.let { "${it.toInt()} lb" } ?: "—",
                reps = prSet?.reps ?: 0
            )
        }
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
                fromMs = targetMs,
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

    private fun buildHallOfFame(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>,
        bodyweightLb: Double? = null
    ): List<PrRecord> {
        return allSets
            .filter { it.weightLb != null }
            .groupBy { it.exerciseId }
            .mapNotNull { (exerciseId, sets) ->
                val plan = Program.exercise(exerciseId) ?: return@mapNotNull null
                val bestSet = sets.maxByOrNull { it.weightLb!! } ?: return@mapNotNull null
                val rel = if (bodyweightLb != null && bodyweightLb > 0)
                    (bestSet.weightLb!! / bodyweightLb * 10).toInt() / 10.0
                else null
                PrRecord(
                    exerciseId = exerciseId,
                    exerciseName = plan.name,
                    maxWeightLb = bestSet.weightLb!!,
                    bestReps = bestSet.reps,
                    sessionDate = bestSet.sessionStartedAt,
                    muscle = plan.muscle,
                    relativeStrength = rel
                )
            }
            .sortedWith(compareBy({ it.muscle.displayName }, { it.exerciseName }))
    }

    private fun buildExerciseHistory(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): Map<String, List<HistoryPoint>> {
        return allSets
            .filter { it.weightLb != null }
            .groupBy { it.exerciseId }
            .mapValues { (_, sets) ->
                sets
                    .groupBy { it.sessionStartedAt }
                    .map { (sessionDate, sessionSets) ->
                        HistoryPoint(
                            sessionDate = sessionDate,
                            maxWeightLb = sessionSets.maxOf { it.weightLb!! }
                        )
                    }
                    .sortedBy { it.sessionDate }
            }
    }

    /** Epley estimated 1-rep max. */
    private fun e1rm(weightLb: Double, reps: Int): Double = weightLb * (1.0 + reps / 30.0)

    /** Per-lift estimated-1RM progression: best e1RM per session, with growth rate + stall flag. */
    private fun buildE1rmLifts(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): List<E1rmLift> {
        return allSets
            .filter { it.weightLb != null && it.weightLb > 0 }
            .groupBy { it.exerciseId }
            .mapNotNull { (id, sets) ->
                val name = Program.exercise(id)?.name ?: return@mapNotNull null
                val perSession = sets.groupBy { it.sessionStartedAt }.toSortedMap()
                val points = perSession.map { (_, ss) -> ss.maxOf { e1rm(it.weightLb!!, it.reps) } }
                if (points.isEmpty()) return@mapNotNull null
                val dates = perSession.keys.toList()
                val first = points.first()
                val current = points.last()
                val monthlyPct = if (points.size >= 2 && first > 0) {
                    val months = ((dates.last() - dates.first()) / (30.44 * 24 * 60 * 60 * 1000)).coerceAtLeast(0.5)
                    (current - first) / first / months * 100.0
                } else null
                val stalling = points.size >= 3 && run {
                    val recent = points.takeLast(3)
                    val hi = recent.max()
                    hi > 0 && (hi - recent.min()) / hi < 0.01
                }
                E1rmLift(
                    exerciseId = id, exerciseName = name, currentE1rm = current,
                    history = points, monthlyPct = monthlyPct, stalling = stalling
                )
            }
            .sortedByDescending { it.currentE1rm }
            .take(6)
    }

    /** Average RPE per finished session, oldest → newest (only sessions that recorded RPE). */
    private fun buildAvgRpePerSession(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): List<Double> {
        return allSets
            .filter { it.rpe != null }
            .groupBy { it.sessionStartedAt }
            .toSortedMap()
            .map { (_, ss) -> ss.mapNotNull { it.rpe }.average() }
    }

    /** Session count per ISO week for the last 12 weeks, oldest → newest. */
    private fun buildWeeklySessionCounts(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): List<Int> {
        val zone = ZoneId.systemDefault()
        val byWeek = allSets.map { it.sessionStartedAt }.distinct()
            .groupingBy {
                val d = Instant.ofEpochMilli(it).atZone(zone).toLocalDate()
                d.minusDays(d.dayOfWeek.value.toLong() - 1)
            }
            .eachCount()
        val now = LocalDate.now(zone)
        val thisWeek = now.minusDays(now.dayOfWeek.value.toLong() - 1)
        return (11 downTo 0).map { i -> byWeek[thisWeek.minusWeeks(i.toLong())] ?: 0 }
    }

    /** Best weight at each rep count for the single most-trained lift. */
    private fun buildRepMaxes(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): RepMaxSet? {
        val byExercise = allSets.filter { it.weightLb != null && it.weightLb > 0 }.groupBy { it.exerciseId }
        val top = byExercise.maxByOrNull { it.value.size } ?: return null
        val name = Program.exercise(top.key)?.name ?: return null
        val entries = top.value
            .groupBy { it.reps }
            .map { (reps, ss) -> RepMaxEntry(reps = reps, weightLb = ss.maxOf { it.weightLb!! }) }
            .sortedBy { it.reps }
        return if (entries.isEmpty()) null else RepMaxSet(exerciseName = name, entries = entries)
    }

    /** Consecutive recent weeks (incl. an in-progress current week) hitting the session target. */
    private fun computeConsistencyStreak(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): Int {
        if (allSets.isEmpty()) return 0
        val zone = ZoneId.systemDefault()
        val sessionsPerWeek = allSets.map { it.sessionStartedAt }.distinct()
            .groupingBy {
                val d = Instant.ofEpochMilli(it).atZone(zone).toLocalDate()
                d.minusDays(d.dayOfWeek.value.toLong() - 1) // ISO week start (Monday)
            }
            .eachCount()
        val now = LocalDate.now(zone)
        val thisWeek = now.minusDays(now.dayOfWeek.value.toLong() - 1)
        var streak = 0
        for (i in 0 until 52) {
            val w = thisWeek.minusWeeks(i.toLong())
            val count = sessionsPerWeek[w] ?: 0
            when {
                count >= CONSISTENCY_TARGET -> streak++
                i == 0 -> {} // current week may still be in progress — don't break the streak
                else -> return streak
            }
        }
        return streak
    }

    /** Average estimated-1RM growth per month across lifts, as a percent. */
    private fun computeProgressiveOverload(lifts: List<E1rmLift>): Double? =
        lifts.mapNotNull { it.monthlyPct }.takeIf { it.isNotEmpty() }?.average()

    /** Working sets per muscle group in the current rolling week (#volume landmarks). */
    private fun buildWeeklySetsByMuscle(
        sets: List<com.forge.app.data.db.projections.SetWithExerciseId>
    ): List<MuscleSetCount> {
        val byMuscle = mutableMapOf<com.forge.app.program.MuscleGroup, Int>()
        sets.forEach { s ->
            val plan = Program.exercise(s.exerciseId) ?: return@forEach
            byMuscle.merge(plan.muscle, 1, Int::plus)
        }
        return byMuscle.map { (m, n) -> MuscleSetCount(muscle = m, sets = n) }
            .sortedByDescending { it.sets }
    }

    /** Split all logged sets into strength (1–5) / hypertrophy (6–12) / endurance (13+). */
    private fun buildRepRangeDist(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): RepRangeDist? {
        if (allSets.isEmpty()) return null
        var s = 0; var h = 0; var e = 0
        allSets.forEach {
            when {
                it.reps <= 5 -> s++
                it.reps <= 12 -> h++
                else -> e++
            }
        }
        return RepRangeDist(strength = s, hypertrophy = h, endurance = e)
    }

    /** Count of sets logged at each RPE value (only sets where RPE was recorded). */
    private fun buildRpeDistribution(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): List<RpeBucket> {
        return allSets.mapNotNull { it.rpe }
            .groupingBy { it }
            .eachCount()
            .map { (rpe, count) -> RpeBucket(rpe = rpe, count = count) }
            .sortedBy { it.rpe }
    }

    private fun buildExerciseFrequency(
        rows: List<com.forge.app.data.db.dao.LoggedExerciseDao.ExerciseFreqRow>
    ): List<ExerciseFrequency> {
        val maxSessions = rows.maxOfOrNull { it.sessionCount } ?: 1
        return rows.mapNotNull { row ->
            val name = Program.exercise(row.exerciseId)?.name ?: return@mapNotNull null
            ExerciseFrequency(
                exerciseId = row.exerciseId,
                exerciseName = name,
                sessionCount = row.sessionCount,
                outOf = maxSessions
            )
        }.sortedByDescending { it.sessionCount }.take(10)
    }

    private fun buildTimeToPr(
        rows: List<com.forge.app.data.db.dao.LoggedExerciseDao.ExercisePrDate>
    ): List<TimeToPrEntry> {
        return rows
            .groupBy { it.exerciseId }
            .mapNotNull { (exerciseId, dates) ->
                if (dates.size < 2) return@mapNotNull null
                val sorted = dates.sortedBy { it.sessionDate }
                val avgMs = sorted.zipWithNext { a, b -> b.sessionDate - a.sessionDate }.average()
                val avgDays = (avgMs / (24 * 60 * 60 * 1000)).toInt().coerceAtLeast(1)
                val name = Program.exercise(exerciseId)?.name ?: return@mapNotNull null
                TimeToPrEntry(exerciseId = exerciseId, exerciseName = name, avgDaysBetween = avgDays, prCount = dates.size)
            }
            .sortedBy { it.avgDaysBetween }
    }

    private fun buildEffortDistribution(
        rows: List<com.forge.app.data.db.dao.LoggedExerciseDao.EffortWithDate>
    ): List<WeeklyEffortCounts> {
        val zone = ZoneId.systemDefault()
        val grouped = rows.groupBy { row ->
            val date = Instant.ofEpochMilli(row.sessionDate).atZone(zone).toLocalDate()
            date.minusDays(date.dayOfWeek.value.toLong() - 1) // ISO week start Monday
        }
        return grouped.entries
            .sortedBy { it.key }
            .takeLast(8)
            .map { (weekStart, weekRows) ->
                val label = weekStart.toString().substring(5) // "MM-dd"
                WeeklyEffortCounts(
                    weekLabel = label,
                    easy = weekRows.count { it.difficulty == "EASY" },
                    justRight = weekRows.count { it.difficulty == "JUST_RIGHT" },
                    hard = weekRows.count { it.difficulty == "HARD" },
                    brutal = weekRows.count { it.difficulty == "BRUTAL" }
                )
            }
    }

    private fun buildPrsByDayOfWeek(prTimes: List<Long>): List<Int> {
        val zone = ZoneId.systemDefault()
        val counts = IntArray(7)
        prTimes.forEach { ms ->
            val dow = Instant.ofEpochMilli(ms).atZone(zone).dayOfWeek.value - 1 // 0=Mon
            counts[dow]++
        }
        return counts.toList()
    }

    private fun buildVolumeDeloadTrend(
        rows: List<com.forge.app.data.db.dao.SessionDao.SessionVolumeDeloadRow>
    ): List<VolumeDeloadPoint> {
        return rows
            .filter { it.totalVolumeLb != null && (it.totalVolumeLb ?: 0.0) > 0 }
            .takeLast(30)
            .map { row ->
                VolumeDeloadPoint(
                    sessionDate = row.startedAt,
                    dayKey = row.dayKey,
                    totalVolumeLb = row.totalVolumeLb ?: 0.0,
                    isDeload = row.deloadMarkedHere
                )
            }
    }

    private fun buildDayTypeBestVsAvg(
        rows: List<com.forge.app.data.db.dao.SessionDao.DayVolumeStats>
    ): List<DayTypeVolumeStats> {
        return rows.mapNotNull { row ->
            val dayName = Program.days.firstOrNull { it.key == row.dayKey }?.defaultName ?: return@mapNotNull null
            DayTypeVolumeStats(
                dayKey = row.dayKey,
                dayName = dayName,
                avgVolumeLb = row.avgVolume,
                maxVolumeLb = row.maxVolume,
                sessionCount = row.sessionCount
            )
        }.sortedBy { it.dayKey }
    }

    private fun buildExerciseVolumeHistory(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): Map<String, List<VolumePoint>> {
        return allSets
            .filter { it.weightLb != null }
            .groupBy { it.exerciseId }
            .mapValues { (_, sets) ->
                sets.groupBy { it.sessionStartedAt }
                    .map { (sessionDate, sessionSets) ->
                        VolumePoint(
                            sessionDate = sessionDate,
                            totalVolumeLb = sessionSets.sumOf { (it.weightLb ?: 0.0) * it.reps }
                        )
                    }
                    .sortedBy { it.sessionDate }
            }
    }

    private val RADAR_EXERCISE_IDS = listOf("ua1", "ua2", "la1", "ub1", "ub2", "lb1")

    private fun buildCompoundMaxes(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): Map<String, Double> {
        return allSets
            .filter { it.weightLb != null && it.exerciseId in RADAR_EXERCISE_IDS }
            .groupBy { it.exerciseId }
            .mapValues { (_, sets) -> sets.maxOf { it.weightLb!! } }
    }

    private fun buildDayTypeBreakdown(rows: List<com.forge.app.data.db.dao.SessionDao.DayTypeStats>): List<DayTypeBreakdown> {
        return rows.mapNotNull { row ->
            val dayName = Program.days.firstOrNull { it.key == row.dayKey }?.defaultName ?: return@mapNotNull null
            DayTypeBreakdown(
                dayKey = row.dayKey,
                dayName = dayName,
                avgDurationMin = (row.avgDurationMin ?: 0.0).toInt(),
                prRate = row.prRate ?: 0.0,
                skipRate = 0.0, // would need logged exercise skip data — approximate as 0 for now
                sessionCount = row.sessionCount
            )
        }.sortedBy { it.dayKey }
    }

    private fun buildInsights(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>,
        weekSets: List<com.forge.app.data.db.projections.SetWithExerciseId>,
        dayTypeRows: List<com.forge.app.data.db.dao.SessionDao.DayTypeStats>
    ): List<InsightFlag> {
        val insights = mutableListOf<InsightFlag>()
        // Best time-of-day (#41)
        val zone = ZoneId.systemDefault()
        val prsByHour = allSets.filter { it.weightLb != null }
            .groupBy { Instant.ofEpochMilli(it.sessionStartedAt).atZone(zone).hour }
        val bestHour = prsByHour.maxByOrNull { it.value.size }?.key
        if (bestHour != null) {
            val label = when {
                bestHour < 10 -> "morning"
                bestHour < 13 -> "late morning"
                bestHour < 17 -> "afternoon"
                else -> "evening"
            }
            insights.add(InsightFlag("⏰", "Best time to train", "You log the most sets in the $label (${bestHour}:00)."))
        }
        // Most improved exercise (#41): biggest % gain in max weight over last 3 months
        val threeMonthsAgo = clock.nowMs() - 90L * 24 * 3600 * 1000
        val recentByExercise = allSets.filter { it.weightLb != null && it.sessionStartedAt >= threeMonthsAgo }
            .groupBy { it.exerciseId }
        val mostImproved = recentByExercise.entries.mapNotNull { (exId, sets) ->
            val sorted = sets.sortedBy { it.sessionStartedAt }
            val first = sorted.take(sorted.size / 2).maxOfOrNull { it.weightLb!! } ?: return@mapNotNull null
            val last = sorted.drop(sorted.size / 2).maxOfOrNull { it.weightLb!! } ?: return@mapNotNull null
            if (first <= 0) return@mapNotNull null
            val pct = ((last - first) / first * 100).toInt()
            val name = Program.exercise(exId)?.name ?: return@mapNotNull null
            Triple(name, pct, last)
        }.maxByOrNull { it.second }
        if (mostImproved != null && mostImproved.second > 5) {
            insights.add(InsightFlag("📈", "Most improved", "${mostImproved.first} is up ~${mostImproved.second}% in 3 months."))
        }
        // Muscle balance: flag if one muscle group dominates weekly volume
        val weekVolumeByMuscle = weekSets.groupBy { Program.exercise(it.exerciseId)?.muscle?.displayName ?: "Other" }
            .mapValues { (_, sets) -> sets.sumOf { (it.weightLb ?: 0.0) * it.reps } }
        val totalWeekVol = weekVolumeByMuscle.values.sum()
        if (totalWeekVol > 0) {
            val dominant = weekVolumeByMuscle.maxByOrNull { it.value }
            if (dominant != null && dominant.value / totalWeekVol > 0.5) {
                insights.add(InsightFlag("⚖️", "Muscle balance", "${dominant.key} is over 50% of your weekly volume. Consider balancing."))
            }
        }
        // Volume drop deload suggestion (#80)
        val recentDeload = allSets.map { it.sessionStartedAt }.sorted().takeLast(6)
        if (recentDeload.size == 6) {
            val halfStart = recentDeload[3]
            val firstHalfVol = allSets.filter { it.sessionStartedAt < halfStart && it.weightLb != null }
                .sumOf { (it.weightLb ?: 0.0) * it.reps }
            val secondHalfVol = allSets.filter { it.sessionStartedAt >= halfStart && it.weightLb != null }
                .sumOf { (it.weightLb ?: 0.0) * it.reps }
            if (firstHalfVol > 0 && secondHalfVol < firstHalfVol * 0.8) {
                insights.add(InsightFlag("💤", "Consider a deload", "Volume has dropped 20%+ recently. You might benefit from a recovery week."))
            }
        }
        return insights
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
        val lastMonthStart = now.minusMonths(1).atDay(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val thisM = sessionDao.aggregateInRange(thisMonthStart, thisMonthStart + 32L * 24 * 3600 * 1000)
        val lastM = sessionDao.aggregateInRange(lastMonthStart, thisMonthStart)
        if (lastM.sessionCount == 0 && thisM.sessionCount == 0) return null
        return PeriodComparison(
            label = "MONTH",
            current = PeriodStats(thisM.sessionCount, thisM.totalVolume ?: 0.0, thisM.totalPrs, thisM.totalSets),
            previous = PeriodStats(lastM.sessionCount, lastM.totalVolume ?: 0.0, lastM.totalPrs, lastM.totalSets)
        )
    }

    private fun buildExerciseYoY(
        allSets: List<com.forge.app.data.db.projections.SetWithExerciseAndSession>
    ): List<ExerciseYoY> {
        val zone = ZoneId.systemDefault()
        val now = LocalDate.now(zone)
        val thisYearStart = now.withDayOfYear(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val lastYearStart = now.minusYears(1).withDayOfYear(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val lastYearEnd = thisYearStart

        return allSets
            .filter { it.weightLb != null }
            .groupBy { it.exerciseId }
            .mapNotNull { (exerciseId, sets) ->
                val plan = Program.exercise(exerciseId) ?: return@mapNotNull null
                val thisYearMax = sets.filter { it.sessionStartedAt >= thisYearStart }.maxOfOrNull { it.weightLb!! }
                val lastYearMax = sets.filter { it.sessionStartedAt in lastYearStart until lastYearEnd }.maxOfOrNull { it.weightLb!! }
                if (thisYearMax == null || lastYearMax == null) return@mapNotNull null
                ExerciseYoY(
                    exerciseId = exerciseId,
                    exerciseName = plan.name,
                    thisYearMaxLb = thisYearMax,
                    lastYearMaxLb = lastYearMax
                )
            }
            .sortedByDescending { it.delta }
    }

    private fun buildWeekActivity(
        sessions: List<Session>,
        cardioEntries: List<CardioEntry>
    ): List<WeekActivityRow> {
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        val isoWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
        val dayLabels = listOf("MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN")
        return (0..6).map { dow ->
            val date = isoWeekStart.plusDays(dow.toLong())
            val session = sessions.firstOrNull { s ->
                Instant.ofEpochMilli(s.startedAt).atZone(zone).toLocalDate() == date
            }
            val cardio = if (session == null) cardioEntries.firstOrNull { c ->
                Instant.ofEpochMilli(c.date).atZone(zone).toLocalDate() == date
            } else null
            val dayPlan = session?.let { s -> Program.days.firstOrNull { it.key == s.dayKey } }
            WeekActivityRow(
                dayOfWeek = dow,
                dayLabel = dayLabels[dow],
                sessionName = dayPlan?.defaultName,
                muscleWord = dayPlan?.word,
                durationMin = session?.finishedAt?.let { fin -> ((fin - session.startedAt) / 60_000).toInt() },
                setCount = session?.setCount ?: 0,
                hasPr = (session?.prCount ?: 0) > 0,
                cardioType = cardio?.type?.replaceFirstChar { it.uppercase() },
                cardioDurationMin = cardio?.durationMin,
                cardioDistanceKm = cardio?.distanceKm
            )
        }
    }

    companion object {
        /** Weekly session count that counts toward the consistency streak. */
        private const val CONSISTENCY_TARGET = 3
        private const val WEEK_MS: Long = 7L * 24 * 60 * 60 * 1000
        private const val HEATMAP_DAYS = 49
        private const val HEATMAP_WINDOW_MS: Long = HEATMAP_DAYS.toLong() * 24 * 60 * 60 * 1000
        private const val STRENGTH_CURVE_MAX_POINTS = 10
        /** DB Bench Press — Antho's main upper-body lift, shown as the default curve. */
        private const val STRENGTH_CURVE_EXERCISE_ID = "ua1"
    }
}
