package com.forge.app.domain.coach

import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.ProgressionAdvisor
import com.forge.app.domain.adapt.Recommendation
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.MuscleGroup

/**
 * Outcome of one Weekly Coach Pass.
 *
 * SHADOW/PROPOSED: the planner has adjustments — Phase 1 displayed them; Phase 3 proposes
 * them for one-tap apply. HOLD: the correct call is "no change", with the reason spelled out
 * (an explained hold is how a quiet coach proves it isn't a broken one — hardening 4).
 * ERROR is never produced here — the repository stamps crashes so a failure can't impersonate
 * a deliberate hold (hardening 13).
 */
enum class CoachPassStatus { SHADOW, HOLD, ERROR }

/** One adjustment the coach wants to make, with the reason and the data to apply/undo it. */
data class ShadowDecision(
    /** "deload" | "swap" | "rep_shift" | "volume_up" | "volume_down" | "revert". */
    val type: String,
    /** Exercise library id, or "week" for week-level calls. */
    val targetKey: String,
    val targetName: String,
    /** Short imperative line for the Brief row ("Rotate DB Row → Wide Lat Pulldown"). */
    val summary: String,
    val reason: String,
    /** Day the target slot lives on — rep/sets overrides are per (day, exercise). */
    val dayKey: String = "",
    /** Type-specific apply argument: swap = replacement lib id; volume = new set count; rep_shift = new range; revert = original decision id. */
    val payload: String? = null
)

data class CoachPassResult(
    val status: CoachPassStatus,
    /** Set when status == HOLD — why holding is the right call this week. */
    val holdReason: String?,
    val decisions: List<ShadowDecision>
)

/**
 * Everything the repository assembles for one pass beyond the snapshot (auto-coach Phase 3).
 * All of it is derived from the coach's own records + user customizations — pure inputs.
 */
data class CoachPassInputs(
    val experience: String,
    /** Last-week session target (days/week) — volume additions require adherence. */
    val sessionsTarget: Int = 0,
    /** Exercise ids carrying ANY user customization — coach-locked, untouchable (hardening 9). */
    val lockedExerciseIds: Set<String> = emptySet(),
    /** Muscles with an applied volume change still inside its outcome window (hardening 11). */
    val volumeLockedMuscles: Set<MuscleGroup> = emptySet(),
    /** Net applied volume delta per muscle over the drift window — the ±2 cap (hardening 11). */
    val volumeNetByMuscle: Map<MuscleGroup, Int> = emptyMap(),
    /** Failed applied changes the watcher flagged — surfaced first as revert proposals. */
    val revertProposals: List<ShadowDecision> = emptyList(),
    /**
     * Structural proposals the user most recently declined — keys "type:targetKey:payload" for
     * swap/rep_shift whose latest decision was skipped or reverted. The planner won't re-propose the
     * identical change, so a rejected swap stops reappearing every week (seam fix, finding 19).
     */
    val declinedStructural: Set<String> = emptySet()
)

/**
 * The Weekly Coach Pass planner — pure function of the snapshot + pass inputs, like every
 * advisor. Composes the engine reads (deload score, plateau ladder) plus the Phase 3 volume
 * rules into the weekly call a coach would make. It never writes; the repository applies
 * decisions through existing user write paths only (hardening 8).
 *
 * Priorities: deload supersedes everything → reverts (fix mistakes before new changes) →
 * stall-triggered structural changes (swap > rep shift) → volume ±1. Beginners get at most
 * ONE change a week, others two (hardening 4). Weight nudges are no longer emitted here —
 * the in-session chip and the Overview coach feed already deliver them live.
 */
object AutoCoachPlanner {

    /** Below this many finished sessions there is no baseline to coach against. */
    private const val MIN_SESSIONS = 4
    /** A training gap at least this long means "ease back in", not "restructure". */
    private const val GAP_HOLD_DAYS = 14
    /** Per-muscle net applied-volume drift cap, in sets (hardening 11). */
    private const val VOLUME_DRIFT_CAP = 2
    /** Never push a slot past this many sets (mirrors VolumeModel.MAX_SETS). */
    private const val MAX_SLOT_SETS = 5
    /** An exercise skipped this often in its last 4 bouts is oversized — drop a set. */
    private const val SKIP_DOWNSIZE_COUNT = 3
    private const val DAY_MS = 24L * 60 * 60 * 1000

