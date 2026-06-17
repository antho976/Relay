package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.AdviceEventDao
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.AdviceEvent
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.InsightEngine
import com.forge.app.domain.adapt.PrefsSnap
import com.forge.app.domain.adapt.ProgressionAdvisor
import com.forge.app.domain.adapt.RatioCounts
import com.forge.app.domain.adapt.ReadinessAdvisor
import com.forge.app.domain.adapt.Recommendation
import com.forge.app.domain.adapt.RecommendationArbiter
import com.forge.app.domain.adapt.SnapshotAssembler
import com.forge.app.program.Equipment
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.GenerationParams
import com.forge.app.program.MuscleGroup
import com.forge.app.program.ProblemArea
import com.forge.app.program.Program
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The adaptation engine's only impure piece: fetches history + prefs and delegates the
 * grouping to the pure [SnapshotAssembler] (mirrors how [TrophyRepository.snapshot] feeds
 * [com.forge.app.domain.trophy.TrophyEvaluator]).
 *
 * Call [snapshot] from surface-level entry points (Overview open, session finish) — never
 * from the per-set logging hot path; the fan-out reads the whole history.
 */
@Singleton
class AdaptationRepository @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedExerciseDao: LoggedExerciseDao,
    private val loggedSetDao: LoggedSetDao,
    private val moodDao: MoodDao,
    private val cardioDao: CardioDao,
    private val adviceEventDao: AdviceEventDao,
    private val vacationDao: com.forge.app.data.db.dao.VacationDao,
    private val programRepository: ProgramRepository,
    private val programCustomizationRepo: ProgramCustomizationRepository,
    private val settingsRepository: SettingsRepository,
    private val healthConnectManager: com.forge.app.data.health.HealthConnectManager,
    private val clock: Clock
) {

    /** Moods/cardio older than this can't influence any current signal — skip loading them. */
    private val signalWindowMs = 90L * 24 * 60 * 60 * 1000

    /**
     * How far back to read Health Connect: at least [signalWindowMs], but always enough to cover
     * DeloadAdvisor's fatigue window PLUS its prior-month baseline (+1 week margin), so widening
     * either threshold can't leave the resting-HR driver's prior window outside the fetched data.
     */
    private val healthLookbackMs: Long = run {
        val t = AdaptThresholds()
        val needDays = (t.deloadWindowDays + t.deloadPriorBaselineDays + 7).toLong()
        maxOf(signalWindowMs, needDays * 24 * 60 * 60 * 1000)
    }

    /** Dismissing or applying advice mutes that id for this long — no re-nagging. */
    private val adviceCooldownMs = 14L * 24 * 60 * 60 * 1000

    /** How long a snapshot stays reusable by [snapshotCached] — long enough to bridge a screen hop. */
    private val SNAPSHOT_CACHE_MS = 60_000L

    /**
     * The full adaptation snapshot. THROWS on a read/assembly failure so error-aware callers can tell a
     * real failure apart from a genuinely empty pre-baseline history — the Week Brief hides "Last week"
     * and drops its "N sessions logged" countdown, the weekly pass records STATUS_ERROR, and Stats'
     * engine read degrades to no pulse/plateaus. Surfaces that fan out from this but have NO error path
     * of their own use [snapshotOrEmpty] instead.
     */
    suspend fun snapshot(): AdaptationSnapshot = assembleSnapshot()

    /**
     * Crash-safe snapshot for surfaces with no error handling of their own (the Overview coach feed runs
     * in a bare launch): a single corrupt row degrades to an empty (pre-baseline) snapshot instead of
     * taking the screen down. Re-throws [CancellationException] so coroutine cancellation still propagates
     * (a bare runCatching/catch would otherwise swallow it and keep the cancelled coroutine running).
     */
    suspend fun snapshotOrEmpty(): AdaptationSnapshot =
        try {
            assembleSnapshot()
        } catch (c: kotlinx.coroutines.CancellationException) {
            throw c
        } catch (t: Throwable) {
            emptySnapshot()
        }

    /** Empty pre-baseline snapshot used when [assembleSnapshot] fails — pure, can't itself throw. */
    private fun emptySnapshot(): AdaptationSnapshot = SnapshotAssembler.assemble(
        nowMs = clock.nowMs(),
        program = Program.days,
        swapCandidateIds = { emptyList() },
        sessions = emptyList(),
        loggedExercises = emptyList(),
        loggedSets = emptyList(),
        prefs = PrefsSnap(),
        zoneId = java.time.ZoneId.systemDefault()
    )

    private suspend fun assembleSnapshot(): AdaptationSnapshot {
        // The DAO queries already exclude unfinished + untracked; the session list must too.
        val sessions = sessionDao.allFinished().filter { !it.isUntracked }
        val loggedExercises = loggedExerciseDao.allForFinishedSessions()
        val loggedSets = loggedSetDao.allForFinishedSessions()

        val equipment = settingsRepository.availableEquipment.first()
            .mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet()
        val disliked = settingsRepository.dislikedExercises.first()
        val frozenIds = settingsRepository.frozenExerciseIds.first()
        val prefs = PrefsSnap(
            plateLb = settingsRepository.plateWeightLb.first(),
            likedIds = settingsRepository.likedExercises.first(),
            dislikedIds = disliked,
            pinnedIds = settingsRepository.pinnedExercises.first(),
            maxDbLb = settingsRepository.maxDbWeightLb.first(),
            // 0 = no active deload; lets DeloadAdvisor suppress right after an apply, before the
            // first deload-week session is even logged (seam fix, finding 18).
            lastDeloadAppliedMs = settingsRepository.deloadWeekStartMs.first().takeIf { it > 0 }
        )

        // The engine plans against EFFECTIVE sets (baseline + the user's/coach's per-slot overrides),
        // not the bare baseline — otherwise the coach re-proposes volume onto a slot it already boosted
        // and bakes a phantom set at the next regenerate (seam findings 2/15).
        val effectiveDays = Program.days.map { day ->
            day.copy(exercises = programCustomizationRepo.effectivePlanForDay(day.key))
        }

        val now = clock.nowMs()
        val snap = SnapshotAssembler.assemble(
            nowMs = now,
            program = effectiveDays,
            swapCandidateIds = { plan ->
                ExerciseLibrary.swapCandidates(plan.muscle, equipment, disliked, frozenIds)
                    .map { it.id }
                    .filter { it != plan.id }
            },
            sessions = sessions,
            loggedExercises = loggedExercises,
            loggedSets = loggedSets,
            prefs = prefs,
            moods = moodDao.since(now - signalWindowMs),
            cardio = cardioDao.since(now - signalWindowMs),
            // Off-app recovery signals (sleep, resting HR). Returns empty unless the user has
            // connected Health Connect AND granted access — so this is a no-op for everyone else.
            // Fetch back far enough to cover DeloadAdvisor's prior-month baseline (its resting-HR
            // driver compares the fatigue window against deloadWindowDays + deloadPriorBaselineDays
            // ago), so widening either threshold can't silently starve that window.
            health = healthConnectManager.readRecovery(now - healthLookbackMs, now),
            zoneId = java.time.ZoneId.systemDefault()
        )
        lastSnapshot = snap
        lastSnapshotAtMs = now
        return snap
    }

    @Volatile private var lastSnapshot: AdaptationSnapshot? = null
    @Volatile private var lastSnapshotAtMs: Long = 0L

    /**
     * Like [snapshot] but reuses one assembled in the last [SNAPSHOT_CACHE_MS] — for READ-ONLY display
     * (the Coach Lab) opened right after a path that already snapshotted (the Week Brief / Overview), so
     * the whole-history fan-out doesn't run twice in one user action. Mutating callers must use [snapshot].
     */
    suspend fun snapshotCached(): AdaptationSnapshot {
        lastSnapshot?.let { if (clock.nowMs() - lastSnapshotAtMs <= SNAPSHOT_CACHE_MS) return it }
        return snapshot()
    }

    /**
     * Today's readiness scale (System 6), or null below the data gates / at net zero.
     * Deliberately lighter than [snapshot] — three small queries, no whole-history set
     * fan-out — so the day screen can load it at session open.
     */
    suspend fun readinessScale(): Recommendation.ReadinessScale? {
        val now = clock.nowMs()
        return ReadinessAdvisor.evaluate(
            sessions = sessionDao.allFinished().filter { !it.isUntracked },
            moods = moodDao.since(now - signalWindowMs),
            cardio = cardioDao.since(now - signalWindowMs),
            nowMs = now,
            zoneId = java.time.ZoneId.systemDefault(),
            onVacation = com.forge.app.domain.vacation.VacationCalendar.onVacation(vacationDao.all())
        )
    }

    /**
     * The Overview coach feed off ONE snapshot: actionable arbitrated recommendations (deload +
     * plateau ladder) PLUS a sub-threshold fatigue read so Overview can show "recovery signals
     * building" before a deload actually fires (Tier 3). Insights stay on Stats.
     */
    data class CoachFeed(
        val recommendations: List<Recommendation>,
        /** Fatigue that's building but hasn't crossed the deload line yet, or null. */
        val fatigueBuilding: DeloadAdvisor.FatigueAssessment?,
        val fatigueThreshold: Int
    )

    suspend fun coachFeed(): CoachFeed {
        // Overview calls this from a bare viewModelScope.launch with no error path, so degrade to an
        // empty snapshot (no recs) rather than crashing the home screen on a corrupt row.
        val s = snapshotOrEmpty()
        val t = AdaptThresholds()
        val recs = ProgressionAdvisor.evaluate(s, t) + listOfNotNull(DeloadAdvisor.evaluate(s, t))
        val arbitrated = RecommendationArbiter.arbitrate(recs, mutedAdviceIds())
        // The same "building" band AutoCoachPlanner uses for its consolidation hold — read from the
        // shared threshold so the Overview nudge and the planner can't describe different ranges.
        val building = DeloadAdvisor.fatigue(s, t)
            ?.takeIf { it.score in (t.deloadScoreThreshold - t.consolidateBandPoints) until t.deloadScoreThreshold }
        return CoachFeed(arbitrated, building, t.deloadScoreThreshold)
    }

    /** Just the actionable recommendations (kept for callers that don't need the fatigue read). */
    suspend fun coachRecommendations(): List<Recommendation> = coachFeed().recommendations

    /**
     * Everything the Stats page reads from the engine, off ONE snapshot fan-out: the fatigue
     * pulse (System 5), the plateau ladder (System 1), the always-on balance ratios (System 4's
     * counting, ungated), and the insight observations. Call once per Stats open — surface-level
     * entry point, never the per-set hot path. (Insights live here, not in StatsRepository's
     * reactive combine, so Stats open does ONE snapshot instead of two.)
     */
    data class EngineStatsRead(
        val fatigue: DeloadAdvisor.FatigueAssessment?,
        val deloadScoreThreshold: Int,
        val plateaus: List<Recommendation>,
        val ratios: List<RatioCounts>,
        val insights: List<Recommendation.Insight>
    )

    suspend fun engineStatsRead(): EngineStatsRead {
        val s = snapshot()
        val t = AdaptThresholds()
        return EngineStatsRead(
            fatigue = DeloadAdvisor.fatigue(s, t),
            deloadScoreThreshold = t.deloadScoreThreshold,
            plateaus = ProgressionAdvisor.evaluate(s, t),
            ratios = InsightEngine.balanceRatios(s, t),
            insights = InsightEngine.evaluate(s)
        )
    }

    // ─── Advice feedback (cooldowns + future calibration) ─────────────────────

    suspend fun logAdviceApplied(adviceId: String) = logAdvice(adviceId, "applied")
    suspend fun logAdviceDismissed(adviceId: String) = logAdvice(adviceId, "dismissed")

    private suspend fun logAdvice(adviceId: String, action: String) {
        adviceEventDao.insert(AdviceEvent(adviceId = adviceId, action = action, loggedAt = clock.nowMs()))
    }

    /** Applied OR dismissed inside the cooldown window → the engine shuts up about it. */
    suspend fun mutedAdviceIds(): Set<String> {
        val cutoff = clock.nowMs() - adviceCooldownMs
        return adviceEventDao.recent()
            .filter { it.loggedAt >= cutoff && (it.action == "dismissed" || it.action == "applied") }
            .mapTo(mutableSetOf()) { it.adviceId }
    }

    // ─── Apply paths (always user-initiated; the engine never writes silently) ─

    /**
     * One-tap apply for [Recommendation.DeloadSuggestion]: regenerate the current split at
     * deload volume — the same operation as Settings → "Generate deload week". The params
     * mirror SettingsViewModel.buildParams; keep the two in sync.
     */
    suspend fun applyDeloadWeek() {
        val params = GenerationParams(
            daysPerWeek = settingsRepository.daysPerWeek.first(),
            emphasis = settingsRepository.programEmphasis.first(),
            goal = settingsRepository.userGoal.first().ifBlank { "build_muscle" },
            experience = settingsRepository.programExperience.first(),
            problemAreas = settingsRepository.problemAreas.first()
                .mapNotNull { ProblemArea.fromCode(it) }.toSet(),
            priorityMuscles = settingsRepository.priorityMuscles.first()
                .mapNotNull { runCatching { MuscleGroup.fromCode(it) }.getOrNull() }.toSet(),
            pinned = settingsRepository.pinnedExercises.first(),
            // Mirror buildParams: without this a deload generated from the coach card could
            // prescribe dumbbell movements above the user's heaviest available dumbbell.
            dbMaxLb = settingsRepository.maxDbWeightLb.first(),
            deload = true,
            frozenIds = settingsRepository.frozenExerciseIds.first()
        )
        // Persist the deload BEFORE regenerating: generate() leaves a deload marker untouched but
        // clears it for any non-deload regenerate, so a later manual "Generate" exits the deload (#18).
        settingsRepository.setDeloadWeekStartMs(clock.nowMs())
        programRepository.generate(
            params,
            settingsRepository.availableEquipment.first()
                .mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet(),
            settingsRepository.likedExercises.first(),
            settingsRepository.dislikedExercises.first()
        )
        logAdviceApplied("deload.suggest")
    }
}
