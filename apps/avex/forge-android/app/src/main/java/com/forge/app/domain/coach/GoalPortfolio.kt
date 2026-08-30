package com.forge.app.domain.coach

import com.forge.app.core.time.mondayStartMs
import com.forge.app.data.db.entities.CoachGoal
import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.bestE1rm
import com.forge.app.domain.adapt.countsForProgression
import com.forge.app.program.MuscleGroup
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The Goal Portfolio (Coach v3 A2): what the athlete is actually working toward, as data the coach
 * can reason about — not one hidden `userGoal` string that only reshapes rep templates.
 *
 * Pure, like every advisor: a function of the goals plus an [AdaptationSnapshot]. Each active goal
 * gets a current reading, a weekly rate, an ETA and an on/off-track verdict; conflicting goals are
 * flagged with a sequencing proposal rather than silently degrading each other.
 *
 * Silence rules match the rest of the engine: below a goal's data gate the trajectory is null (the
 * reading still renders — a number the user can see beats a verdict they can't check).
 */
object GoalPortfolio {

    private const val DAY_MS = 24L * 60 * 60 * 1000
    private const val WEEK_MS = 7 * DAY_MS

    /** Weeks of history a rate needs before an ETA is offered. */
    private const val MIN_WEEKS_FOR_RATE = 3

    /** How far ahead an ETA stays meaningful; past this the honest answer is "not at this rate". */
    private const val MAX_ETA_WEEKS = 52

    /** Bodyweight goals: pounds of drift that separates a real trend from day-to-day water. */
    private const val WEIGHT_TREND_LB = 1.5

    /**
     * One goal's live state. [current] and [target] share the kind's unit; [reading] is the
     * already-translated line for the UI so no surface re-derives it.
     */
    data class GoalState(
        val goal: CoachGoal,
        val kind: CoachGoalKind,
        val current: Double?,
        val target: Double?,
        val perWeek: Double?,
        val etaWeeks: Int?,
        val onTrack: Boolean?,
        val reachedNow: Boolean,
        val reading: String
    )

    /** Two active goals that fight, with the order the coach proposes and why. */
    data class GoalConflict(
        val first: CoachGoal,
        val second: CoachGoal,
        val explanation: String,
        val proposal: String,
        val lessonId: String? = LESSON_GOALS_FIGHT
    )

    const val LESSON_GOALS_FIGHT = "coach.why_goals_fight"

    fun evaluate(goals: List<CoachGoal>, s: AdaptationSnapshot): List<GoalState> =
        goals.filter { it.isActive }
            .sortedWith(compareBy({ it.priority }, { it.createdAt }))
            .mapNotNull { goal -> CoachGoalKind.fromCode(goal.kind)?.let { state(goal, it, s) } }

    /**
     * Conflicts among the active goals. Pure pair scan — the set is tiny (a portfolio of 8 would be
     * a lot), so an O(n²) walk is the honest implementation.
     */
    /**
     * @param s the snapshot, so a bodyweight goal's DIRECTION can be read from its target against
     *   the athlete's current weight. Optional only so existing tests can omit it; without it the
     *   direction falls back to the stored phase, and an unknown direction claims no conflict.
     */
    fun conflicts(goals: List<CoachGoal>, s: AdaptationSnapshot? = null): List<GoalConflict> {
        val active = goals.filter { it.isActive }
        val out = mutableListOf<GoalConflict>()
        for (i in active.indices) {
            for (j in i + 1 until active.size) {
                conflict(active[i], active[j], s)?.let { out += it }
            }
        }
        return out
    }

    /**
     * Is this bodyweight goal a CUT? Null when the direction cannot be established.
     *
     * Both conflicts below are about an ENERGY DEFICIT: losing weight competes with adding to a max,
     * and building a muscle "needs a surplus more than it needs volume alone". Neither is true of a
     * bulk — a bulk is synergistic with both — yet the branches returned unconditionally, and the
     * `cut` variable computed one line above them was never read. So someone gaining weight to add
     * muscle was told their two goals fight, in cutting language, on the strength of a check that
     * had been written and then not wired up.
     *
     * The stored phase wins when present, because it is what the user picked. Otherwise the target
     * against the current smoothed weight answers it, with the same tolerance the goal is declared
     * met at — inside that band the goal is maintenance, not a cut.
     */
    private fun isCut(goal: CoachGoal, s: AdaptationSnapshot?): Boolean? {
        when {
            goal.note.contains(WeightPhase.CUT.code) -> return true
            goal.note.contains(WeightPhase.BULK.code) -> return false
            goal.note.contains(WeightPhase.MAINTAIN.code) -> return false
        }
        val target = goal.targetValue ?: return null
        val current = s?.bodyweight
            ?.sortedBy { it.recordedAt }
            ?.let { WeightPhase.smoothedLatest(it) }
            ?: return null
        if (abs(current - target) <= WEIGHT_TREND_LB) return false
        return target < current
    }

