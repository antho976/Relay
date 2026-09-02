package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.adapt.bestE1rm

/** The watcher's verdict on one applied change. */
data class WatchVerdict(
    val decisionId: Long,
    /**
     * "ok" | "failed" | "not_followed" — pending decisions simply get no verdict this pass.
     * NOT FOLLOWED (B1) means the window was unlivable (illness, a training gap), so the change
     * is closed out without counting for or against the coach.
     */
    val outcome: String,
    /** Why it failed, or why it couldn't be judged — becomes the revert proposal's reason. */
    val failReason: String? = null
)

/**
 * Hardening decision 5 — once the coach acts, it must notice when a change was wrong. Pure
 * function over the applied decisions still awaiting a verdict, run at the start of each
 * weekly pass.
 *
 * Per type, inside the [WINDOW_DAYS] evaluation window:
 *  - swap: the changed exercise getting skipped (≥2 of its bouts since apply) is avoidance —
 *    failed. Quietly trained until the window closes — ok.
 *  - rep_shift: same skip rule, plus an evidence check at window close — a rep-range shift exists
 *    to RESTART progress, so if the lift's best e1RM since the change sits below where it was
 *    before (by [AdaptThresholds.repShiftRegressFraction]) the shift backfired — failed.
 *  - volume_up: the fatigue score crossing into deload territory after the addition — failed.
 *  - volume_down / deload / revert: conservative actions; they pass once the window closes.
 *
 * User reverts are recorded directly by the repository at undo time (status reverted +
 * outcome failed) — the watcher never sees them. A "failed" verdict does two things upstream:
 * a revert proposal in the next Brief, and a hit to that type's trust record (Phase 4's
 * promotion gate reads the outcome column).
 */
object OutcomeWatcher {

    const val WINDOW_DAYS = 14
    private const val SKIP_FAIL_COUNT = 2
    private const val DAY_MS = 24L * 60 * 60 * 1000

