package com.forge.app.domain.coach

import com.forge.app.data.db.entities.CoachDecision
import kotlin.math.roundToInt

/**
 * How much initiative the coach has earned (Coach v3 E).
 *
 * `TrustLedger` answers "may this TYPE of change auto-apply?" from per-type streaks. This answers a
 * different question: how much may the coach DO on its own — propose, apply, plan, or act first and
 * tell you after. Trust here is global, earned from outcomes, and it unlocks initiative rather than
 * just bigger edits.
 *
 * **Demotion is rate-based, with hysteresis.** V2's any-failure rule is right for a per-type streak
 * and wrong for a tier: a coach making many autonomous calls at a realistic 70–80% win rate would
 * oscillate between tiers forever, which reads as a broken coach rather than a careful one. So a
 * tier is lost on a sustained failure RATE or on user reverts, and only after enough decided calls
 * to mean something.
 *
 * **T4 is never automatic.** Reaching it unlocks a consent card, nothing more. A coach that starts
 * acting first because a threshold ticked over has taken something it was not given.
 */
object TrustLadder {

    /** Decided calls needed before any rate is meaningful. */
    const val MIN_DECIDED = 8

    /** Win rate required to hold each tier, and the (lower) rate at which it is lost. */
    const val T2_EARN_RATE = 0.65
    const val T3_EARN_RATE = 0.75
    const val T4_EARN_RATE = 0.85

    /** Hysteresis band: a tier is only lost this far BELOW the rate that earned it. */
    const val DEMOTE_MARGIN = 0.15

    /** Weeks of coached history each tier needs, so trust is time-earned as well as outcome-earned. */
    const val T2_WEEKS = 3
    const val T3_WEEKS = 8
    const val T4_WEEKS = 16

    /** Reverts in the recent window that cap the coach regardless of its win rate. */
    const val REVERT_CAP = 3

    /** How many recent coached weeks [REVERT_CAP] actually looks at. */
    const val REVERT_WINDOW_WEEKS = 8

    enum class Tier(val level: Int, val displayName: String, val whatItMeans: String) {
        /** Watching, saying nothing. */
        OBSERVE(0, "Observing", "Learning your training. It won't suggest anything yet."),

        /** Suggests; you decide everything. */
        PROPOSE(1, "Proposing", "It suggests changes and you decide every one."),

        /** Applies the change types it has earned, one at a time. */
        AUTO_APPLY(2, "Auto-applying", "Change types it has a track record on apply themselves, always undoable."),

        /** Plans ahead: blocks, projects, bigger edits, and it tells you first. */
        PROACTIVE(3, "Proactive", "It plans blocks and starts projects on its own, announcing them first."),

        /** Owns the program, acts first, informs after. Opt-in only. */
        AUTONOMOUS(4, "Autonomous", "It owns the program, acts first and tells you after. Everything still undoable.");

        companion object {
            fun of(level: Int): Tier = entries.firstOrNull { it.level == level } ?: OBSERVE
        }
    }

    /**
     * @param tier what the coach has EARNED.
     * @param effective what it may actually do, after the user's own cap and T4 consent.
     * @param winRate the rate that produced it, for the readout.
     * @param decided how many calls that rate is based on.
     * @param awaitingConsent true when T4 is earned but not yet accepted — the consent card's cue.
     */
    data class Assessment(
        val tier: Tier,
        val effective: Tier,
        val winRate: Double,
        val decided: Int,
        val weeksCoached: Int,
        val reverts: Int,
        val awaitingConsent: Boolean
    ) {
        /** Caps that scale with initiative: how many changes one weekly pass may make. */
        val changesPerWeek: Int
            get() = when (effective) {
                Tier.OBSERVE, Tier.PROPOSE -> 2
                Tier.AUTO_APPLY -> 2
                Tier.PROACTIVE -> 4
                Tier.AUTONOMOUS -> 5
            }

        /** How many sets the coach may move on one muscle in a week. */
        val volumeStep: Int
            get() = if (effective.level >= Tier.PROACTIVE.level) 2 else 1

        /** May the coach start work without being asked first? */
        val mayInitiate: Boolean get() = effective.level >= Tier.PROACTIVE.level

        /** May it act before telling you? */
        val mayActFirst: Boolean get() = effective == Tier.AUTONOMOUS
    }

