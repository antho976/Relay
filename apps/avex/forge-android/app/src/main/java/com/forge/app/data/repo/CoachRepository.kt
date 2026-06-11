package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CoachDao
import com.forge.app.data.db.dao.ExerciseCustomizationDao
import com.forge.app.data.db.dao.ProgramCustomizationDao
import com.forge.app.data.db.dao.VacationDao
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.CoachPass
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.coach.CoachPassInputs
import com.forge.app.domain.coach.CoachPassStatus
import com.forge.app.domain.coach.OutcomeWatcher
import com.forge.app.domain.coach.TrustLedger
import com.forge.app.domain.coach.TypeTrust
import com.forge.app.domain.coach.WeeklyReview
import com.forge.app.domain.coach.WeeklyReviewData
import com.forge.app.domain.vacation.VacationCalendar
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.MuscleGroup
import kotlinx.coroutines.flow.first
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.IsoFields
import java.time.temporal.TemporalAdjusters
import javax.inject.Inject
import javax.inject.Singleton

/** The one-line "new report ready" banner shown on Overview until the brief is seen. */
data class CoachBanner(val weekId: String, val text: String)

/** Everything the Week Brief screen renders: the pass, its decisions, and the review numbers. */
data class CoachBrief(
    val pass: CoachPass,
    val decisions: List<CoachDecision>,
    /** Null when the review itself failed to assemble — the Brief still renders the pass. */
    val review: WeeklyReviewData?
)

/**
 * The Weekly Coach Pass trigger + Week Brief assembly + the propose/apply lifecycle
 * (auto-coach Phase 3).
 *
 * The pass is idempotent by ISO week id (hardening 10) and every pass writes a row —
 * proposed, hold, vacation-hold, or error (hardening 4 + 13). Before planning, the watcher
 * judges previously applied changes (hardening 5); failed ones come back as revert proposals.
 *
 * Applying NEVER rebuilds the program: every decision routes through the same write paths
 * user actions use (persistent swap, rep/sets overlay, the existing deload apply), each
 * recording its before-state for single-change undo (hardening 6 + 8). Slots the user has
 * customized are never targeted (hardening 9 — filtered at planning time).
 */