    // ── Per-kind readings ──────────────────────────────────────────────────────

    private fun state(goal: CoachGoal, kind: CoachGoalKind, s: AdaptationSnapshot): GoalState =
        when (kind) {
            CoachGoalKind.LIFT_1RM -> liftState(goal, kind, s)
            CoachGoalKind.MUSCLE_VOLUME -> weeklySetsState(goal, kind, s)
            CoachGoalKind.BODYWEIGHT -> bodyweightState(goal, kind, s)
            CoachGoalKind.CONSISTENCY -> consistencyState(goal, kind, s)
            CoachGoalKind.CONDITIONING -> conditioningState(goal, kind, s)
            CoachGoalKind.BALANCE -> balanceState(goal, kind, s)
        }

    private fun liftState(goal: CoachGoal, kind: CoachGoalKind, s: AdaptationSnapshot): GoalState {
        // Training bouts only: a test-day single is a measurement, not the trend (A1's rule).
        val bouts = s.exerciseHistory[goal.targetKey].orEmpty()
            .filter { it.countsForProgression && !it.skipped }
        val points = bouts.mapNotNull { b -> b.bestE1rm()?.let { b.sessionStartedAt to it } }
        val current = points.lastOrNull()?.second
        val perWeek = weeklySlope(points)
        val target = goal.targetValue
        val eta = etaWeeks(current, target, perWeek)
        return GoalState(
            goal = goal, kind = kind, current = current, target = target, perWeek = perWeek,
            etaWeeks = eta, onTrack = onTrack(current, target, perWeek),
            reachedNow = current != null && target != null && current >= target,
            reading = when {
                current == null -> "no lifts logged yet"
                target == null -> "e1RM ${current.roundToInt()} lb"
                else -> "e1RM ${current.roundToInt()} of ${target.roundToInt()} lb"
            }
        )
    }

    private fun weeklySetsState(goal: CoachGoal, kind: CoachGoalKind, s: AdaptationSnapshot): GoalState {
        val muscle = MuscleGroup.entries.firstOrNull { it.code == goal.targetKey }
        val slotsForMuscle = s.program.flatMap { it.slots }.filter { muscle == null || it.muscle == muscle }
            .map { it.exerciseId }.toSet()
        // ISO week: a weekly-sets goal has to be measured over the week the user sees on Stats,
        // not the 7 x 24 h ending at whatever moment the pass happens to run.
        val weekStart = mondayStartMs(s.nowMs, s.zoneId)
        val sets = s.exerciseHistory
            .filterKeys { it in slotsForMuscle }
            .values.flatten()
            .filter { it.countsForProgression && !it.skipped && it.sessionStartedAt >= weekStart }
            .sumOf { it.sets.count { set -> set.durationSeconds == null } }
            .toDouble()
        val target = goal.targetValue
        return GoalState(
            goal = goal, kind = kind, current = sets, target = target, perWeek = null,
            etaWeeks = null, onTrack = target?.let { sets >= it },
            reachedNow = target != null && sets >= target,
            reading = if (target == null) "${sets.roundToInt()} sets this week"
            else "${sets.roundToInt()} of ${target.roundToInt()} sets this week"
        )
    }

    private fun bodyweightState(goal: CoachGoal, kind: CoachGoalKind, s: AdaptationSnapshot): GoalState {
        val entries = s.bodyweight.sortedBy { it.recordedAt }
        val current = WeightPhase.smoothedLatest(entries)
        val points = entries.map { it.recordedAt to it.weightLb }
        val perWeek = weeklySlope(points)
        val target = goal.targetValue
        val eta = etaWeeks(current, target, perWeek)
        val reached = current != null && target != null && abs(current - target) <= WEIGHT_TREND_LB
        return GoalState(
            goal = goal, kind = kind, current = current, target = target, perWeek = perWeek,
            etaWeeks = eta,
            // A bodyweight goal is the only goal you can be AT: reached, and holding. `movingToward`
            // answers one question — is the slope pointing at the target — and at the target it fell
            // to the `else` branch and demanded a POSITIVE slope. So an athlete who hit their goal
            // weight and maintained it read as off track, and for a cut the only reading that
            // counted as on track was putting the weight back on. `reached` uses the same tolerance
            // the goal is declared met at, so "met" and "on track" can never contradict each other.
            onTrack = if (current == null || target == null || perWeek == null) null
            else reached || movingToward(current, target, perWeek),
            reachedNow = reached,
            reading = when {
                current == null -> "no weigh-ins yet"
                target == null -> "${current.roundToInt()} lb"
                else -> "${current.roundToInt()} lb, target ${target.roundToInt()}"
            }
        )
    }

