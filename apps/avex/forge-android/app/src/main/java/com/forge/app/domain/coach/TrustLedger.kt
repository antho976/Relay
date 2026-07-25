package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CoachDecision

/** One adjustment type's earned-trust state (hardening decisions 2 + 3). */
data class TypeTrust(
    val type: String,
    /** Consecutive accepted (applied) proposals since the last skip/revert/failed outcome. */
    val streak: Int,
    /** Accepted streak required before this type may auto-apply. */
    val required: Int,
    val earned: Boolean
) {
    val label: String
        get() = when (type) {
            "swap" -> "Exercise rotations"
            "rep_shift" -> "Rep-range shifts"
            "volume_up" -> "Added sets"
            "volume_down" -> "Removed sets"
            else -> type
        }
}

/**
 * Hardening decisions 2 + 3 — auto-apply is EARNED per adjustment type, never toggled on:
 * a type qualifies after enough consecutive accepted proposals, and any bad outcome (failed
 * watcher verdict or a user revert) demotes it back to propose-only until the streak is
 * rebuilt from scratch. Tiered by blast radius: conservative types (removing a set, shifting
 * reps) earn at [CONSERVATIVE_STREAK]; aggressive ones (swaps, added sets) at
 * [AGGRESSIVE_STREAK]. Deload and revert NEVER auto-apply — a whole-program regeneration and
 * an undo both deserve a human tap, always.
 *
 * Pure function over the decision history; Phase 4's autopilot reads [earnedTypes] at pass
 * time. Undecided rows (proposed / Phase-1 shadow) don't count either way.
 */
object TrustLedger {

    const val CONSERVATIVE_STREAK = 3
    const val AGGRESSIVE_STREAK = 4

    private val REQUIRED = mapOf(
        "volume_down" to CONSERVATIVE_STREAK,
        "rep_shift" to CONSERVATIVE_STREAK,
        "swap" to AGGRESSIVE_STREAK,
        "volume_up" to AGGRESSIVE_STREAK
    )

    fun assess(decisions: List<CoachDecision>): List<TypeTrust> = REQUIRED.map { (type, required) ->
        // Decided rows only, oldest → newest by insertion order. A "folded" change (one a regenerate
        // baked into the baseline) only PARTICIPATES once the watcher has actually judged it: folding
        // ends the watcher's jurisdiction, so a folded+pending change is "applied but never validated"
        // and must stay neutral here, like an undecided proposal — otherwise a refresh or auto-rotation
        // that folds a change before its 14-day outcome window could promote autopilot on unvalidated
        // history (auto-coach seam audit 2026-06-15, finding P1). A folded change validated "ok" counts;
        // one the watcher (which still sees in-window folds — see CoachDao.pendingOutcome) rules
        // "failed" breaks the streak.
        val decided = decisions
            .filter { it.type == type }
            // NOT FOLLOWED is neutral (B1): the athlete was away or unwell, so the window says
            // nothing about the advice. It must neither extend a streak nor break one, so it is
            // invisible here rather than counted either way.
            .filter { it.outcome != CoachDecision.OUTCOME_NOT_FOLLOWED }
            .filter {
                when (it.status) {
                    "applied", "skipped", "reverted" -> true
                    "folded" -> it.outcome != "pending" // only a watcher-judged fold participates
                    else -> false
                }
            }
            .sortedBy { it.id }
        // Walk backwards: streak = consecutive accepted, broken by a skip, a revert, an applied change
        // judged failed, or a folded change that proved failed. An applied change still inside its
        // window (outcome pending) counts — it stays under the watcher and demotes later if it fails;
        // a folded change must be proven "ok", since the watcher can no longer revisit it after folding.
        var streak = 0
        for (d in decided.reversed()) {
            val accepted = when (d.status) {
                "applied" -> d.outcome != "failed"
                "folded" -> d.outcome == "ok"
                else -> false // skipped / reverted
            }
            if (accepted) streak++ else break
        }
        TypeTrust(type = type, streak = streak, required = required, earned = streak >= required)
    }.sortedBy { it.type }

    /** Types currently allowed to auto-apply (autopilot consults this at pass time). */
    fun earnedTypes(decisions: List<CoachDecision>): Set<String> =
        assess(decisions).filter { it.earned }.map { it.type }.toSet()
}