    /**
     * Assess global trust.
     *
     * @param decisions the whole coach ledger.
     * @param weeksCoached distinct weeks with a pass — trust is partly just time.
     * @param userCap the tier ceiling the user set in Settings, if any.
     * @param autonomyConsented whether the user accepted T4 when it was offered.
     */
    fun assess(
        decisions: List<CoachDecision>,
        weeksCoached: Int,
        userCap: Tier? = null,
        autonomyConsented: Boolean = false
    ): Assessment {
        // NOT FOLLOWED is invisible here for the same reason it is in TrustLedger: an unlived window
        // says nothing about the advice (B1).
        val judged = decisions.filter {
            it.outcome == CoachDecision.OUTCOME_OK || it.outcome == CoachDecision.OUTCOME_FAILED
        }
        // Reverts the COACH performed on itself. A "revert" decision carries the original's id in
        // its payload, and applying it marks that original `reverted` — same status a user undo
        // writes. Counting those against the coach punished it for correcting its own mistake,
        // which is the behaviour the ladder is supposed to reward.
        val selfRevertedIds = decisions
            .filter { it.type == "revert" }
            .mapNotNull { it.payload?.toLongOrNull() }
            .toSet()
        // REVERT_CAP is documented as "reverts in the recent window", but this method is handed the
        // WHOLE ledger, so it was counting every revert ever. Three undos across a year of use
        // pinned the coach at PROPOSE permanently, with no path back however well it did afterwards.
        val recentWeeks = decisions.map { it.weekId }.distinct().sortedDescending()
            .take(REVERT_WINDOW_WEEKS).toSet()
        val reverts = decisions.count {
            it.status == "reverted" && it.weekId in recentWeeks && it.id !in selfRevertedIds
        }
        val wins = judged.count { it.outcome == CoachDecision.OUTCOME_OK }
        val decided = judged.size
        val rate = if (decided == 0) 0.0 else wins.toDouble() / decided

        val earned = when {
            decided < MIN_DECIDED || weeksCoached < T2_WEEKS -> if (decided == 0) Tier.OBSERVE else Tier.PROPOSE
            reverts >= REVERT_CAP -> Tier.PROPOSE
            rate >= T4_EARN_RATE && weeksCoached >= T4_WEEKS -> Tier.AUTONOMOUS
            rate >= T3_EARN_RATE && weeksCoached >= T3_WEEKS -> Tier.PROACTIVE
            rate >= T2_EARN_RATE -> Tier.AUTO_APPLY
            else -> Tier.PROPOSE
        }

        // T4 is offered, never taken: without consent the coach sits at T3 no matter what it earned.
        val consented = if (earned == Tier.AUTONOMOUS && !autonomyConsented) Tier.PROACTIVE else earned
        val capped = userCap?.let { cap -> if (consented.level > cap.level) cap else consented } ?: consented

        return Assessment(
            tier = earned,
            effective = capped,
            winRate = rate,
            decided = decided,
            weeksCoached = weeksCoached,
            reverts = reverts,
            awaitingConsent = earned == Tier.AUTONOMOUS && !autonomyConsented
        )
    }

    /**
     * Should a coach currently AT [current] be demoted given a fresh assessment? Separated from
     * [assess] so the hysteresis is explicit: you fall a tier only when the rate drops meaningfully
     * below what earned it, not the moment it dips.
     */
    fun shouldDemote(current: Tier, assessment: Assessment): Boolean {
        if (assessment.decided < MIN_DECIDED) return false
        if (assessment.reverts >= REVERT_CAP) return current.level > Tier.PROPOSE.level
        val holdRate = when (current) {
            Tier.AUTONOMOUS -> T4_EARN_RATE - DEMOTE_MARGIN
            Tier.PROACTIVE -> T3_EARN_RATE - DEMOTE_MARGIN
            Tier.AUTO_APPLY -> T2_EARN_RATE - DEMOTE_MARGIN
            else -> return false
        }
        return assessment.winRate < holdRate
    }

    /** The readout line for Coach Lab: what it may do, and what it's based on. */
    fun describe(a: Assessment): String {
        if (a.decided == 0) return "No judged calls yet. It's watching."
        val pct = (a.winRate * 100).roundToInt()
        return "${a.effective.whatItMeans} Based on $pct% of ${a.decided} judged calls."
    }
}