    private fun consistencyState(goal: CoachGoal, kind: CoachGoalKind, s: AdaptationSnapshot): GoalState {
        val fourWeeks = s.nowMs - 4 * WEEK_MS
        val recent = s.sessions.filter { it.startedAt >= fourWeeks && it.finishedAt != null }
        val perWeekRate = recent.size / 4.0
        val target = goal.targetValue
        return GoalState(
            goal = goal, kind = kind, current = perWeekRate, target = target, perWeek = null,
            etaWeeks = null, onTrack = target?.let { perWeekRate >= it - 0.25 },
            reachedNow = target != null && perWeekRate >= target,
            reading = if (target == null) "${format1(perWeekRate)} sessions a week"
            else "${format1(perWeekRate)} of ${format1(target)} sessions a week"
        )
    }

    private fun conditioningState(goal: CoachGoal, kind: CoachGoalKind, s: AdaptationSnapshot): GoalState {
        val weekStart = mondayStartMs(s.nowMs, s.zoneId)
        val minutes = s.cardio
            .filter { it.date >= weekStart && it.restReason == null }
            .sumOf { it.durationMin }
            .toDouble()
        val target = goal.targetValue
        return GoalState(
            goal = goal, kind = kind, current = minutes, target = target, perWeek = null,
            etaWeeks = null, onTrack = target?.let { minutes >= it },
            reachedNow = target != null && minutes >= target,
            reading = if (target == null) "${minutes.roundToInt()} min this week"
            else "${minutes.roundToInt()} of ${target.roundToInt()} min this week"
        )
    }

    private fun balanceState(goal: CoachGoal, kind: CoachGoalKind, s: AdaptationSnapshot): GoalState {
        val ratio = BalancePair.fromCode(goal.targetKey)?.let { pair ->
            val fourWeeks = s.nowMs - 4 * WEEK_MS
            val byMuscle = mutableMapOf<MuscleGroup, Int>()
            for (day in s.program) {
                for (slot in day.slots) {
                    val sets = s.exerciseHistory[slot.exerciseId].orEmpty()
                        .filter { it.countsForProgression && !it.skipped && it.sessionStartedAt >= fourWeeks }
                        .sumOf { it.sets.size }
                    if (sets > 0) byMuscle[slot.muscle] = (byMuscle[slot.muscle] ?: 0) + sets
                }
            }
            val a = pair.left.sumOf { byMuscle[it] ?: 0 }
            val b = pair.right.sumOf { byMuscle[it] ?: 0 }
            if (b <= 0) null else a.toDouble() / b
        }
        // The target for a balance goal is always parity; targetValue tightens the band if set.
        val band = goal.targetValue ?: 0.15
        val reached = ratio != null && abs(ratio - 1.0) <= band
        return GoalState(
            goal = goal, kind = kind, current = ratio, target = 1.0, perWeek = null,
            etaWeeks = null, onTrack = if (ratio == null) null else reached,
            reachedNow = reached,
            reading = if (ratio == null) "not enough logged work on both sides"
            else "${format2(ratio)} to 1"
        )
    }

    // ── Conflict matrix ────────────────────────────────────────────────────────

