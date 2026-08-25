package com.forge.app.ui.coach

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.data.repo.CoachBrief
import com.forge.app.data.repo.CoachRepository
import com.forge.app.data.repo.CoachTimeline
import com.forge.app.data.repo.CoachWatch
import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.bestE1rm
import com.forge.app.domain.coach.GoalPortfolio
import com.forge.app.ui.common.ProgramChangeGuard
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.Instant
import javax.inject.Inject
import kotlin.math.roundToInt

/**
 * One state for the whole redesigned Coach page: the Week Brief (proposals + review), the
 * signals read (what the coach watches), the journey (trust, milestones, week record), plus
 * the raw series its charts draw. The chart series come off the SAME cached snapshot the
 * brief already assembled, so the page costs one whole-history fan-out, not four.
 */
@HiltViewModel
class CoachViewModel @Inject constructor(
    private val coachRepo: CoachRepository,
    private val adaptationRepo: AdaptationRepository,
    private val settingsRepo: SettingsRepository,
    private val goalRepo: com.forge.app.data.repo.CoachGoalRepository,
    private val academyRepo: com.forge.app.data.repo.AcademyRepository,
    private val blockRepo: com.forge.app.data.repo.BlockRepository,
    private val projectRepo: com.forge.app.data.repo.ProjectRepository,
    private val programChangeGuard: ProgramChangeGuard
) : ViewModel() {

    /** Health Connect series for the signal deep dives; empty lists mean not connected. */
    data class HealthSeries(
        /** Hours per night, oldest first (last [SLEEP_NIGHTS_SHOWN] synced nights). */
        val sleepHours: List<Float> = emptyList(),
        /** The nightly average at or below which sleep reads as a recovery drag. */
        val sleepFloorHours: Float = 6.5f,
        /** Resting heart rate readings in bpm, oldest first. */
        val restingHr: List<Int> = emptyList(),
        val hrWindowAvg: Int? = null,
        val hrBaseline: Int? = null,
        /** Overnight HRV (RMSSD ms): recent-window average vs the prior-window baseline (W6). */
        val hrvWindowAvg: Int? = null,
        val hrvBaseline: Int? = null
    )

    data class UiState(
        val loading: Boolean = true,
        /** Freestyle has no plan to coach against; the page explains itself and loads nothing. */
        val freestyle: Boolean = false,
        val brief: CoachBrief? = null,
        val watch: CoachWatch? = null,
        val timeline: CoachTimeline? = null,
        /** Best estimated 1RM per non-skipped bout, per program slot, oldest first. */
        val e1rmBySlot: Map<String, List<Double>> = emptyMap(),
        val health: HealthSeries = HealthSeries(),
        /** Whole days until the next weekly brief (next Monday); 0 when unknown. */
        val daysToNextBrief: Int = 0,
        /** A2: the Goal Portfolio, priority order, each with its live reading and ETA. */
        val goals: List<GoalPortfolio.GoalState> = emptyList(),
        /** Goals that fight, with the coach's sequencing proposal. */
        val goalConflicts: List<GoalPortfolio.GoalConflict> = emptyList(),
        /** Unlocked-but-unread Academy lessons. */
        val newLessons: Int = 0,
        /** The live training block (C), or null when the coach is running reactively. */
        val block: com.forge.app.data.db.entities.TrainingBlock? = null,
        /** D: the running project, and the next one the coach would propose. */
        val project: com.forge.app.data.db.entities.CoachProject? = null,
        val projectProposal: com.forge.app.domain.coach.ProjectScanner.Candidate? = null,
        /** Every project the coach would consider, best first — the user picks, not the ranking. */
        val projectOptions: List<com.forge.app.domain.coach.ProjectScanner.Candidate> = emptyList(),
        /** D: what the coach has measured about this athlete specifically. */
        val profile: com.forge.app.domain.coach.PersonalProfile.Profile =
            com.forge.app.domain.coach.PersonalProfile.Profile.DEFAULTS
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        viewModelScope.launch { load() }
    }

    private suspend fun load() {
        // Defense in depth: the coach entry points are hidden for freestyle, but if the page is
        // reached anyway, don't run the weekly pass against an empty program (it would write a
        // coach_pass row and could auto-apply).
        if (settingsRepo.freestyleMode.first()) {
            _state.value = UiState(loading = false, freestyle = true)
            return
        }
        val brief = runCatching { coachRepo.brief() }.getOrNull()
        val watch = runCatching { coachRepo.coachLab() }.getOrNull()
        val timeline = runCatching { coachRepo.timeline() }.getOrNull()
        // brief()/coachLab() just snapshotted; this reuses their cache instead of a fresh fan-out.
        val snap = runCatching { adaptationRepo.snapshotCached() }.getOrNull()
        // A2: the coach's own moments — a stall held because the athlete is cutting unlocks its
        // lesson the first time it happens. Idempotent, so calling it on every open is fine.
        runCatching { academyRepo.syncCoachMoments() }
        _state.value = UiState(
            loading = false,
            brief = brief,
            watch = watch,
            timeline = timeline,
            e1rmBySlot = snap?.let(::e1rmSeries).orEmpty(),
            health = snap?.let(::healthSeries) ?: HealthSeries(),
            daysToNextBrief = snap?.let { 7 - todayIndex(it) } ?: 0,
            goals = runCatching { goalRepo.states() }.getOrDefault(emptyList()),
            goalConflicts = runCatching { goalRepo.conflicts() }.getOrDefault(emptyList())
                .also { if (it.isNotEmpty()) runCatching { academyRepo.onGoalConflict() } },
            newLessons = runCatching { academyRepo.newCount() }.getOrDefault(0),
            block = runCatching { blockRepo.active() }.getOrNull(),
            project = runCatching { projectRepo.active() }.getOrNull(),
            projectProposal = runCatching { projectRepo.proposal() }.getOrNull(),
            projectOptions = runCatching { projectRepo.proposals() }.getOrDefault(emptyList()),
            profile = snap?.let { com.forge.app.domain.coach.PersonalProfile.build(it) }
                ?: com.forge.app.domain.coach.PersonalProfile.Profile.DEFAULTS
        )
        // Opening the page clears the Overview "new report" banner for this week.
        brief?.let { runCatching { coachRepo.markSeen(it.pass.weekId) } }
    }

    // ─── Decision lifecycle ────────────────────────────────────────────────────

    fun apply(decisionId: Long) {
        // A deload regenerates the program and discards any in-progress workout; guard it.
        val isDeload = _state.value.brief?.decisions
            ?.firstOrNull { it.id == decisionId }?.type == "deload"
        act(guarded = isDeload) { coachRepo.applyDecision(decisionId) }
    }

    fun skip(decisionId: Long) = act(guarded = false) { coachRepo.skipDecision(decisionId) }

    fun undo(decisionId: Long) = act(guarded = false) { coachRepo.undoDecision(decisionId) }

    fun applyAll(weekId: String) {
        val hasDeload = _state.value.brief?.decisions
            ?.any { it.status == CoachRepository.STATUS_PROPOSED && it.type == "deload" } == true
        act(guarded = hasDeload) { coachRepo.applyAll(weekId) }
    }

    private fun act(guarded: Boolean, block: suspend () -> Unit) = viewModelScope.launch {
        if (guarded) programChangeGuard.run { runAndRefresh(block) } else runAndRefresh(block)
    }

    private suspend fun runAndRefresh(block: suspend () -> Unit) {
        runCatching { block() }
        // A lifecycle tap can move trust, learned biases and the brief itself; refresh all three
        // reads (each falls back to the last good value on failure). The chart series don't change.
        val s = _state.value
        _state.value = s.copy(
            brief = runCatching { coachRepo.refreshBrief() }.getOrNull() ?: s.brief,
            watch = runCatching { coachRepo.coachLab() }.getOrNull() ?: s.watch,
            timeline = runCatching { coachRepo.timeline() }.getOrNull() ?: s.timeline
        )
    }

    // ─── Goal portfolio (A2) ───────────────────────────────────────────────────

    fun addGoal(kind: com.forge.app.domain.coach.CoachGoalKind, targetKey: String, targetValue: Double?) =
        viewModelScope.launch {
            runCatching { goalRepo.add(kind, targetKey, targetValue) }
            refreshGoals()
        }

    private suspend fun refreshGoals() {
        val s = _state.value
        _state.value = s.copy(
            goals = runCatching { goalRepo.states() }.getOrDefault(s.goals),
            goalConflicts = runCatching { goalRepo.conflicts() }.getOrDefault(s.goalConflicts)
        )
    }

    // ─── Training block (C) ────────────────────────────────────────────────────

    /** Start a block around the athlete's top goal. Announced, never silent. */
    fun startBlock() = viewModelScope.launch {
        val weekId = _state.value.brief?.pass?.weekId ?: return@launch
        runCatching { blockRepo.start(weekId = weekId) }
        val block = runCatching { blockRepo.active() }.getOrNull()
        _state.update { it.copy(block = block) }
    }

    /** End it early — the user's veto is always one tap away. */
    fun endBlock() = viewModelScope.launch {
        runCatching { blockRepo.end() }
        _state.update { it.copy(block = null) }
    }

    // ─── Projects (D) ──────────────────────────────────────────────────────────

    /** Accept the proposed project. Propose-only at this phase: nothing starts without this tap. */
    fun acceptProject() = viewModelScope.launch {
        val candidate = _state.value.projectProposal ?: return@launch
        runCatching { projectRepo.accept(candidate) }
        refreshProjects()
    }

    /** Start a specific project the user chose from [UiState.projectOptions]. */
    fun startProject(candidate: com.forge.app.domain.coach.ProjectScanner.Candidate) =
        viewModelScope.launch {
            runCatching { projectRepo.accept(candidate) }
            refreshProjects()
        }

    fun completeProject() = viewModelScope.launch {
        _state.value.project?.let { runCatching { projectRepo.complete(it.id) } }
        refreshProjects()
    }

    fun abandonProject() = viewModelScope.launch {
        _state.value.project?.let { runCatching { projectRepo.abandon(it.id) } }
        refreshProjects()
    }

    private suspend fun refreshProjects() {
        // Read everything first, then update atomically: `_state.value = _state.value.copy(...)`
        // snapshotted the state BEFORE these suspending reads and wrote the whole object back after,
        // reverting whatever another coroutine had written in between.
        val project = runCatching { projectRepo.active() }.getOrNull()
        val proposal = runCatching { projectRepo.proposal() }.getOrNull()
        val options = runCatching { projectRepo.proposals() }.getOrDefault(emptyList())
        val lessons = runCatching { academyRepo.newCount() }.getOrNull()
        _state.update {
            it.copy(
                project = project,
                projectProposal = proposal,
                projectOptions = options,
                newLessons = lessons ?: it.newLessons
            )
        }
    }

    // ─── Chart series (pure reads off the snapshot) ────────────────────────────

    private fun todayIndex(s: AdaptationSnapshot): Int =
        Instant.ofEpochMilli(s.nowMs).atZone(s.zoneId).toLocalDate().dayOfWeek.value - 1

    private fun e1rmSeries(s: AdaptationSnapshot): Map<String, List<Double>> =
        s.exerciseHistory.mapValues { (_, bouts) ->
            bouts.filter { !it.skipped }.mapNotNull { it.bestE1rm() }.takeLast(E1RM_BOUTS_SHOWN)
        }.filterValues { it.size >= 2 }

    private fun healthSeries(s: AdaptationSnapshot): HealthSeries {
        val t = AdaptThresholds()
        val dayMs = 24L * 60 * 60 * 1000
        val windowStart = s.nowMs - t.deloadWindowDays * dayMs
        val priorStart = windowStart - t.deloadPriorBaselineDays * dayMs
        val sleep = s.health.sleepNights.sortedBy { it.endedAtMs }
            .takeLast(SLEEP_NIGHTS_SHOWN).map { it.durationMin / 60f }
        val hr = s.health.restingHr.sortedBy { it.timeMs }
        val windowHr = hr.filter { it.timeMs >= windowStart }.map { it.bpm }
        val priorHr = hr.filter { it.timeMs in priorStart until windowStart }.map { it.bpm }
        // Overnight HRV (W6) — same window-vs-baseline framing as resting HR, same sample gate,
        // so the two heart readings can't drift apart in meaning.
        val hrv = s.health.hrv.sortedBy { it.timeMs }
        val windowHrv = hrv.filter { it.timeMs >= windowStart }.map { it.rmssdMs }
        val priorHrv = hrv.filter { it.timeMs in priorStart until windowStart }.map { it.rmssdMs }
        return HealthSeries(
            sleepHours = sleep,
            sleepFloorHours = t.deloadSleepDebtMinutes / 60f,
            restingHr = hr.takeLast(HR_READINGS_SHOWN).map { it.bpm },
            hrWindowAvg = windowHr.takeIf { it.size >= t.deloadMinRestingHrSamples }
                ?.average()?.roundToInt(),
            hrBaseline = priorHr.takeIf { it.size >= t.deloadMinRestingHrSamples }
                ?.average()?.roundToInt(),
            hrvWindowAvg = windowHrv.takeIf { it.size >= t.deloadMinRestingHrSamples }
                ?.average()?.roundToInt(),
            hrvBaseline = priorHrv.takeIf { it.size >= t.deloadMinRestingHrSamples }
                ?.average()?.roundToInt()
        )
    }

    private companion object {
        const val E1RM_BOUTS_SHOWN = 12
        const val SLEEP_NIGHTS_SHOWN = 14
        const val HR_READINGS_SHOWN = 30
    }
}
