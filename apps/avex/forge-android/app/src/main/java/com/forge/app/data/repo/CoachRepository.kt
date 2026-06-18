package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CoachDao
import com.forge.app.data.db.dao.ExerciseCustomizationDao
import com.forge.app.data.db.dao.ProgramCustomizationDao
import com.forge.app.data.db.dao.VacationDao
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.CoachPass
import com.forge.app.data.db.entities.OverlaySource
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.ProgressionAdvisor
import com.forge.app.domain.adapt.Recommendation
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.coach.CoachGenBias
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
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
    val review: WeeklyReviewData?,
    /** Finished tracked sessions so far — drives the "still learning, N to go" subtitle (CO1). */
    val sessionsLogged: Int = 0,
    /** Sessions needed before the weekly pass starts calling adjustments. */
    val minSessions: Int = 0,
    /**
     * Changes applied from the PREVIOUS brief (last week's pass) — shown as a "what changed"
     * delta so the user can see whether a past change is now in effect. Empty for the very first
     * brief or when nothing was applied last week.
     */
    val previousApplied: List<CoachDecision> = emptyList()
) {
    /** Sessions remaining before the coach activates; 0 once it's calling weekly adjustments. */
    val sessionsToGo: Int get() = (minSessions - sessionsLogged).coerceAtLeast(0)
}

/**
 * The "Coach lab" read-out (finding #5): a plain-language portrait of what the coach is
 * currently watching and how far along its learning is — surfaced so a quiet coach reads as
 * "still gathering data", not broken. Nothing here is a verdict; it's the inputs, labelled.
 */
data class CoachWatch(
    val sessionsLogged: Int,
    /** Finished sessions needed before the weekly pass starts calling adjustments. */
    val minSessions: Int,
    /** max(0, minSessions − sessionsLogged) — 0 once the coach is active. */
    val sessionsToGo: Int,
    /** True when earned changes apply themselves (Settings → coach mode "auto"). */
    val autopilot: Boolean,
    /** Current accumulated-fatigue score, or null below the deload data gates. */
    val fatigueScore: Int?,
    val fatigueThreshold: Int,
    /** Lifts with logged history, "concrete" once they clear the judge-it gate. */
    val trackedLifts: List<TrackedLift>,
    /** Recovery inputs the coach reads — each active or "still forming". */
    val recoverySignals: List<RecoverySignal>,
    /** What the coach has learned from changes you've accepted (empty when it hasn't yet). */
    val learnedBiases: List<LearnedBias>
)

data class TrackedLift(val name: String, val bouts: Int, val concrete: Boolean, val stalling: Boolean = false)
data class RecoverySignal(val label: String, val detail: String, val active: Boolean)
data class LearnedBias(val label: String, val detail: String)

/**
 * The "Coach learning timeline" (Tier 6 narrative surface): how the coach has grown over time —
 * its earned trust per change type, the journey milestones it's hit (and the ones still ahead),
 * and the week-by-week record. Read-only; everything here is derived from the coach's own ledger.
 */
data class CoachTimeline(
    val trust: List<TypeTrust>,
    val milestones: List<CoachMilestone>,
    val weeks: List<CoachRepository.CoachHistoryEntry>
)

