package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.RestEvent
import com.forge.app.data.db.types.EffortRating
import com.forge.app.program.ExercisePlan
import com.forge.app.program.SessionEstimate
import kotlin.math.abs
import kotlin.math.roundToInt

/** The two rest-behavior buckets — compounds and isolations rest differently. */
enum class MovementRole { COMPOUND, ISOLATION }

/** One qualified observation: how long the user actually rested vs the role's base prescription. */
data class RestSample(val role: MovementRole, val baseSeconds: Int, val realizedSeconds: Int)

/**
 * Personal rest-correction factors per [MovementRole], learned from [RestSample]s.
 * A factor of 0.85 means "this user consistently rests ~15% less than the canonical
 * prescription". Neutral (factor absent / too few samples) means the canonical
 * [SessionEstimate] values are used untouched.
 */
data class RestTuning(
    val factors: Map<MovementRole, Double> = emptyMap(),
    val sampleCounts: Map<MovementRole, Int> = emptyMap()
) {
    companion object {
        val NEUTRAL = RestTuning()
    }
}

/** A rest duration plus the always-present explanation of how it was derived. */
data class RestPrescription(val seconds: Int, val reason: String)

/**
 * System 2 of the adaptation engine: adaptive rest. Pure — both entry points are
 * deterministic functions of their inputs.
 *
 * Base prescription comes from [SessionEstimate.restSeconds] (movement-role + rep-heaviness
 * aware) plus the existing +30s-after-brutal rule; on top of that, a per-role personal
 * correction factor is learned from realized rest behavior ([RestEvent]s) — the median of
 * realized ÷ base, clamped to ±[AdaptThresholds.maxRestAdjust], applied only once a role
 * has ≥ [AdaptThresholds.minRestSamples] qualified samples. Realized is compared against
 * the *base*, not the (possibly already-tuned) planned seconds, so the factor converges
 * instead of oscillating back to neutral once the user follows the tuned timer.
 *
 * A per-exercise manual override (#59) always wins — a user's explicit choice is not a
 * signal to second-guess.
 */
object RestAdvisor {

    fun movementRole(plan: ExercisePlan): MovementRole =
        if (SessionEstimate.isCompound(plan)) MovementRole.COMPOUND else MovementRole.ISOLATION

    /**
     * Map raw [events] to qualified samples. Drops events outside sanity bounds (sub-15s
     * intervals are mistaps; 10+ minutes means the user wandered off) and events whose
     * exercise can't be resolved anymore ([planFor] returns null).
     */
    fun samples(
        events: List<RestEvent>,
        t: AdaptThresholds = AdaptThresholds(),
        compoundBase: Int = SessionEstimate.COMPOUND_REST,
        isolationBase: Int = SessionEstimate.ISOLATION_REST,
        planFor: (String) -> ExercisePlan?
    ): List<RestSample> = events.mapNotNull { e ->
        if (e.realizedSeconds < t.minSaneRestSeconds || e.realizedSeconds > t.maxSaneRestSeconds) return@mapNotNull null
        val plan = planFor(e.exerciseId) ?: return@mapNotNull null
        RestSample(
            role = movementRole(plan),
            // The factor is realized ÷ base, so this base MUST be the SAME one [restSeconds] later
            // applies the factor to (the user's Session-settings override), or the learned correction
            // mis-scales every prescription for anyone whose base isn't the canonical 180/90.
            baseSeconds = SessionEstimate.restSeconds(plan, compoundBase, isolationBase),
            realizedSeconds = e.realizedSeconds
        )
    }

    /** Learn per-role correction factors: median(realized ÷ base), clamped to ±maxRestAdjust. */
    fun tuning(samples: List<RestSample>, t: AdaptThresholds = AdaptThresholds()): RestTuning {
        val byRole = samples.filter { it.baseSeconds > 0 }.groupBy { it.role }
        val factors = byRole.mapValues { (_, list) ->
            median(list.map { it.realizedSeconds.toDouble() / it.baseSeconds })
                .coerceIn(1 - t.maxRestAdjust, 1 + t.maxRestAdjust)
        }
        return RestTuning(factors = factors, sampleCounts = byRole.mapValues { it.value.size })
    }

    /** The rest the timer should run for after a set of [plan], with its explanation. */
    fun restSeconds(
        plan: ExercisePlan,
        lastEffort: EffortRating?,
        overrideSeconds: Int?,
        tuning: RestTuning = RestTuning.NEUTRAL,
        t: AdaptThresholds = AdaptThresholds(),
        compoundBase: Int = SessionEstimate.COMPOUND_REST,
        isolationBase: Int = SessionEstimate.ISOLATION_REST
    ): RestPrescription {
        if (overrideSeconds != null) {
            return RestPrescription(overrideSeconds, "your custom rest for this exercise")
        }
        val role = movementRole(plan)
        val base = SessionEstimate.restSeconds(plan, compoundBase, isolationBase)
        val roleLabel = if (role == MovementRole.COMPOUND) "compound" else "isolation"
        val parts = mutableListOf("$roleLabel base ${mmss(base)}")

        val factor = tuning.factors[role]
            ?.takeIf { (tuning.sampleCounts[role] ?: 0) >= t.minRestSamples }
        var seconds = base
        if (factor != null) {
            seconds = roundTo(base * factor, t.restRoundSeconds)
            val pct = ((factor - 1) * 100).roundToInt()
            if (abs(pct) >= 1 && seconds != base) {
                // Surface how much evidence the adjustment rests on, so the tuning reads as learned,
                // not arbitrary.
                val n = tuning.sampleCounts[role] ?: 0
                val pctPart = if (pct < 0) "−${-pct}% to match your usual rest" else "+$pct% to match your usual rest"
                parts += "$pctPart (from $n rest${if (n == 1) "" else "s"} logged)"
            }
        }
        if (lastEffort == EffortRating.BRUTAL) {
            seconds += t.brutalRestBonusSeconds
            parts += "+${t.brutalRestBonusSeconds}s after a brutal set"
        }
        return RestPrescription(seconds, parts.joinToString(" · "))
    }

    /**
     * Session-length estimate with the user's realized rest pace folded in. Falls back to
     * the canonical estimate exactly while tuning is below its sample gates (the rest
     * prescription itself enforces them via [restSeconds]).
     */
    fun estimateMinutes(
        plan: com.forge.app.program.DayPlan,
        tuning: RestTuning,
        t: AdaptThresholds = AdaptThresholds(),
        compoundBase: Int = SessionEstimate.COMPOUND_REST,
        isolationBase: Int = SessionEstimate.ISOLATION_REST
    ): Int = SessionEstimate.estimateMinutes(plan) { ex ->
        restSeconds(
            ex, lastEffort = null, overrideSeconds = null, tuning = tuning, t = t,
            compoundBase = compoundBase, isolationBase = isolationBase
        ).seconds
    }

    private fun median(values: List<Double>): Double {
        val sorted = values.sorted()
        val mid = sorted.size / 2
        return if (sorted.size % 2 == 1) sorted[mid] else (sorted[mid - 1] + sorted[mid]) / 2
    }

    private fun roundTo(value: Double, step: Int): Int =
        ((value / step).roundToInt() * step).coerceAtLeast(step)

    private fun mmss(seconds: Int): String = "${seconds / 60}:" + "%02d".format(seconds % 60)
}
