package com.forge.app.domain.trophy

import com.forge.app.program.Trophies
import com.forge.app.program.Trophy
import com.forge.app.program.UnlockRule

/**
 * Stateless evaluator. Given a [TrophyStatsSnapshot], decides which trophies the user
 * has earned. Output is the set of trophy ids whose [UnlockRule] is satisfied — the
 * caller diffs this against already-unlocked ids to find what's *newly* unlocked.
 *
 * The rules are simple "at-least" thresholds, but encoding them as a sealed hierarchy
 * (in [com.forge.app.program.Trophies]) keeps them as data: easy to log, test, and
 * extend without touching this when statistics or rule families change.
 */
object TrophyEvaluator {

    /** Trophy ids currently satisfied by [snapshot]. Iterates the static catalogue once. */
    fun unlockedByRule(snapshot: TrophyStatsSnapshot, catalogue: List<Trophy> = Trophies.all): Set<String> =
        catalogue.asSequence()
            .filter { isUnlocked(it.unlock, snapshot) }
            .map { it.id }
            .toSet()

    /** Tests a single rule against the snapshot. Public for unit tests. */
    fun isUnlocked(rule: UnlockRule, s: TrophyStatsSnapshot): Boolean = when (rule) {
        is UnlockRule.TotalSessionsAtLeast -> s.totalLoggedExercises >= rule.n
        is UnlockRule.TotalPRsAtLeast -> s.totalPrs >= rule.n
        is UnlockRule.BrutalCountAtLeast -> s.brutalRatings >= rule.n
        is UnlockRule.SwapCountAtLeast -> s.swapsUsed >= rule.n
        is UnlockRule.FullTargetHitsAtLeast -> s.fullTargetHits >= rule.n
        is UnlockRule.WorkoutsCompletedAtLeast -> s.finishedSessions >= rule.n
        is UnlockRule.DistinctDaysTrainedAtLeast -> s.distinctDayKeysTrained >= rule.n
        is UnlockRule.MaxBenchAtLeast -> s.maxBenchLb >= rule.lb
        is UnlockRule.MaxSquatAtLeast -> s.maxSquatLb >= rule.lb
        is UnlockRule.MaxSessionVolumeAtLeast -> s.maxSessionVolumeLb >= rule.lb
    }

    /**
     * Renders a "X / Y" progress hint for a still-locked trophy. Returns `null` when
     * the rule type doesn't have a clean numeric reading (none currently — every rule
     * is a threshold — but the door is open for future categorical rules).
     */
    fun progressHint(rule: UnlockRule, s: TrophyStatsSnapshot): String? = when (rule) {
        is UnlockRule.TotalSessionsAtLeast -> "${s.totalLoggedExercises} / ${rule.n}"
        is UnlockRule.TotalPRsAtLeast -> "${s.totalPrs} / ${rule.n}"
        is UnlockRule.BrutalCountAtLeast -> "${s.brutalRatings} / ${rule.n}"
        is UnlockRule.SwapCountAtLeast -> "${s.swapsUsed} / ${rule.n}"
        is UnlockRule.FullTargetHitsAtLeast -> "${s.fullTargetHits} / ${rule.n}"
        is UnlockRule.WorkoutsCompletedAtLeast -> "${s.finishedSessions} / ${rule.n}"
        is UnlockRule.DistinctDaysTrainedAtLeast -> "${s.distinctDayKeysTrained} / ${rule.n} days"
        is UnlockRule.MaxBenchAtLeast -> "${s.maxBenchLb.toInt()} / ${rule.lb.toInt()} lb"
        is UnlockRule.MaxSquatAtLeast -> "${s.maxSquatLb.toInt()} / ${rule.lb.toInt()} lb"
        is UnlockRule.MaxSessionVolumeAtLeast -> "${s.maxSessionVolumeLb.toInt()} / ${rule.lb.toInt()} lb"
    }
}
