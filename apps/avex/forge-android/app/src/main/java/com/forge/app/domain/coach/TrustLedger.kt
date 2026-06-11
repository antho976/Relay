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
        // Decided rows only, oldest → newest by insertion order.
        val decided = decisions
            .filter { it.type == type && it.status in setOf("applied", "skipped", "reverted") }
            .sortedBy { it.id }
        // Walk backwards: streak = consecutive accepted, broken by a skip, a revert, or an
        // applied change the watcher judged failed (demotion resets the count entirely).
        var streak = 0
        for (d in decided.reversed()) {
            if (d.status == "applied" && d.outcome != "failed") streak++ else break
        }
        TypeTrust(type = type, streak = streak, required = required, earned = streak >= required)
    }.sortedBy { it.type }

    /** Types currently allowed to auto-apply (autopilot consults this at pass time). */
    fun earnedTypes(decisions: List<CoachDecision>): Set<String> =
        assess(decisions).filter { it.earned }.map { it.type }.toSet()
}
