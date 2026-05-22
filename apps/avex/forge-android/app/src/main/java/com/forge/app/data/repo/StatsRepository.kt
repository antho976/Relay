package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.program.Program
import com.forge.app.ui.gym.stats.state.HeatmapCell
import com.forge.app.ui.gym.stats.state.MuscleVolume
import com.forge.app.ui.gym.stats.state.PrEntry
import com.forge.app.ui.gym.stats.state.StrengthCurve
import com.forge.app.ui.gym.stats.state.Totals
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
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
    private val clock: Clock
) {

    data class WeeklyStats(
        val workouts: Int = 0,
        val volumeLb: Double = 0.0,
        val cardioMinutes: Int = 0,
        val totalFinishedSessions: Int = 0
    )

    fun observeWeeklyStats(): Flow<WeeklyStats> {
        val weekStartMs = clock.nowMs() - WEEK_MS
        return combine(
            sessionDao.observeFinishedCountSince(weekStartMs),
            sessionDao.observeVolumeSince(weekStartMs),
            cardioDao.observeMinutesSince(weekStartMs, excludeType = "rest"),
            sessionDao.observeFinishedCount()
        ) { workouts, volume, cardio, totalFinished ->
            WeeklyStats(
                workouts = workouts,
                volumeLb = volume ?: 0.0,
                cardioMinutes = cardio ?: 0,
                totalFinishedSessions = totalFinished
            )
        }
    }

    // ─── Gym stats subtab ──────────────────────────────────────────────────────

    data class GymStats(
        val totals: Totals,
        val heatmap: List<HeatmapCell>,
        val volumeByMuscle: List<MuscleVolume>,
        val strengthCurve: StrengthCurve?,
        val recentPrs: List<PrEntry>
    )

    /**
     * Single observable feeding the Gym → Stats subtab. Aggregates several DAO flows
     * into the snapshot the UI consumes. Heatmap window and curve exercise are fixed
     * here; ideally configurable later via settings.
     */
    fun observeGymStats(): Flow<GymStats> {
        val heatmapStartMs = clock.nowMs() - HEATMAP_WINDOW_MS
        val volumeStartMs = clock.nowMs() - WEEK_MS

        val totalsFlow: Flow<Totals> = combine(
            sessionDao.observeFinishedCount(),
            loggedExerciseDao.observeTotalLogged(),
            loggedExerciseDao.observePrCount()
        ) { workouts, exercises, prs ->
            Totals(workouts = workouts, exercisesLogged = exercises, prs = prs)
        }

        return combine(
            totalsFlow,
            loggedExerciseDao.observeHeatmapTimestamps(heatmapStartMs),
            loggedSetDao.observeSetsSinceWithExerciseId(volumeStartMs),
            loggedSetDao.observeAllFinishedSetsWithSession(),
            loggedExerciseDao.observeRecentPrs()
        ) { totals, heatmapRows, volumeSets, allSets, prRows ->
            GymStats(
                totals = totals,
                heatmap = buildHeatmap(heatmapRows.map { it.startedAt }),
                volumeByMuscle = buildVolumeByMuscle(volumeSets),
                strengthCurve = buildStrengthCurveFor(STRENGTH_CURVE_EXERCISE_ID, allSets),
                recentPrs = buildPrEntries(prRows, allSets)
            )
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

    companion object {
        private const val WEEK_MS: Long = 7L * 24 * 60 * 60 * 1000
        private const val HEATMAP_DAYS = 49
        private const val HEATMAP_WINDOW_MS: Long = HEATMAP_DAYS.toLong() * 24 * 60 * 60 * 1000
        private const val STRENGTH_CURVE_MAX_POINTS = 10
        /** DB Bench Press — Antho's main upper-body lift, shown as the default curve. */
        private const val STRENGTH_CURVE_EXERCISE_ID = "ua1"
    }
}
