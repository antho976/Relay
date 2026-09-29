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
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.ensureActive
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
    private val inputSignals: com.forge.app.data.repo.EngineInputSignals,
    private val programChangeGuard: ProgramChangeGuard,
    private val snackbar: com.forge.app.ui.common.SnackbarController
) : ViewModel() {

    /** Health Connect series for the signal deep dives; empty lists mean not connected. */
    data class HealthSeries(
        /**
         * Hours per night, oldest first: the last [SLEEP_NIGHTS_SHOWN] nights inside the recent
         * window, the same window the Sleep input's count reads, so the chart under it never shows
         * weeks-old nights.
         */
        val sleepHours: List<Float> = emptyList(),
        /** The nightly average at or below which sleep reads as a recovery drag. */
        val sleepFloorHours: Float = 6.5f,
        /** Resting heart rate readings in bpm inside the recent window, oldest first. */
        val restingHr: List<Int> = emptyList(),
        /**
         * Any night / resting-HR reading at all in the whole read, window or not: Health Connect
         * is connected even when nothing recent has synced, so its input offers no Connect.
         */
        val sleepSynced: Boolean = false,
        val restingHrSynced: Boolean = false,
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
        /**
         * Advanced tracking (Settings → Coach). Off, the page is the account and what is next;
         * on, the readings behind the calls are drawn too: signals, block, inputs, learned.
         */
        val advanced: Boolean = false,
        /**
         * Epoch millis before which the page does not offer advanced tracking in its pop-up.
         * Defaults to never, so nothing is offered until the stored preference has been read.
         */
        val advancedPromptAfter: Long = Long.MAX_VALUE,
        val brief: CoachBrief? = null,
        val watch: CoachWatch? = null,
        val timeline: CoachTimeline? = null,
        /**
         * Decisions whose Undo the repository just declined (window over, or a newer call owns the
         * slot). A declined undo writes nothing, so the brief and timeline re-read equal and the
         * page's clock does not move: without this the dead pill stayed up. Held until the account
         * itself changes, when the page re-reads its clock and every pill is re-judged.
         */
        val undoRefused: Set<Long> = emptySet(),
        /** Best estimated 1RM per non-skipped bout, per program slot, oldest first. */
        val e1rmBySlot: Map<String, List<Double>> = emptyMap(),
        val health: HealthSeries = HealthSeries(),
        /**
         * Today's readiness, the same read that scales the day's weight targets. Null when the
         * advisor is silent: below [readinessGateSessions], or nothing today moves the number.
         */
        val readiness: com.forge.app.domain.adapt.Recommendation.ReadinessScale? = null,
        /** Finished sessions readiness needs before it reads anything at all. */
        val readinessGateSessions: Int = AdaptThresholds().readinessMinSessions,
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
        /** A block start or end is in flight; the action is inert until it lands (M-13). */
        val blockBusy: Boolean = false,
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

    suspend fun refreshWhileVisible() = kotlinx.coroutines.coroutineScope {
        // The advanced-tracking switch is a view preference, not an engine input: it flips which
        // regions the page draws without re-running the weekly pass, so it rides its own collector.
        launch { settingsRepo.coachAdvanced.collect { v -> _state.update { it.copy(advanced = v) } } }
        launch {
            settingsRepo.coachAdvancedPromptAfter.collect { v ->
                _state.update { it.copy(advancedPromptAfter = v) }
            }
        }
        inputSignals.changes().collect { load() }
    }

    private suspend fun load() = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Default) {
        // Defense in depth: the coach entry points are hidden for freestyle, but if the page is
        // reached anyway, don't run the weekly pass against an empty program (it would write a
        // coach_pass row and could auto-apply).
        if (settingsRepo.freestyleMode.first()) {
            _state.value = UiState(
                loading = false,
                freestyle = true,
                advanced = _state.value.advanced,
                advancedPromptAfter = _state.value.advancedPromptAfter
            )
            return@withContext
        }
        val brief = runCatching { coachRepo.brief() }.getOrNull()
        val watch = runCatching { coachRepo.coachLab() }.getOrNull()
        val timeline = runCatching { coachRepo.timeline() }.getOrNull()
        // A fresh snapshot reflects current training and Health Connect inputs on each visible refresh.
        val snap = runCatching { adaptationRepo.snapshotCached() }.getOrNull()
        // A2: the coach's own moments — a stall held because the athlete is cutting unlocks its
        // lesson the first time it happens. Idempotent, so calling it on every open is fine.
        runCatching { academyRepo.syncCoachMoments() }
        val activeBlock = runCatching { blockRepo.active() }.getOrNull()
        // Off the snapshot above, so the page reads the same sessions and Health Connect nights
        // the day screen scales its targets from, without a second whole-history walk.
        val readiness = runCatching { adaptationRepo.readinessScale(snapshot = snap) }
            .onFailure { if (it is CancellationException) throw it }
            .getOrNull()
        kotlinx.coroutines.currentCoroutineContext().ensureActive()
        _state.update { current -> current.copy(
            loading = false,
            freestyle = false,
            brief = brief,
            watch = watch,
            timeline = timeline,
            undoRefused = keptUndoRefusals(current, brief, timeline),
            e1rmBySlot = snap?.let(::e1rmSeries).orEmpty(),
            health = snap?.let(::healthSeries) ?: HealthSeries(),
            readiness = readiness,
            daysToNextBrief = snap?.let { 7 - todayIndex(it) } ?: 0,
            block = activeBlock,
            profile = snap?.let { com.forge.app.domain.coach.PersonalProfile.build(it) }
                ?: com.forge.app.domain.coach.PersonalProfile.Profile.DEFAULTS
        ) }
        // Opening the page clears the Overview "new report" banner for this week.
        brief?.let { runCatching { coachRepo.markSeen(it.pass.weekId) } }
        // NOTHING else. Opening the page used to also load the goal and project state, which no
        // composable reads (P-06). Moving it after first paint stopped it delaying the frame and
        // left the cost: `goalRepo.states()` and `conflicts()` each assemble a full adaptation
        // snapshot — session, exercise and set history, related repositories, Health Connect
        // recovery — and the project fields cost three more queries plus a scanner pass, on every
        // single open, for output that is never rendered.
        //
        // Every action that can CHANGE any of it already refreshes it (`addGoal` → [refreshGoals];
        // the block and project actions → [refreshProjects]), so the state is loaded by the surface
        // that asks for it. That includes the goal-conflict Academy unlock, which now runs when a
        // goal is added or edited — the moment a conflict can come into being — instead of on every
        // visit to a page that does not show one. Reinstate an eager load here the day a composable
        // genuinely reads these fields.
    }

    /**
     * Flip advanced tracking from the page itself. The preference is the one source of truth and
     * [refreshWhileVisible] collects it, so the page redraws from the write, not from here.
     */
    fun setAdvanced(v: Boolean) = viewModelScope.launch {
        runCatching {
            settingsRepo.setCoachAdvanced(v)
            // Turning it on retires the pop-up for good; otherwise switching it off again would
            // bring the offer straight back (the pop-up shows whenever advanced is off).
            if (v) settingsRepo.setCoachAdvancedPromptAfter(Long.MAX_VALUE)
        }
    }

    /** The advanced-tracking pop-up's "Remind me later": offer it again in a week. */
    fun remindAdvancedLater() = viewModelScope.launch {
        runCatching {
            settingsRepo.setCoachAdvancedPromptAfter(System.currentTimeMillis() + ADVANCED_PROMPT_SNOOZE_MS)
        }
    }

    /** The advanced-tracking pop-up's "Ignore": never offer it again. Settings → Coach still has it. */
    fun ignoreAdvancedPrompt() = viewModelScope.launch {
        runCatching { settingsRepo.setCoachAdvancedPromptAfter(Long.MAX_VALUE) }
    }

    // ─── Decision lifecycle ────────────────────────────────────────────────────

    fun apply(decisionId: Long) {
        // A deload regenerates the program and discards any in-progress workout; guard it.
        val isDeload = _state.value.brief?.decisions
            ?.firstOrNull { it.id == decisionId }?.type == "deload"
        act(guarded = isDeload) { coachRepo.applyDecision(decisionId) }
    }

    fun skip(decisionId: Long) = act(guarded = false) { coachRepo.skipDecision(decisionId) }

    /** An Undo the repository declines (window over, or a newer call owns the slot) says so. */
    fun undo(decisionId: Long) = act(guarded = false) {
        if (!coachRepo.undoDecision(decisionId)) {
            _state.update { it.copy(undoRefused = it.undoRefused + decisionId) }
            snackbar.show("Couldn't undo that change.")
        }
    }

    fun applyAll(weekId: String) {
        val hasDeload = _state.value.brief?.decisions
            ?.any { it.status == CoachRepository.STATUS_PROPOSED && it.type == "deload" } == true
        act(guarded = hasDeload) { coachRepo.applyAll(weekId) }
    }

    private fun act(guarded: Boolean, block: suspend () -> Unit) = viewModelScope.launch {
        if (guarded) programChangeGuard.run { runAndRefresh(block) } else runAndRefresh(block)
    }

    /**
     * [UiState.undoRefused] survives a re-read that leaves the account as it was (a declined undo
     * writes nothing) and clears once it changes: the page's clock re-stamps then and re-judges
     * every pill, and an undo that just succeeded may have freed an older call's slot.
     */
    private fun keptUndoRefusals(s: UiState, brief: CoachBrief?, timeline: CoachTimeline?): Set<Long> =
        if (brief == s.brief && timeline == s.timeline) s.undoRefused else emptySet()

    private suspend fun runAndRefresh(block: suspend () -> Unit) {
        // A cancelled scope must unwind, not be laundered into a refresh: the repository's apply is
        // transactional, so cancellation rolls it back cleanly, and refreshing here would read and
        // publish state from inside a coroutine that is already cancelled (H-03).
        runCatching { block() }.onFailure { if (it is CancellationException) throw it }
        // A lifecycle tap can move trust, learned biases and the brief itself; refresh all three
        // reads (each falls back to the last good value on failure). The chart series don't change.
        // Read first, then update atomically, as [refreshGoals] does: a copy() of a snapshot taken
        // BEFORE these suspending reads wrote the whole state back after them, reverting whatever
        // landed in between (the advanced switch's collector, a block action's busy flag).
        val brief = runCatching { coachRepo.refreshBrief() }.getOrNull()
        val watch = runCatching { coachRepo.coachLab() }.getOrNull()
        val timeline = runCatching { coachRepo.timeline() }.getOrNull()
        _state.update { s ->
            val newBrief = brief ?: s.brief
            val newTimeline = timeline ?: s.timeline
            s.copy(
                brief = newBrief,
                watch = watch ?: s.watch,
                timeline = newTimeline,
                undoRefused = keptUndoRefusals(s, newBrief, newTimeline)
            )
        }
    }

    // ─── Goal portfolio (A2) ───────────────────────────────────────────────────

    fun addGoal(kind: com.forge.app.domain.coach.CoachGoalKind, targetKey: String, targetValue: Double?) =
        viewModelScope.launch {
            runCatching { goalRepo.add(kind, targetKey, targetValue) }
            refreshGoals()
        }

    private suspend fun refreshGoals() {
        // Read first, then update atomically — the same rule [refreshProjects] states: assembling a
        // copy() around suspending reads reverts whatever landed in between.
        val goals = runCatching { goalRepo.states() }.getOrNull()
        val conflicts = runCatching { goalRepo.conflicts() }.getOrNull()
            ?.also { if (it.isNotEmpty()) runCatching { academyRepo.onGoalConflict() } }
        _state.update {
            it.copy(
                goals = goals ?: it.goals,
                goalConflicts = conflicts ?: it.goalConflicts
            )
        }
    }

    // ─── Training block (C) ────────────────────────────────────────────────────

    /** Start a block around the athlete's top goal. Announced, never silent. */
    fun startBlock() = viewModelScope.launch {
        val weekId = _state.value.brief?.pass?.weekId ?: return@launch
        if (!claimBlockAction()) return@launch
        try {
            runCatching { blockRepo.start(weekId = weekId) }
            val block = runCatching { blockRepo.active() }.getOrNull()
            _state.update { it.copy(block = block) }
        } finally {
            _state.update { it.copy(blockBusy = false) }
        }
    }

    /** End it early — the user's veto is always one tap away. */
    fun endBlock() = viewModelScope.launch {
        if (!claimBlockAction()) return@launch
        try {
            runCatching { blockRepo.end() }
            _state.update { it.copy(block = null) }
        } finally {
            _state.update { it.copy(blockBusy = false) }
        }
    }

    /**
     * Claim the block action for one start or end, or report that one is already in flight. The
     * repository serialises the writes; this keeps a second tap from queueing a second one behind
     * the first (M-13).
     */
    private fun claimBlockAction(): Boolean {
        var claimed = false
        _state.update { s ->
            claimed = !s.blockBusy
            if (claimed) s.copy(blockBusy = true) else s
        }
        return claimed
    }

    // ─── Projects (D) ──────────────────────────────────────────────────────────

    /** Start a specific project the user chose from [UiState.projectOptions]. */
    fun startProject(candidate: com.forge.app.domain.coach.ProjectScanner.Candidate) =
        viewModelScope.launch {
            runCatching { projectRepo.accept(candidate) }
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
        // The charts cover the same window as the input counts above them (coachLab's
        // windowStart): one night synced after a five-week gap must not chart the old nights.
        val sleep = s.health.sleepNights.filter { it.endedAtMs >= windowStart }
            .sortedBy { it.endedAtMs }
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
            restingHr = hr.filter { it.timeMs >= windowStart }.takeLast(HR_READINGS_SHOWN).map { it.bpm },
            sleepSynced = s.health.sleepNights.isNotEmpty(),
            restingHrSynced = hr.isNotEmpty(),
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
        /** How long "Remind me later" holds the advanced-tracking pop-up back: one week. */
        const val ADVANCED_PROMPT_SNOOZE_MS = 7L * 24 * 60 * 60 * 1000
    }
}