    fun evaluate(
        s: AdaptationSnapshot,
        inputs: CoachPassInputs,
        t: AdaptThresholds = AdaptThresholds()
    ): CoachPassResult {
        if (s.sessions.size < MIN_SESSIONS) return hold(
            "Still learning your baseline — ${s.sessions.size} session(s) logged. " +
                "Keep training; the coach starts calling adjustments at $MIN_SESSIONS."
        )
        val lastSessionAt = s.sessions.maxOf { it.startedAt }
        if (s.nowMs - lastSessionAt >= GAP_HOLD_DAYS * DAY_MS) return hold(
            "You're just getting back after a break — run the plan as-is this week and ease in. " +
                "No changes until there's fresh data."
        )

        // A deload call supersedes everything else (mirrors the arbiter's suppression rule):
        // restructuring a plan the same week you're told to recover is noise. Pending reverts
        // are deliberately not carried into a deload week — the regeneration resets the slate.
        DeloadAdvisor.evaluate(s, t)?.let { deload ->
            return CoachPassResult(
                CoachPassStatus.SHADOW, null,
                listOf(
                    ShadowDecision(
                        type = "deload", targetKey = "week", targetName = "Whole week",
                        summary = "Run a deload week — lighter volume across the board",
                        reason = deload.reason
                    )
                )
            )
        }

        val ladder = ProgressionAdvisor.evaluate(s, t)
        val stalledIds = ladder.mapNotNull {
            when (it) {
                is Recommendation.VariationSwap -> it.exerciseId
                is Recommendation.RepRangeShift -> it.exerciseId
                is Recommendation.WeightChange -> it.exerciseId
                else -> null
            }
        }.toSet()

        // Stall-triggered structural candidates, most decisive first. Coach-locked slots are
        // skipped — a slot the user edited is theirs (hardening 9).
        val slotByExercise = s.program.flatMap { day -> day.slots.map { day.dayKey to it } }
            .associateBy { it.second.exerciseId }
        val structural = ladder.mapNotNull { rec ->
            when (rec) {
                is Recommendation.VariationSwap -> {
                    if (rec.exerciseId in inputs.lockedExerciseIds) return@mapNotNull null
                    val replacementId = rec.candidateIds.firstOrNull() ?: return@mapNotNull null
                    // Don't re-propose a swap the user already declined (finding 19).
                    if ("swap:${rec.exerciseId}:$replacementId" in inputs.declinedStructural) return@mapNotNull null
                    val replacementName = ExerciseLibrary.byId(replacementId)?.name ?: replacementId
                    val dayKey = slotByExercise[rec.exerciseId]?.first ?: ""
                    0 to ShadowDecision(
                        "swap", rec.exerciseId, rec.exerciseName,
                        "Rotate ${rec.exerciseName} → $replacementName", rec.reason,
                        dayKey = dayKey, payload = replacementId
                    )
                }
                is Recommendation.RepRangeShift -> {
                    if (rec.exerciseId in inputs.lockedExerciseIds) return@mapNotNull null
                    if ("rep_shift:${rec.exerciseId}:${rec.toReps}" in inputs.declinedStructural) return@mapNotNull null
                    val dayKey = slotByExercise[rec.exerciseId]?.first ?: ""
                    1 to ShadowDecision(
                        "rep_shift", rec.exerciseId, rec.exerciseName,
                        "Shift ${rec.exerciseName} from ${rec.fromReps} to ${rec.toReps} reps", rec.reason,
                        dayKey = dayKey, payload = rec.toReps
                    )
                }
                else -> null
            }
        }.sortedBy { it.first }.map { it.second }

        val volume = volumeDecisions(s, inputs, stalledIds, t)

        val candidates = inputs.revertProposals + structural + volume
        val tracked = trackedLiftCount(s, t)
        if (candidates.isEmpty()) {
            return if (tracked == 0) hold(
                "Not enough lift history to judge progress yet — keep logging and the weekly calls will start."
            ) else hold(
                "All $tracked tracked lift(s) are progressing or too fresh to judge — " +
                    "the plan is working. That's what a hold looks like."
            )
        }

        val cap = if (inputs.experience == "beginner") 1 else 2
        return CoachPassResult(CoachPassStatus.SHADOW, null, candidates.take(cap))
    }

