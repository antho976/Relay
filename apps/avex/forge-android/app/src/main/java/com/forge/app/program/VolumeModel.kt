package com.forge.app.program

import kotlin.math.ceil
import kotlin.math.roundToInt

/**
 * Turns a split's *structure* into concrete per-slot set counts (program-unlock Phase 4 — generator
 * intelligence). Sets are scheme-based (compounds get 3, pump accessories 2) so per-session volume
 * stays sane; **weekly volume scales naturally with frequency** because more training days = more
 * slots for a muscle. A per-muscle **weekly cap** trims junk volume on high-frequency splits, and
 * `emphasis` finally does something — focused muscles get an extra set per slot (it was a no-op before).
 */
object VolumeModel {

    const val MIN_SETS = 2
    const val MAX_SETS = 5
    private const val EMPHASIS_BONUS_SETS = 1
    /** Cap on how far an emphasised muscle's weekly total may exceed its [weeklyCap] (junk-volume guard). */
    private const val MAX_EMPHASIS_HEADROOM = 2

    private fun baseSets(scheme: RepScheme): Int = when (scheme) {
        RepScheme.STRENGTH -> 3      // the day's heavy compound
        RepScheme.HYPERTROPHY -> 3
        RepScheme.PUMP -> 2          // accessory / isolation
    }

    /**
     * Extra sets the heavy compound carries for a strength goal (2026-09-21): 4-6-rep work needs
     * more sets to reach a useful dose, and it is the one slot a strength trainee is there for.
     * Only at full volume — a beginner's ramp and a deload keep the plain base.
     */
    private const val STRENGTH_GOAL_BONUS_SETS = 1

    private fun slotSets(slot: MuscleSlot, focus: Set<MuscleGroup>, volumeFactor: Double, goal: String, minSets: Int): Int {
        val withEmphasis = baseSets(slot.scheme) + if (slot.muscle in focus) EMPHASIS_BONUS_SETS else 0
        val scaled = withEmphasis * volumeFactor
        // The heavy compound rounds UP so a beginner's 0.8 ramp keeps 3 sets of the main lift while
        // the accessories drop to 2 — a compound emphasis rather than a flat 2-2-2 day.
        val rounded = if (slot.scheme == RepScheme.STRENGTH) ceil(scaled).toInt() else scaled.roundToInt()
        val strengthBonus = if (slot.scheme == RepScheme.STRENGTH && goal == "get_stronger" && volumeFactor >= 1.0)
            STRENGTH_GOAL_BONUS_SETS else 0
        return (rounded + strengthBonus).coerceIn(minSets, MAX_SETS)
    }

    /** Conservative direct-set ceilings, not biological limits. Arms/delts/glutes also receive
     * substantial work from presses, pulls and leg compounds, so their direct budgets are lower. */
    val weeklyCap: Map<MuscleGroup, Int> = mapOf(
        MuscleGroup.CHEST to 18,
        MuscleGroup.BACK to 20,
        MuscleGroup.SHOULDERS to 10,
        MuscleGroup.REAR_DELTS to 8,
        MuscleGroup.BICEPS to 8,
        MuscleGroup.TRICEPS to 8,
        MuscleGroup.QUADS to 18,
        MuscleGroup.HAMSTRINGS to 16,
        MuscleGroup.GLUTES to 10,
        MuscleGroup.CALVES to 14,
        MuscleGroup.CORE to 12
    )

    /** Muscles whose volume the `emphasis` setting boosts. Empty/"balanced" → no boost. */
    fun emphasisFocus(emphasis: String): Set<MuscleGroup> = when (emphasis) {
        "arms-shoulders", "arms_shoulders" ->
            setOf(MuscleGroup.BICEPS, MuscleGroup.TRICEPS, MuscleGroup.SHOULDERS, MuscleGroup.REAR_DELTS)
        "upper" ->
            setOf(MuscleGroup.CHEST, MuscleGroup.BACK, MuscleGroup.SHOULDERS, MuscleGroup.BICEPS, MuscleGroup.TRICEPS)
        "legs" ->
            setOf(MuscleGroup.QUADS, MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES, MuscleGroup.CALVES)
        else -> emptySet()
    }