/** One step of the coach's journey — a named achievement, [reached] or still ahead. */
data class CoachMilestone(val label: String, val detail: String, val reached: Boolean)

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
        /** A delta a regenerate has baked into the baseline: still feeds the bias, no longer undoable. */
        const val STATUS_FOLDED = "folded"

        /** undo_data sentinel: there was no prior override/swap — undo removes, not restores. */
        private const val NONE = "∅"
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
            // Don't launder a cancelled scope into a persisted STATUS_ERROR pass — let cancellation unwind.
            if (e is kotlinx.coroutines.CancellationException) throw e
            // Fail loud: an errored pass is its own status, never a silent hold.
            CoachPass(weekId, clock.nowMs(), STATUS_ERROR,
                "Couldn't evaluate this week (${e.message ?: e.javaClass.simpleName}) — no changes were considered.")
        }

        // Atomic + idempotent (seam fix, finding 14): pass + decisions commit in ONE transaction, so
        // process death can't leave a permanently empty "proposed" pass; the pass PK uses IGNORE, so a
        // concurrent caller that lost the race no-ops (no crash, no doubled decisions) and reads the
        // winner's row below instead.
        val won = coachDao.insertPassWithDecisions(pass, decisions)
        if (won && decisions.isNotEmpty()) autoApplyEarnedTypes(weekId)
        return coachDao.pass(weekId) ?: pass
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
     * The Coach learning timeline (Tier 6): earned trust + journey milestones + the week-by-week
     * record, all from the coach's own ledger. One read for the whole narrative screen.
     */
    suspend fun timeline(limit: Int = 26): CoachTimeline {
        val passes = coachDao.recentPasses(limit)
        val all = coachDao.allDecisions()
        val trust = TrustLedger.assess(all)
        // Group the decisions we already loaded by week instead of a per-pass query (was 1 + N).
        val byWeek = all.groupBy { it.weekId }
        val weeks = passes.map { CoachHistoryEntry(it, byWeek[it.weekId].orEmpty().sortedBy { d -> d.id }) }
        return CoachTimeline(trust = trust, milestones = coachMilestones(passes, all, trust), weeks = weeks)
    }

    /** Fixed achievement ladder, each marked reached/ahead from the pass + decision ledger. */
    private fun coachMilestones(
        passes: List<CoachPass>,
        all: List<CoachDecision>,
        trust: List<TypeTrust>
    ): List<CoachMilestone> {
        // "Applied" includes folded (baked into the baseline) and reverted (applied, then undone) —
        // both crossed the apply line once. "Wins" are changes the watcher later judged ok.
        val applied = all.count { it.status == "applied" || it.status == "folded" || it.status == "reverted" }
        val wins = all.count { it.outcome == "ok" }
        val earned = trust.filter { it.earned }
        val proposed = all.isNotEmpty()
        return listOf(
            CoachMilestone(
                "First weekly report",
                if (passes.isNotEmpty()) "${passes.size} week${if (passes.size == 1) "" else "s"} on record" else "After your first full week",
                passes.isNotEmpty()
            ),
            CoachMilestone(
                "First suggestion made",
                if (proposed) "The coach has started calling changes" else "Once it has a baseline to act on",
                proposed
            ),
            CoachMilestone(
                "First change applied",
                if (applied > 0) "$applied accepted so far" else "When you apply a weekly suggestion",
                applied > 0
            ),
            CoachMilestone(
                "First win confirmed",
                if (wins > 0) "$wins change${if (wins == 1) "" else "s"} stuck after the watch window" else "When an applied change proves out",
                wins > 0
            ),
            CoachMilestone(
                "Autopilot unlocked",
                if (earned.isNotEmpty()) earned.joinToString(", ") { it.label } else "Accept a change type a few weeks running",
                earned.isNotEmpty()
            )
        )
    }

    /**
     * Watcher pre-pass + planner inputs. Runs only when a fresh weekly pass is being created,
     * so outcome verdicts land on a weekly cadence (the watcher's window math handles the rest).
     */
    private suspend fun assembleInputs(snapshot: AdaptationSnapshot): CoachPassInputs {
        // 1. Judge previously applied changes (hardening 5) — including in-window folds, so a refresh
        // that baked a change in before its window can't shield it from an ok/failed verdict (finding
        // P1). Then re-derive revert proposals from the durable outcome column — every still-applied
        // FAILED change owes a revert, even if a prior pass's proposal was dropped by deload-supersession
        // / the cap / an error (seam fix, finding 4). A folded-then-failed change isn't revertable in
        // place (the overlay is gone); it self-heals by dropping out of the bias at the next generate.
        val pending = coachDao.pendingOutcome()
        val verdicts = OutcomeWatcher.evaluate(pending, snapshot)
        verdicts.forEach { coachDao.setOutcome(it.decisionId, it.outcome) }
        val reverts = OutcomeWatcher.revertProposalsFor(coachDao.appliedFailed())

        val allDecisions = coachDao.allDecisions()

        // 2. Coach-locked slots (hardening 9): a USER customization locks its slot. Keyed off the
        // overlay's origin tag, NOT an all-time "coach-touched" subtraction — that hack both stripped
        // protection from genuine user edits on coach-touched slots and let inert coach residue count
        // as a user lock (seam findings 5/6). A live coach overlay (source=coach) simply isn't a lock,
        // so the coach can still revert its own change without self-locking. Targets of pending revert
        // proposals are also locked so a NEW structural change can't stack on a slot mid-revert (#8).
        val locked = (
            programCustomizationDao.all()
                .filter {
                    it.source == OverlaySource.USER &&
                        (it.setsOverride > 0 || it.repRangeOverride != null || it.removed ||
                            it.exerciseId.startsWith("custom_"))
                }
                .map { it.exerciseId } +
                exerciseCustomizationDao.all()
                    .filter { it.source == OverlaySource.USER && it.swappedName.isNotBlank() }
                    .map { it.exerciseId } +
                reverts.map { it.targetKey }
            ).toSet()

        // 3. Anti-oscillation (hardening 11): muscles with a volume change still in (or just
        // failing) its outcome window are locked; net drift per muscle is capped.
        val slotMuscle = snapshot.program.flatMap { it.slots }.associate { it.exerciseId to it.muscle }
        fun muscleOf(d: CoachDecision): MuscleGroup? = slotMuscle[d.targetKey]
        val stillPending = pending.filter { p -> verdicts.none { it.decisionId == p.id } }
        val volumeLocked = (stillPending + pending.filter { p ->
            verdicts.any { it.decisionId == p.id && it.outcome == "failed" }
        }).filter { it.type.startsWith("volume") }.mapNotNull(::muscleOf).toSet()
        // The drift cap reads the SAME derivation the baseline is folded from (CoachGenBias) — one
        // source of truth, so the planner can never re-add volume it already donated to the baseline.
        // (Previously a 56-day/current-slot window that decayed while the baked bias did not — the
        // ratchet-past-cap bug, seam finding 16.)
        val net = CoachGenBias.from(allDecisions).volumeBias

        // 4. Structural decline memory (finding 19): a swap/rep_shift whose MOST RECENT decision was
        // skipped or reverted is "declined" — the planner won't re-propose that identical change, so a
        // rejected rotation stops reappearing every week. A later applied decision clears the decline.
        val declined = allDecisions
            .filter { it.type == "swap" || it.type == "rep_shift" }
            .filter { it.payload != null }
            .groupBy { "${it.type}:${it.targetKey}:${it.payload}" }
            .filterValues { rows -> rows.maxByOrNull { it.id }!!.status in setOf(STATUS_SKIPPED, "reverted") }
            .keys

        return CoachPassInputs(
            experience = settings.programExperience.first(),
            sessionsTarget = settings.daysPerWeek.first(),
            lockedExerciseIds = locked,
            volumeLockedMuscles = volumeLocked,
            volumeNetByMuscle = net,
            revertProposals = reverts,
            declinedStructural = declined
        )
    }

    // ─── Apply / skip / undo (every write is an existing user path) ──────────

    /**
     * Serializes the lifecycle mutations. The Brief/Settings fire one coroutine per tap with no
     * in-flight guard, so a double-tap Apply (or Apply+Skip) could otherwise interleave read-then-
     * write across the suspend DAO calls and corrupt undo_data or orphan a live overlay (seam #12).
     */
    private val lifecycleMutex = Mutex()

    /** True if the slot the decision targets now carries the USER's own customization (finding 7). */
    private suspend fun userOwnsSlot(d: CoachDecision): Boolean = when (d.type) {
        "swap" -> customizationRepo.getSwap(d.targetKey)
            ?.let { it.source == OverlaySource.USER && it.swappedName.isNotBlank() } ?: false
        "rep_shift", "volume_up", "volume_down" -> programCustomizationRepo.overrideFor(d.dayKey, d.targetKey)
            ?.let { it.source == OverlaySource.USER && (it.setsOverride > 0 || it.repRangeOverride != null || it.removed) }
            ?: false
        else -> false
    }

    /** True if a newer still-active coach decision owns this slot's overlay field (LIFO undo, #9). */
    private suspend fun newerCoachDecisionOwnsSlot(d: CoachDecision): Boolean {
        val newer = coachDao.activeOnTargetAfter(d.targetKey, d.id)
        return when (d.type) {
            "swap" -> newer.any { it.type == "swap" }
            "rep_shift" -> newer.any { it.type == "rep_shift" && it.dayKey == d.dayKey }
            "volume_up", "volume_down" -> newer.any { it.type.startsWith("volume") && it.dayKey == d.dayKey }
            else -> false
        }
    }

    /** One-tap apply for a proposed decision; records the before-state for undo. */
    suspend fun applyDecision(id: Long) = lifecycleMutex.withLock { applyDecisionLocked(id) }

    private suspend fun applyDecisionLocked(id: Long) {
        val d = coachDao.decision(id) ?: return
        if (d.status != STATUS_PROPOSED) return
        // Re-validate against the CURRENT program (locks were computed at pass time): if the user has
        // customized this slot since the pass ran, applying would silently clobber their edit — skip
        // it instead (seam fix, finding 7). deload/revert aren't slot-scoped, so they're exempt.
        if (userOwnsSlot(d)) { coachDao.setStatus(id, STATUS_SKIPPED); return }
        when (d.type) {
            "deload" -> {
                // The one non-delta apply: the existing deload regeneration (not undoable —
                // regenerating again is the way back; its reconcile also clears overlay rows).
                adaptationRepository.applyDeloadWeek()
                coachDao.markApplied(id, clock.nowMs(), null)
            }
            "swap" -> {
                val def = d.payload?.let { ExerciseLibrary.byId(it) } ?: return
                // Capture the before-state if the row carries a swap OR a bare unit override — a
                // unit-only row (blank name, non-blank unit) was previously recorded as "nothing" and
                // its unit destroyed on undo (seam fix, finding 11). undo splits "name|unit" back, so a
                // blank name restores the unit-only override cleanly.
                val prev = customizationRepo.getSwap(d.targetKey)
                    ?.takeIf { it.swappedName.isNotBlank() || it.swappedUnit.isNotBlank() }
                customizationRepo.setSwap(d.targetKey, def.name, def.unit.code, source = OverlaySource.COACH, swappedExerciseId = def.id)
                // Record name|unit|swapId so undo restores the exact prior swap — including its
                // re-attribution id (#11). Without the id, undo would strand the coach's id on the
                // restored row and mis-attribute every PR/stat. Old 2-part rows parse with a null id.
                coachDao.markApplied(id, clock.nowMs(), prev?.let { encodeSwapUndo(it.swappedName, it.swappedUnit, it.swappedExerciseId) } ?: NONE)
            }
            "rep_shift" -> {
                val to = d.payload ?: return
                val prev = programCustomizationRepo.overrideFor(d.dayKey, d.targetKey)?.repRangeOverride
                programCustomizationRepo.setRepRange(d.dayKey, d.targetKey, to, source = OverlaySource.COACH)
                coachDao.markApplied(id, clock.nowMs(), prev ?: NONE)
            }
            "volume_up", "volume_down" -> {
                val newSets = d.payload?.toIntOrNull() ?: return
                val prev = programCustomizationRepo.overrideFor(d.dayKey, d.targetKey)?.setsOverride ?: 0
                programCustomizationRepo.setSetsOverride(d.dayKey, d.targetKey, newSets, source = OverlaySource.COACH)
                coachDao.markApplied(id, clock.nowMs(), prev.toString())
            }
            "revert" -> {
                val originalId = d.payload?.toLongOrNull() ?: return
                undoDecisionLocked(originalId)
                coachDao.markApplied(id, clock.nowMs(), null)
            }
            else -> return
        }
    }

    /** Apply every still-proposed decision of [weekId], in order. */
    suspend fun applyAll(weekId: String) =
        coachDao.decisionsFor(weekId).filter { it.status == STATUS_PROPOSED }.forEach { applyDecision(it.id) }

    suspend fun skipDecision(id: Long) = lifecycleMutex.withLock {
        val d = coachDao.decision(id) ?: return@withLock
        if (d.status == STATUS_PROPOSED) coachDao.setStatus(id, STATUS_SKIPPED)
    }

    /**
     * Single-change undo (hardening 6): restore the recorded before-state through the same write
     * path, mark reverted + failed (a user undo IS a failed outcome).
     *
     * Compare-and-restore: the overlay is rewritten ONLY while it still holds the coach's own write
     * (source=coach). If the user has since edited the slot, or a regenerate already cleared/folded
     * the overlay, a blind restore would destroy the user's edit (finding 10) or write a stale
     * absolute onto a fresh program (finding 0) — in those cases the decision is just retired (the
     * baseline/overlay already moved on; folded volume drops out of the bias at the next generate).
     * A restored before-state belongs to the user again, so it's written back as source=user.
     */
    suspend fun undoDecision(id: Long) = lifecycleMutex.withLock { undoDecisionLocked(id) }

    /** Serialize a swap's prior state for undo as JSON — robust to '|' or any char in the name (#11). */
    private fun encodeSwapUndo(name: String, unit: String, swapId: String?): String =
        org.json.JSONObject().apply {
            put("name", name); put("unit", unit); put("swapId", swapId ?: "")
        }.toString()

    /** Parse a swap undo record: JSON for new rows, legacy "name|unit[|swapId]" for old ones (#11). */
    private fun decodeSwapUndo(raw: String): Triple<String, String, String?> {
        if (raw.startsWith("{")) runCatching {
            val o = org.json.JSONObject(raw)
            return Triple(o.optString("name"), o.optString("unit"), o.optString("swapId").ifBlank { null })
        }
        // A corrupt/empty legacy record ("" or "|" — possible after a failed restore or migration edge)
        // splits to a blank name; surface it explicitly so the caller can avoid writing it into a slot.
        if (raw.isBlank()) return Triple("", "", null)
        return raw.split("|").let {
            Triple(it[0], it.getOrElse(1) { "" }, it.getOrNull(2)?.ifBlank { null })
        }
    }

    private suspend fun undoDecisionLocked(id: Long) {
        val d = coachDao.decision(id) ?: return
        if (d.status != STATUS_APPLIED) return
        // Per-slot LIFO: if a newer still-active coach decision owns this slot's overlay, refuse —
        // restoring the older before-state would silently wipe the newer change, and the watcher's
        // own revert pipeline reverts newest-first so the chain unwinds cleanly (seam fix, finding 9).
        if (newerCoachDecisionOwnsSlot(d)) return
        when (d.type) {
            "swap" -> {
                if (customizationRepo.getSwap(d.targetKey)?.source == OverlaySource.COACH) {
                    val prev = d.undoData
                    if (prev == null || prev == NONE) customizationRepo.clearSwap(d.targetKey)
                    else {
                        // restoreSwap (not setSwap) force-sets the id, so a prior swap with no
                        // re-attribution id clears the coach's instead of inheriting it (#11).
                        val (name, unit, swapId) = decodeSwapUndo(prev)
                        // A corrupt/blank undo record would otherwise restore a BLANK exercise name into
                        // the slot (renders as an empty name on the session screen). Drop the coach overlay
                        // entirely instead, so the slot falls back to the real base-program exercise (#11).
                        if (name.isBlank()) customizationRepo.clearSwap(d.targetKey)
                        else customizationRepo.restoreSwap(
                            d.targetKey, name, unit,
                            source = OverlaySource.USER,
                            swappedExerciseId = swapId
                        )
                    }
                }
            }
            "rep_shift" -> {
                if (programCustomizationRepo.overrideFor(d.dayKey, d.targetKey)?.source == OverlaySource.COACH) {
                    if (d.undoData == null || d.undoData == NONE)
                        programCustomizationRepo.clearRepRange(d.dayKey, d.targetKey)
                    else programCustomizationRepo.setRepRange(d.dayKey, d.targetKey, d.undoData, source = OverlaySource.USER)
                }
            }
            "volume_up", "volume_down" -> {
                if (programCustomizationRepo.overrideFor(d.dayKey, d.targetKey)?.source == OverlaySource.COACH)
                    programCustomizationRepo.setSetsOverride(
                        d.dayKey, d.targetKey, d.undoData?.toIntOrNull() ?: 0, source = OverlaySource.USER
                    )
            }
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
        // Snapshot once and reuse: feeds the review AND the activation-progress subtitle (CO1).
        val snapshot = runCatching { adaptationRepository.snapshot() }.getOrNull()
        val review = snapshot?.let { snap ->
            runCatching {
                WeeklyReview.assemble(
                    s = snap,
                    weekStartMs = weekStartMs(),
                    sessionsTarget = settings.daysPerWeek.first(),
                    hasDeloadShadow = decisions.any { it.type == "deload" }
                )
            }.getOrNull()
        }
        // Previous brief's applied changes — the "what changed" delta for the user. Reads the two most
        // recent passes and takes decisions from the one BEFORE this week's pass that were actually applied
        // (applied, folded, or reverted all crossed the apply line). Falls back silently on any error.
        val previousApplied = runCatching {
            val recent = coachDao.recentPasses(2)
            val prevWeekId = recent.firstOrNull { it.weekId != pass.weekId }?.weekId
            if (prevWeekId != null) {
                coachDao.decisionsFor(prevWeekId)
                    .filter { it.status == STATUS_APPLIED || it.status == STATUS_FOLDED || it.status == "reverted" }
            } else emptyList()
        }.getOrDefault(emptyList())
        return CoachBrief(
            pass, decisions, review,
            sessionsLogged = snapshot?.sessions?.size ?: 0,
            // When the snapshot failed (null) we DON'T know the real session count — leave minSessions 0
            // so sessionsToGo is 0 and the Brief shows the neutral "watching and learning" line instead
            // of telling an established user "Still learning — 0 of N sessions logged" (a failed read is
            // not a fresh account). The "Last week" section is already hidden because review is null.
            minSessions = if (snapshot != null) AutoCoachPlanner.MIN_SESSIONS else 0,
            previousApplied = previousApplied
        )
    }

    /**
     * The "Coach lab" portrait (finding #5): what the coach is tracking, right now, in plain
     * words — tracked lifts, the recovery inputs it reads, and what it has learned so far. Pure
     * read off ONE snapshot + the decision history; never writes. Call on screen open only.
     */
    suspend fun coachLab(): CoachWatch {
        // Read-only display: reuse a recent snapshot (e.g. the Week Brief's) rather than re-running the
        // whole-history fan-out again when Coach Lab is opened straight from it.
        val s = adaptationRepository.snapshotCached()
        val t = AdaptThresholds()
        val bias = CoachGenBias.from(coachDao.allDecisions())
        val dayMs = 24L * 60 * 60 * 1000
        val windowStart = s.nowMs - t.deloadWindowDays * dayMs

        // Lifts the plateau ladder (System 1) currently flags as stalling — same source the weekly
        // pass uses, so the lab shows exactly what the coach is acting on.
        val stalledIds = ProgressionAdvisor.evaluate(s, t).mapNotNull { rec ->
            when (rec) {
                is Recommendation.VariationSwap -> rec.exerciseId
                is Recommendation.RepRangeShift -> rec.exerciseId
                is Recommendation.WeightChange -> rec.exerciseId
                else -> null
            }
        }.toSet()

        // Tracked lifts: anything with non-skipped history; "concrete" once it clears the same
        // weighted-bout gate the planner uses to judge progress (AutoCoachPlanner.trackedLiftIds).
        val tracked = s.exerciseHistory.mapNotNull { (slotId, bouts) ->
            val nonSkipped = bouts.count { !it.skipped }
            if (nonSkipped == 0) return@mapNotNull null
            val weighted = bouts.count { b -> !b.skipped && b.sets.any { it.weightLb != null && !it.isAssisted } }
            val name = ExerciseLibrary.byId(slotId)?.name
                ?: bouts.lastOrNull { !it.swappedName.isNullOrBlank() }?.swappedName
                ?: slotId
            TrackedLift(
                name = name, bouts = nonSkipped,
                concrete = weighted > t.plateauMinBouts, stalling = slotId in stalledIds
            )
        }.sortedWith(
            compareByDescending<TrackedLift> { it.stalling }
                .thenByDescending { it.concrete }.thenByDescending { it.bouts }
        )

        val moodCount = s.moods.count { it.recordedAt >= windowStart }
        val restFlags = s.cardio.count { it.date >= windowStart && (it.restReason == "sore" || it.restReason == "sick") }
        val sleepNights = s.health.sleepNights.count { it.endedAtMs >= windowStart }
        val hrReadings = s.health.restingHr.count { it.timeMs >= windowStart }
        val recovery = listOf(
            RecoverySignal("Effort check-ins",
                if (moodCount > 0) "$moodCount in the last 2 weeks" else "none yet — rate how a session felt", moodCount > 0),
            RecoverySignal("Rest-day flags",
                if (restFlags > 0) "$restFlags sore/sick day(s) noted" else "none flagged", restFlags > 0),
            RecoverySignal("Sleep",
                if (sleepNights > 0) "$sleepNights night(s) via Health Connect" else "not connected", sleepNights > 0),
            RecoverySignal("Resting heart rate",
                if (hrReadings > 0) "$hrReadings reading(s) via Health Connect" else "not connected", hrReadings > 0)
        )

        val learned = buildList {
            bias.volumeBias.forEach { (muscle, delta) ->
                add(LearnedBias("${muscle.displayName} volume", "${if (delta > 0) "+" else ""}$delta set(s) carried forward"))
            }
            if (bias.repBias.isNotEmpty()) add(LearnedBias("Rep ranges", "${bias.repBias.size} lift(s) re-ranged"))
            if (bias.prefer.isNotEmpty()) add(LearnedBias("Preferred rotations", "${bias.prefer.size} swap(s) kept"))
            if (bias.avoid.isNotEmpty()) add(LearnedBias("Avoided rotations", "${bias.avoid.size} swap(s) steered around"))
        }

        val fatigue = DeloadAdvisor.fatigue(s, t)
        return CoachWatch(
            sessionsLogged = s.sessions.size,
            minSessions = AutoCoachPlanner.MIN_SESSIONS,
            sessionsToGo = (AutoCoachPlanner.MIN_SESSIONS - s.sessions.size).coerceAtLeast(0),
            autopilot = settings.coachMode.first() == "auto",
            fatigueScore = fatigue?.score,
            fatigueThreshold = t.deloadScoreThreshold,
            trackedLifts = tracked,
            recoverySignals = recovery,
            learnedBiases = learned
        )
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