    /**
     * Volume ±1 (hardening 11): +1 set to ONE muscle whose tracked lifts are all progressing,
     * only when fatigue is quiet, last week's sessions hit the target, the muscle isn't inside
     * a pending outcome window, and its net coach-driven drift stays within ±2 sets. −1 set
     * from an exercise the user keeps skipping (the plan is bigger than their week).
     */
    private fun volumeDecisions(
        s: AdaptationSnapshot,
        inputs: CoachPassInputs,
        stalledIds: Set<String>,
        t: AdaptThresholds
    ): List<ShadowDecision> {
        val out = mutableListOf<ShadowDecision>()
        val slots = s.program.flatMap { day -> day.slots.map { day.dayKey to it } }
        val trackedIds = trackedLiftIds(s, t)

        // ── −1: chronically skipped exercise ──────────────────────────────────
        val skipped = slots.mapNotNull { (dayKey, slot) ->
            if (slot.exerciseId in inputs.lockedExerciseIds || slot.targetSets <= 2) return@mapNotNull null
            if ((inputs.volumeNetByMuscle[slot.muscle] ?: 0) <= -VOLUME_DRIFT_CAP) return@mapNotNull null
            if (slot.muscle in inputs.volumeLockedMuscles) return@mapNotNull null
            val last4 = s.exerciseHistory[slot.exerciseId]?.takeLast(4) ?: return@mapNotNull null
            if (last4.size < 4) return@mapNotNull null
            val skips = last4.count { it.skipped }
            if (skips >= SKIP_DOWNSIZE_COUNT) Triple(dayKey, slot, skips) else null
        }.maxByOrNull { it.third }
        skipped?.let { (dayKey, slot, skips) ->
            out += ShadowDecision(
                "volume_down", slot.exerciseId, slot.name,
                "Drop a set from ${slot.name} (${slot.targetSets} → ${slot.targetSets - 1})",
                "Skipped $skips of the last 4 times — a plan you'll actually finish beats a bigger one you won't.",
                dayKey = dayKey, payload = (slot.targetSets - 1).toString()
            )
        }

        // ── +1: one muscle that's earning more ────────────────────────────────
        val fatigue = DeloadAdvisor.fatigue(s, t)
        val fresh = fatigue == null || fatigue.score < t.deloadScoreThreshold - 2
        val weekSessions = s.sessions.count { it.startedAt >= s.nowMs - 7 * DAY_MS }
        if (fresh && inputs.sessionsTarget > 0 && weekSessions >= inputs.sessionsTarget) {
            val byMuscle = slots.groupBy { it.second.muscle }
            val candidate = byMuscle.entries.mapNotNull { (muscle, muscleSlots) ->
                if (muscle in inputs.volumeLockedMuscles) return@mapNotNull null
                if ((inputs.volumeNetByMuscle[muscle] ?: 0) >= VOLUME_DRIFT_CAP) return@mapNotNull null
                val tracked = muscleSlots.map { it.second.exerciseId }.filter { it in trackedIds }
                if (tracked.isEmpty() || tracked.any { it in stalledIds }) return@mapNotNull null
                val slot = muscleSlots
                    .filter { it.second.exerciseId !in inputs.lockedExerciseIds && it.second.targetSets < MAX_SLOT_SETS }
                    .minByOrNull { it.second.targetSets } ?: return@mapNotNull null
                Triple(muscle, slot, tracked.size)
            }.maxWithOrNull(compareBy({ it.third }, { -it.first.ordinal }))
            candidate?.let { (muscle, daySlot, trackedCount) ->
                val (dayKey, slot) = daySlot
                out += ShadowDecision(
                    "volume_up", slot.exerciseId, slot.name,
                    "Add a set to ${slot.name} (${slot.targetSets} → ${slot.targetSets + 1})",
                    "${muscle.displayName} is responding — $trackedCount tracked lift(s) all progressing, " +
                        "recovery is fresh, and last week hit the session target.",
                    dayKey = dayKey, payload = (slot.targetSets + 1).toString()
                )
            }
        }
        return out
    }

    /** Lifts with enough non-skipped, weighted history to be judged — the ladder's own gate. */
    private fun trackedLiftIds(s: AdaptationSnapshot, t: AdaptThresholds): Set<String> =
        s.exerciseHistory.filter { (_, bouts) ->
            bouts.count { b -> !b.skipped && b.sets.any { it.weightLb != null && !it.isAssisted } } > t.plateauMinBouts
        }.keys

    private fun trackedLiftCount(s: AdaptationSnapshot, t: AdaptThresholds): Int = trackedLiftIds(s, t).size

    private fun hold(reason: String) = CoachPassResult(CoachPassStatus.HOLD, reason, emptyList())
}