    /**
     * Allocate sets to every slot: scheme base (+1 for [focus] muscles, ×[volumeFactor] for experience,
     * ±[bias] learned by the coach), then trim any muscle whose weekly total exceeds [weeklyCap].
     * Returns sets per day per slot.
     *
     * [minSets] is the per-slot floor — normally [MIN_SETS], but a deload week passes a lower floor so
     * already-light slots (e.g. a beginner's 2-set accessories) can actually drop instead of flooring
     * at the same value they'd have in a normal week.
     *
     * [bias] is the coach's net applied volume adjustment per muscle (CoachGenBias): each +1 lands on
     * the muscle's currently-smallest slot, each −1 comes off its largest — spread, not stacked —
     * applied BEFORE the weekly cap so the junk-volume guard still has the last word.
     *
     * [goal] is the onboarding goal: `get_stronger` gives the heavy compound an extra set at full
     * volume (see [slotSets]); every other goal uses the scheme bases as they are.
     */
    fun allocate(
        days: List<DayArchetype>,
        focus: Set<MuscleGroup> = emptySet(),
        volumeFactor: Double = 1.0,
        minSets: Int = MIN_SETS,
        bias: Map<MuscleGroup, Int> = emptyMap(),
        goal: String = "build_muscle",
        /**
         * Per-muscle weekly ceilings measured from THIS athlete (Coach v3 D's `PersonalProfile`),
         * overriding the population defaults in [weeklyCap] where they've been earned. Empty until
         * the profile has months of history, at which point generation stops using a number that
         * was never about this person.
         */
        personalCaps: Map<MuscleGroup, Int> = emptyMap()
    ): List<List<Int>> {
        val result: List<IntArray> = days.map { day ->
            IntArray(day.targets.size) { si -> slotSets(day.targets[si], focus, volumeFactor, goal, minSets) }
        }
        val positions = HashMap<MuscleGroup, MutableList<Pair<Int, Int>>>()
        days.forEachIndexed { di, day ->
            day.targets.forEachIndexed { si, slot -> positions.getOrPut(slot.muscle) { mutableListOf() }.add(di to si) }
        }
        // Coach-learned volume bias: fold the net applied ±sets into the baseline.
        bias.forEach { (muscle, delta) ->
            val slots = positions[muscle] ?: return@forEach
            repeat(kotlin.math.abs(delta)) {
                if (delta > 0) {
                    val smallest = slots.filter { (di, si) -> result[di][si] < MAX_SETS }
                        .minByOrNull { (di, si) -> result[di][si] } ?: return@repeat
                    result[smallest.first][smallest.second] += 1
                } else {
                    val biggest = slots.filter { (di, si) -> result[di][si] > minSets }
                        .maxByOrNull { (di, si) -> result[di][si] } ?: return@repeat
                    result[biggest.first][biggest.second] -= 1
                }
            }
        }
        // Per-muscle weekly cap: shave sets until under the ceiling — accessories first (PUMP, then
        // HYPERTROPHY, largest slot first within a scheme), the heavy STRENGTH compound last. Shaving
        // "the largest slot" alone hit the compound first on every tie, so an advanced leg day ran
        // 3 sets of squats next to 4 of leg extensions (2026-09-21).
        positions.forEach { (muscle, slots) ->
            // Focused muscles get a little headroom above the cap so the emphasis isn't immediately
            // trimmed away — but bounded (not slots.size × bonus, which on a high-frequency split let a
            // prioritised muscle blow well past the junk-volume ceiling).
            val cap = (personalCaps[muscle] ?: weeklyCap[muscle] ?: return@forEach) +
                if (muscle in focus && muscle !in personalCaps) minOf(slots.size * EMPHASIS_BONUS_SETS, MAX_EMPHASIS_HEADROOM) else 0
            val personal = muscle in personalCaps
            val floor = if (personal) 0 else minSets
            val effectiveCap = if (personal) cap.coerceAtLeast(0) else maxOf(cap, slots.size * minSets)
            var total = slots.sumOf { (di, si) -> result[di][si] }
            while (total > effectiveCap) {
                val victim = slots.filter { (di, si) -> result[di][si] > floor }
                    .maxWithOrNull(trimOrder(days) { (di, si) -> result[di][si] }) ?: break
                result[victim.first][victim.second] -= 1
                total -= 1
            }
        }
        // A long exercise list or several priority muscles must not silently create a marathon.
        // Trim extra sets, keeping each movement's minimum and the weekly ceilings above intact.
        result.forEachIndexed { di, sets ->
            val sessionCap = maxOf(sets.size * minSets, minOf(24, (24 * volumeFactor).roundToInt()))
            while (sets.sum() > sessionCap) {
                val index = sets.indices.filter { sets[it] > minSets }
                    .maxWithOrNull(compareBy({ trimRank(days[di].targets[it].scheme) }, { sets[it] })) ?: break
                sets[index]--
            }
        }
        return result.map { it.toList() }
    }

    /** Lowest-priority scheme first (PUMP > HYPERTROPHY > STRENGTH), then the largest slot. */
    private fun trimOrder(days: List<DayArchetype>, setsAt: (Pair<Int, Int>) -> Int): Comparator<Pair<Int, Int>> =
        compareBy<Pair<Int, Int>>({ (di, si) -> trimRank(days[di].targets[si].scheme) }, { setsAt(it) })

    private fun trimRank(scheme: RepScheme): Int = when (scheme) {
        RepScheme.PUMP -> 2
        RepScheme.HYPERTROPHY -> 1
        RepScheme.STRENGTH -> 0
    }
}