    fun evaluate(
        applied: List<CoachDecision>,
        s: AdaptationSnapshot,
        t: AdaptThresholds = AdaptThresholds(),
        /**
         * The athlete's life during these windows (B1). Illness or a training gap makes a window
         * unjudgeable: nothing happened, so the change neither worked nor failed. Those close as
         * NOT FOLLOWED — watched, recorded, and counted against nobody.
         */
        life: LifeEvents.State = LifeEvents.State.NONE
    ): List<WatchVerdict> {
        // Only volume_up decisions need the fatigue score — compute it at most once, lazily, instead
        // of once per decision inside the loop.
        val fatigue by lazy { DeloadAdvisor.fatigue(s, t) }
        return applied.mapNotNull { d ->
        val appliedAt = d.appliedAt ?: return@mapNotNull null
        // CALENDAR days, not 14 x 86,400,000 ms. Passes run at most once per ISO week, so a window
        // a few minutes short of closing on the day the pass runs slips a FULL EXTRA WEEK: apply on
        // a Monday evening, open the app the Monday-fortnight morning, and only 13 d 13 h have
        // elapsed — no verdict until the Monday after that, 21 days later, with the row reading
        // "still watching · ~0 days left" throughout. A spring-forward inside the window takes
        // another hour off and widens the band of apply-times that slip.
        val windowClosed = java.time.temporal.ChronoUnit.DAYS.between(
            java.time.Instant.ofEpochMilli(appliedAt).atZone(s.zoneId).toLocalDate(),
            java.time.Instant.ofEpochMilli(s.nowMs).atZone(s.zoneId).toLocalDate()
        ) >= WINDOW_DAYS
        // Adherence before efficacy: a window the athlete wasn't there for says nothing about the
        // advice. Only judged once the window closes, so a live window still gets its real verdict.
        if (windowClosed &&
            LifeEvents.suppressesVerdict(appliedAt, appliedAt + WINDOW_DAYS * DAY_MS, life)
        ) {
            return@mapNotNull WatchVerdict(
                d.id, CoachDecision.OUTCOME_NOT_FOLLOWED,
                "you were away or unwell for this window, so it isn't judged"
            )
        }
        when (d.type) {
            "swap" -> {
                val boutsSince = s.exerciseHistory[d.targetKey].orEmpty()
                    .filter { it.sessionStartedAt >= appliedAt }
                val skips = boutsSince.count { it.skipped }
                when {
                    skips >= SKIP_FAIL_COUNT -> WatchVerdict(
                        d.id, "failed",
                        "${d.targetName} has been skipped $skips times since the change — it isn't landing"
                    )
                    // An "ok" needs evidence: at least one non-skipped bout of the changed slot since
                    // the apply. An empty window used to fall through to ok, so a swap the athlete never
                    // actually performed was recorded as a success and counted toward auto-apply trust
                    // (audit M-08). No exposure is neither a win nor a loss — NOT FOLLOWED.
                    windowClosed && boutsSince.none { !it.skipped } -> WatchVerdict(
                        d.id, CoachDecision.OUTCOME_NOT_FOLLOWED,
                        "${d.targetName} wasn't trained during the watch window, so the change isn't judged"
                    )
                    windowClosed -> WatchVerdict(d.id, "ok")
                    else -> null
                }
            }
            "rep_shift" -> {
                val all = s.exerciseHistory[d.targetKey].orEmpty()
                val boutsSince = all.filter { it.sessionStartedAt >= appliedAt }
                val skips = boutsSince.count { it.skipped }
                when {
                    skips >= SKIP_FAIL_COUNT -> WatchVerdict(
                        d.id, "failed",
                        "${d.targetName} has been skipped $skips times since the change — it isn't landing"
                    )
                    windowClosed -> {
                        // A rep-range shift is meant to break a stall — judge it on strength, not just
                        // attendance. Best e1RM since the change vs. the best before it: a slip below the
                        // tolerance means the shift didn't restart progress, so it owes a revert.
                        val priorBest = all.filter { it.sessionStartedAt < appliedAt && !it.skipped }.bestE1rm()
                        val sinceBest = boutsSince.filter { !it.skipped }.bestE1rm()
                        when {
                            // No post-change e1RM means the shifted range was never actually lifted
                            // under: nothing to judge, and nothing to earn trust on (audit M-08).
                            sinceBest == null -> WatchVerdict(
                                d.id, CoachDecision.OUTCOME_NOT_FOLLOWED,
                                "${d.targetName} has no logged working sets since the rep-range change, so it isn't judged"
                            )
                            priorBest != null && sinceBest < priorBest * t.repShiftRegressFraction -> WatchVerdict(
                                d.id, "failed",
                                "${d.targetName}'s estimated 1RM slipped after the rep-range change — it didn't restart progress"
                            )
                            else -> WatchVerdict(d.id, "ok")
                        }
                    }
                    else -> null
                }
            }
            "volume_up" -> {
                val f = fatigue
                when {
                    f != null && f.score >= t.deloadScoreThreshold -> WatchVerdict(
                        d.id, "failed",
                        "fatigue climbed into deload territory after adding volume"
                    )
                    windowClosed -> WatchVerdict(d.id, "ok")
                    else -> null
                }
            }
            else -> if (windowClosed) WatchVerdict(d.id, "ok") else null
        }
        }
    }

    /** Failed verdicts → revert proposals for the next Brief (applied via per-change undo). */
    fun revertProposals(
        applied: List<CoachDecision>,
        verdicts: List<WatchVerdict>
    ): List<ShadowDecision> {
        val failedById = verdicts.filter { it.outcome == "failed" }.associateBy { it.decisionId }
        return revertProposalsFor(
            applied.filter { it.id in failedById },
            reasonFor = { failedById[it.id]?.failReason ?: "the change didn't land" }
        )
    }

    /**
     * Revert proposals re-derived from every still-applied decision the watcher has already judged
     * FAILED (its outcome column is durable). Building from the column rather than from this pass's
     * verdicts alone makes delivery self-healing: a proposal dropped by deload-supersession, the
     * per-pass cap, or an error pass simply reappears next week (seam fix, finding 4).
     */
    fun revertProposalsFor(
        failed: List<CoachDecision>,
        reasonFor: (CoachDecision) -> String = { "${it.targetName} didn't land — undo the change" }
    ): List<ShadowDecision> =
        // Newest-first: when two decisions chained on one slot both failed, Apply All must revert the
        // NEWER one first (it owns the overlay), so the per-slot LIFO guard lets the chain unwind
        // cleanly in a single pass instead of refusing the older revert (seam fix, finding 9).
        failed.sortedByDescending { it.id }.mapNotNull { d ->
            // A change with no undo data (deload) can't be reverted mechanically — skip.
            if (d.undoData == null && d.type != "swap") return@mapNotNull null
            ShadowDecision(
                type = "revert", targetKey = d.targetKey, targetName = d.targetName,
                summary = "Revert: ${d.summary}",
                reason = reasonFor(d),
                dayKey = d.dayKey, payload = d.id.toString()
            )
        }
}
