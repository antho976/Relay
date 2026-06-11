package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.domain.adapt.AdaptThresholds
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.DeloadAdvisor

/** The watcher's verdict on one applied change. */
data class WatchVerdict(
    val decisionId: Long,
    /** "ok" | "failed" — pending decisions simply get no verdict this pass. */
    val outcome: String,
    /** Why it failed — becomes the revert proposal's reason. */
    val failReason: String? = null
)

/**
 * Hardening decision 5 — once the coach acts, it must notice when a change was wrong. Pure
 * function over the applied decisions still awaiting a verdict, run at the start of each
 * weekly pass.
 *
 * Per type, inside the [WINDOW_DAYS] evaluation window:
 *  - swap / rep_shift: the changed exercise getting skipped (≥2 of its bouts since apply) is
 *    avoidance — failed. Quietly trained until the window closes — ok.
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
        t: AdaptThresholds = AdaptThresholds()
    ): List<WatchVerdict> = applied.mapNotNull { d ->
        val appliedAt = d.appliedAt ?: return@mapNotNull null
        val windowClosed = s.nowMs - appliedAt >= WINDOW_DAYS * DAY_MS
        when (d.type) {
            "swap", "rep_shift" -> {
                val boutsSince = s.exerciseHistory[d.targetKey].orEmpty()
                    .filter { it.sessionStartedAt >= appliedAt }
                val skips = boutsSince.count { it.skipped }
                when {
                    skips >= SKIP_FAIL_COUNT -> WatchVerdict(
                        d.id, "failed",
                        "${d.targetName} has been skipped $skips times since the change — it isn't landing"
                    )
                    windowClosed -> WatchVerdict(d.id, "ok")
                    else -> null
                }
            }
            "volume_up" -> {
                val fatigue = DeloadAdvisor.fatigue(s, t)
                when {
                    fatigue != null && fatigue.score >= t.deloadScoreThreshold -> WatchVerdict(
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

    /** Failed verdicts → revert proposals for the next Brief (applied via per-change undo). */
    fun revertProposals(
        applied: List<CoachDecision>,
        verdicts: List<WatchVerdict>
    ): List<ShadowDecision> {
        val failedById = verdicts.filter { it.outcome == "failed" }.associateBy { it.decisionId }
        return applied.mapNotNull { d ->
            val verdict = failedById[d.id] ?: return@mapNotNull null
            // A change with no undo data (deload) can't be reverted mechanically — skip.
            if (d.undoData == null && d.type != "swap") return@mapNotNull null
            ShadowDecision(
                type = "revert", targetKey = d.targetKey, targetName = d.targetName,
                summary = "Revert: ${d.summary}",
                reason = verdict.failReason ?: "the change didn't land",
                dayKey = d.dayKey, payload = d.id.toString()
            )
        }
    }
}