@Singleton
class CoachRepository @Inject constructor(
    private val coachDao: CoachDao,
    private val adaptationRepository: AdaptationRepository,
    private val settings: SettingsRepository,
    private val vacationDao: VacationDao,
    private val customizationRepo: CustomizationRepository,
    private val programCustomizationRepo: ProgramCustomizationRepository,
    private val programCustomizationDao: ProgramCustomizationDao,
    private val exerciseCustomizationDao: ExerciseCustomizationDao,
    private val clock: Clock
) {
    companion object {
        const val STATUS_SHADOW = "shadow"
        const val STATUS_PROPOSED = "proposed"
        const val STATUS_APPLIED = "applied"
        const val STATUS_SKIPPED = "skipped"
        const val STATUS_HOLD = "hold"
        const val STATUS_ERROR = "error"

        /** undo_data sentinel: there was no prior override/swap — undo removes, not restores. */
        private const val NONE = "∅"
        /** Drift window for the per-muscle ±2 set cap (hardening 11). */
        private const val DRIFT_WINDOW_DAYS = 56L
        private const val DAY_MS = 24L * 60 * 60 * 1000
    }

    /** Run this week's pass if it hasn't run yet; return the (existing or fresh) record. */
    suspend fun ensureWeeklyPass(): CoachPass {
        val zone = ZoneId.systemDefault()
        val today = Instant.ofEpochMilli(clock.nowMs()).atZone(zone).toLocalDate()
        val weekId = weekId(today)
        coachDao.pass(weekId)?.let { return it }

        var decisions: List<CoachDecision> = emptyList()
        val pass = runCatching {
            if (VacationCalendar.onVacation(vacationDao.all())(today)) {
                CoachPass(weekId, clock.nowMs(), STATUS_HOLD,
                    "On vacation — the coach is paused until you're back. Enjoy it.")
            } else {
                val snapshot = adaptationRepository.snapshot()
                val inputs = assembleInputs(snapshot)
                val result = AutoCoachPlanner.evaluate(snapshot, inputs)
                when (result.status) {
                    CoachPassStatus.SHADOW -> {
                        decisions = result.decisions.map {
                            CoachDecision(
                                weekId = weekId, type = it.type, targetKey = it.targetKey,
                                targetName = it.targetName, summary = it.summary,
                                reason = it.reason, status = STATUS_PROPOSED,
                                dayKey = it.dayKey, payload = it.payload
                            )
                        }
                        CoachPass(weekId, clock.nowMs(), STATUS_PROPOSED, null)
                    }
                    else -> CoachPass(weekId, clock.nowMs(), STATUS_HOLD, result.holdReason)
                }
            }
        }.getOrElse { e ->
            // Fail loud: an errored pass is its own status, never a silent hold.
            CoachPass(weekId, clock.nowMs(), STATUS_ERROR,
                "Couldn't evaluate this week (${e.message ?: e.javaClass.simpleName}) — no changes were considered.")
        }

        coachDao.insertPass(pass)
        if (decisions.isNotEmpty()) {
            coachDao.insertDecisions(decisions)
            autoApplyEarnedTypes(weekId)
        }
        return pass
    }

    /**
     * Autopilot (Phase 4): in "auto" mode, decisions whose TYPE has earned trust (TrustLedger —
     * consecutive accepted proposals, demoted on any bad outcome) apply themselves; everything
     * else still waits for a tap. Trust is judged on history BEFORE this week's fresh rows
     * (undecided proposals never count), and an errored pass has no decisions, so the
     * no-auto-apply-on-error rule (hardening 13) holds by construction.
     */
    private suspend fun autoApplyEarnedTypes(weekId: String) {
        if (settings.coachMode.first() != "auto") return
        val earned = TrustLedger.earnedTypes(coachDao.allDecisions().filter { it.weekId != weekId })
        if (earned.isEmpty()) return
        coachDao.decisionsFor(weekId)
            .filter { it.status == STATUS_PROPOSED && it.type in earned }
            .forEach { runCatching { applyDecision(it.id) } }
    }

    /** Per-type earned-trust readout for the Settings → Coach page. */
    suspend fun trust(): List<TypeTrust> = TrustLedger.assess(coachDao.allDecisions())

    /** Coach history: recent passes with their decisions, newest first. */
    data class CoachHistoryEntry(val pass: CoachPass, val decisions: List<CoachDecision>)

    suspend fun history(limit: Int = 12): List<CoachHistoryEntry> =
        coachDao.recentPasses(limit).map { CoachHistoryEntry(it, coachDao.decisionsFor(it.weekId)) }

    /**
     * Watcher pre-pass + planner inputs. Runs only when a fresh weekly pass is being created,
     * so outcome verdicts land on a weekly cadence (the watcher's window math handles the rest).
     */
    private suspend fun assembleInputs(snapshot: AdaptationSnapshot): CoachPassInputs {
        // 1. Judge previously applied changes (hardening 5).
        val pending = coachDao.appliedPendingOutcome()
        val verdicts = OutcomeWatcher.evaluate(pending, snapshot)
        verdicts.forEach { coachDao.setOutcome(it.decisionId, it.outcome) }
        val reverts = OutcomeWatcher.revertProposals(pending, verdicts)

        // 2. Coach-locked slots (hardening 9): any user customization locks its slot — minus
        // targets the coach itself changed (a coach swap must stay revertable, not self-lock).
        val coachTouched = coachDao.appliedSince(0L).map { it.targetKey }.toSet()
        val locked = (
            programCustomizationDao.all().map { it.exerciseId } +
                exerciseCustomizationDao.all().filter { it.swappedName.isNotBlank() }.map { it.exerciseId }
            ).toSet() - coachTouched

        // 3. Anti-oscillation (hardening 11): muscles with a volume change still in (or just
        // failing) its outcome window are locked; net drift per muscle over the window is capped.
        val slotMuscle = snapshot.program.flatMap { it.slots }.associate { it.exerciseId to it.muscle }
        fun muscleOf(d: CoachDecision): MuscleGroup? = slotMuscle[d.targetKey]
        val stillPending = pending.filter { p -> verdicts.none { it.decisionId == p.id } }
        val volumeLocked = (stillPending + pending.filter { p ->
            verdicts.any { it.decisionId == p.id && it.outcome == "failed" }
        }).filter { it.type.startsWith("volume") }.mapNotNull(::muscleOf).toSet()
        val net = coachDao.appliedSince(clock.nowMs() - DRIFT_WINDOW_DAYS * DAY_MS)
            .filter { it.type.startsWith("volume") }
            .mapNotNull { d -> muscleOf(d)?.let { it to if (d.type == "volume_up") 1 else -1 } }
            .groupBy({ it.first }, { it.second })
            .mapValues { (_, deltas) -> deltas.sum() }

        return CoachPassInputs(
            experience = settings.programExperience.first(),
            sessionsTarget = settings.daysPerWeek.first(),
            lockedExerciseIds = locked,
            volumeLockedMuscles = volumeLocked,
            volumeNetByMuscle = net,
            revertProposals = reverts
        )
    }

    // ─── Apply / skip / undo (every write is an existing user path) ──────────

    /** One-tap apply for a proposed decision; records the before-state for undo. */
    suspend fun applyDecision(id: Long) {
        val d = coachDao.decision(id) ?: return
        if (d.status != STATUS_PROPOSED) return
        when (d.type) {
            "deload" -> {
                // The one non-delta apply: the existing deload regeneration (not undoable —
                // regenerating again is the way back; its reconcile also clears overlay rows).
                adaptationRepository.applyDeloadWeek()
                coachDao.markApplied(id, clock.nowMs(), null)
            }
            "swap" -> {
                val def = d.payload?.let { ExerciseLibrary.byId(it) } ?: return
                val prev = customizationRepo.getSwap(d.targetKey)
                    ?.takeIf { it.swappedName.isNotBlank() }
                customizationRepo.setSwap(d.targetKey, def.name, def.unit.code)
                coachDao.markApplied(id, clock.nowMs(), prev?.let { "${it.swappedName}|${it.swappedUnit}" } ?: NONE)
            }
            "rep_shift" -> {
                val to = d.payload ?: return
                val prev = programCustomizationRepo.overrideFor(d.dayKey, d.targetKey)?.repRangeOverride
                programCustomizationRepo.setRepRange(d.dayKey, d.targetKey, to)
                coachDao.markApplied(id, clock.nowMs(), prev ?: NONE)
            }
            "volume_up", "volume_down" -> {
                val newSets = d.payload?.toIntOrNull() ?: return
                val prev = programCustomizationRepo.overrideFor(d.dayKey, d.targetKey)?.setsOverride ?: 0
                programCustomizationRepo.setSetsOverride(d.dayKey, d.targetKey, newSets)
                coachDao.markApplied(id, clock.nowMs(), prev.toString())
            }
            "revert" -> {
                val originalId = d.payload?.toLongOrNull() ?: return
                undoDecision(originalId)
                coachDao.markApplied(id, clock.nowMs(), null)
            }
            else -> return
        }
    }

    /** Apply every still-proposed decision of [weekId], in order. */
    suspend fun applyAll(weekId: String) =
        coachDao.decisionsFor(weekId).filter { it.status == STATUS_PROPOSED }.forEach { applyDecision(it.id) }

    suspend fun skipDecision(id: Long) {
        val d = coachDao.decision(id) ?: return
        if (d.status == STATUS_PROPOSED) coachDao.setStatus(id, STATUS_SKIPPED)
    }

    /**
     * Single-change undo (hardening 6): restore the recorded before-state through the same
     * write path, mark reverted + failed (a user undo IS a failed outcome).
     */
    suspend fun undoDecision(id: Long) {
        val d = coachDao.decision(id) ?: return
        if (d.status != STATUS_APPLIED) return
        when (d.type) {
            "swap" -> {
                val prev = d.undoData
                if (prev == null || prev == NONE) customizationRepo.clearSwap(d.targetKey)
                else prev.split("|").let {
                    customizationRepo.setSwap(d.targetKey, it[0], it.getOrElse(1) { "" })
                }
            }
            "rep_shift" -> {
                if (d.undoData == null || d.undoData == NONE)
                    programCustomizationRepo.clearRepRange(d.dayKey, d.targetKey)
                else programCustomizationRepo.setRepRange(d.dayKey, d.targetKey, d.undoData)
            }
            "volume_up", "volume_down" ->
                programCustomizationRepo.setSetsOverride(d.dayKey, d.targetKey, d.undoData?.toIntOrNull() ?: 0)
            else -> return // deload / revert aren't mechanically undoable
        }
        coachDao.markReverted(id)
    }

    // ─── Brief assembly ────────────────────────────────────────────────────────

    /** The full Week Brief: ensures the pass exists, then assembles review + decisions. */
    suspend fun brief(): CoachBrief {
        val pass = ensureWeeklyPass()
        return briefFor(pass)
    }

    /** Re-read the Brief without re-running the pass — after an apply/skip/undo tap. */
    suspend fun refreshBrief(): CoachBrief? = coachDao.latestPass()?.let { briefFor(it) }

    private suspend fun briefFor(pass: CoachPass): CoachBrief {
        val decisions = coachDao.decisionsFor(pass.weekId)
        val review = runCatching {
            WeeklyReview.assemble(
                s = adaptationRepository.snapshot(),
                weekStartMs = weekStartMs(),
                sessionsTarget = settings.daysPerWeek.first(),
                hasDeloadShadow = decisions.any { it.type == "deload" }
            )
        }.getOrNull()
        return CoachBrief(pass, decisions, review)
    }

    /** One-line summary of where [pass] landed (Overview banner + Settings entry). */
    suspend fun summaryFor(pass: CoachPass): String {
        if (pass.status == STATUS_ERROR) return "Coach · couldn't run this week"
        val ds = coachDao.decisionsFor(pass.weekId)
        val applied = ds.count { it.status == STATUS_APPLIED }
        val open = ds.count { it.status == STATUS_PROPOSED }
        return when {
            open > 0 -> "Coach · $open proposal(s) for this week"
            applied > 0 -> "Coach · $applied change(s) applied this week"
            pass.status == STATUS_SHADOW -> "Coach · observations ready for this week"
            else -> "Coach · holding steady this week"
        }
    }

    /**
     * The "new report" banner for Overview (auto-coach Phase 1 entry-point rework): ensures
     * this week's pass, then returns its summary ONLY if the user hasn't seen it yet. Null once
     * the brief has been opened or the banner dismissed (both call [markSeen]).
     */
    suspend fun pendingBanner(): CoachBanner? {
        val pass = ensureWeeklyPass()
        if (pass.weekId == settings.lastSeenCoachWeekId.first()) return null
        return CoachBanner(pass.weekId, summaryFor(pass))
    }

    /** Mark a week's brief as seen — clears the Overview banner (opened or dismissed). */
    suspend fun markSeen(weekId: String) = settings.setLastSeenCoachWeekId(weekId)

    private fun weekId(date: LocalDate): String = "%d-W%02d".format(
        date.get(IsoFields.WEEK_BASED_YEAR), date.get(IsoFields.WEEK_OF_WEEK_BASED_YEAR)
    )

    private fun weekStartMs(): Long {
        val zone = ZoneId.systemDefault()
        return Instant.ofEpochMilli(clock.nowMs()).atZone(zone).toLocalDate()
            .with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
            .atStartOfDay(zone).toInstant().toEpochMilli()
    }
}