    private fun conflict(a: CoachGoal, b: CoachGoal, s: AdaptationSnapshot? = null): GoalConflict? {
        val ka = CoachGoalKind.fromCode(a.kind) ?: return null
        val kb = CoachGoalKind.fromCode(b.kind) ?: return null

        // Losing weight while chasing a maximal strength target draws on one recovery + energy
        // budget. Compatible enough to hold strength; not compatible with adding to it fast.
        if (ka == CoachGoalKind.BODYWEIGHT && kb == CoachGoalKind.LIFT_1RM ||
            kb == CoachGoalKind.BODYWEIGHT && ka == CoachGoalKind.LIFT_1RM
        ) {
            val (weightGoal, liftGoal) = if (ka == CoachGoalKind.BODYWEIGHT) a to b else b to a
            // Only a deficit conflicts. A bulk feeds a max; an unknown direction is not grounds to
            // tell someone their goals fight.
            if (isCut(weightGoal, s) != true) return null
            return GoalConflict(
                first = weightGoal, second = liftGoal,
                explanation = "Losing weight and adding to a max draw on the same recovery budget.",
                proposal = "Run the weight goal first, then a strength block — holding strength while you cut already counts as winning."
            )
        }
        if (ka == CoachGoalKind.BODYWEIGHT && kb == CoachGoalKind.MUSCLE_VOLUME ||
            kb == CoachGoalKind.BODYWEIGHT && ka == CoachGoalKind.MUSCLE_VOLUME
        ) {
            val (weightGoal, volumeGoal) = if (ka == CoachGoalKind.BODYWEIGHT) a to b else b to a
            // "Needs a surplus" is an argument against a CUT. Gaining weight to build a muscle is
            // the plan, not a conflict with it.
            if (isCut(weightGoal, s) != true) return null
            return GoalConflict(
                first = weightGoal, second = volumeGoal,
                explanation = "Building a muscle needs a surplus more than it needs volume alone.",
                proposal = "Pick which one leads this block; the other holds a maintenance floor."
            )
        }
        // A big conditioning block on top of a max-strength target is the classic interference case.
        if (ka == CoachGoalKind.CONDITIONING && kb == CoachGoalKind.LIFT_1RM ||
            kb == CoachGoalKind.CONDITIONING && ka == CoachGoalKind.LIFT_1RM
        ) {
            val (cardioGoal, liftGoal) = if (ka == CoachGoalKind.CONDITIONING) a to b else b to a
            val minutes = cardioGoal.targetValue ?: 0.0
            if (minutes >= HIGH_CONDITIONING_MINUTES) {
                return GoalConflict(
                    first = cardioGoal, second = liftGoal,
                    explanation = "That much weekly conditioning competes with a strength peak.",
                    proposal = "Keep the conditioning as a base while you peak, then raise it after."
                )
            }
        }
        // Two maximal-strength targets on the same lift is not a portfolio, it's a duplicate.
        if (ka == CoachGoalKind.LIFT_1RM && kb == CoachGoalKind.LIFT_1RM && a.targetKey == b.targetKey) {
            return GoalConflict(
                first = a, second = b,
                explanation = "Two targets on the same lift.",
                proposal = "Keep the further one; the nearer number is a milestone on the way.",
                lessonId = null
            )
        }
        return null
    }

    /** Weekly conditioning minutes past which conditioning starts costing a strength peak. */
    private const val HIGH_CONDITIONING_MINUTES = 180.0

    // ── Shared math ────────────────────────────────────────────────────────────

    /**
     * Least-squares slope per week over (timeMs, value) points, or null below the data gate.
     * Robust enough for a trajectory line and honest about sparse data.
     */
    internal fun weeklySlope(points: List<Pair<Long, Double>>): Double? {
        if (points.size < 3) return null
        val spanMs = points.last().first - points.first().first
        if (spanMs < MIN_WEEKS_FOR_RATE * WEEK_MS) return null
        val xs = points.map { (it.first - points.first().first).toDouble() / WEEK_MS }
        val ys = points.map { it.second }
        val meanX = xs.average()
        val meanY = ys.average()
        var num = 0.0
        var den = 0.0
        for (i in xs.indices) {
            num += (xs[i] - meanX) * (ys[i] - meanY)
            den += (xs[i] - meanX) * (xs[i] - meanX)
        }
        return if (den == 0.0) null else num / den
    }

    private fun etaWeeks(current: Double?, target: Double?, perWeek: Double?): Int? {
        if (current == null || target == null || perWeek == null || perWeek == 0.0) return null
        val remaining = target - current
        if (abs(remaining) < 1e-6) return 0
        val weeks = remaining / perWeek
        if (weeks <= 0) return null // moving away from it; the ETA would be a lie
        val rounded = kotlin.math.ceil(weeks).toInt()
        return if (rounded > MAX_ETA_WEEKS) null else rounded
    }

    private fun onTrack(current: Double?, target: Double?, perWeek: Double?): Boolean? {
        if (current == null || target == null || perWeek == null) return null
        if (current >= target) return true
        return perWeek > 0
    }

    /** Direction only — whether the goal is already MET is the caller's question, not this one's. */
    private fun movingToward(current: Double, target: Double, perWeek: Double): Boolean =
        if (target < current) perWeek < 0 else perWeek > 0

    private fun format1(v: Double): String =
        if (v % 1.0 == 0.0) v.roundToInt().toString() else String.format(java.util.Locale.US, "%.1f", v)

    private fun format2(v: Double): String = String.format(java.util.Locale.US, "%.2f", v)
}

/** The two-sided balance goals the coach can track, as set-volume ratios over the last 4 weeks. */
enum class BalancePair(val code: String, val left: List<MuscleGroup>, val right: List<MuscleGroup>) {
    PUSH_PULL(
        "push_pull",
        listOf(MuscleGroup.CHEST, MuscleGroup.SHOULDERS, MuscleGroup.TRICEPS),
        listOf(MuscleGroup.BACK, MuscleGroup.BICEPS, MuscleGroup.REAR_DELTS)
    ),
    QUAD_HAM(
        "quad_ham",
        listOf(MuscleGroup.QUADS),
        listOf(MuscleGroup.HAMSTRINGS, MuscleGroup.GLUTES)
    );

    companion object {
        fun fromCode(code: String): BalancePair? = entries.firstOrNull { it.code == code }
    }
}
