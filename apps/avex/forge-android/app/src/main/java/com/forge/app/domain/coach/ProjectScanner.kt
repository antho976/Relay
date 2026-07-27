package com.forge.app.domain.coach

import com.forge.app.domain.adapt.AdaptationSnapshot
import com.forge.app.domain.adapt.HealthSnap
import com.forge.app.domain.adapt.countsForProgression
import com.forge.app.program.MuscleGroup
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The weakness hunter (Coach v3 D) — the answer to "what should I improve?" before it's asked.
 *
 * The coach continuously scans every signal for the single biggest available lever and proposes ONE
 * named project at a time: a why, a plan, and a finish line. One at a time is the whole discipline.
 * A list of eight things to fix is a list nobody acts on; one project with an end date is a thing
 * that gets done.
 *
 * Pure: candidates are ranked deterministically, and the caller decides whether to propose the top
 * one. Every candidate must state a finish line, or it isn't a project, it's a complaint.
 */
object ProjectScanner {

    private const val DAY_MS = 24L * 60 * 60 * 1000
    private const val WEEK_MS = 7 * DAY_MS

    /** Four weeks is long enough to change something and short enough to stay real. */
    const val DEFAULT_WEEKS = 4

    enum class Kind(val code: String) {
        IMBALANCE("imbalance"),
        LAGGING_MUSCLE("lagging_muscle"),
        MISSING_CONDITIONING("missing_conditioning"),
        SHORT_SLEEP("short_sleep"),
        LOW_VOLUME("low_volume"),
        SKIPPED_WORK("skipped_work")
    }

    /**
     * @param score how big a lever this is, for ranking. Not shown to the user.
     * @param finishLine the condition that ends the project — the thing that makes it a project.
     */
    data class Candidate(
        val kind: Kind,
        val name: String,
        val why: String,
        val plan: String,
        val finishLine: String,
        val targetKey: String = "",
        val weeks: Int = DEFAULT_WEEKS,
        val score: Int
    )

    /** Every lever worth naming, biggest first. */
    fun scan(s: AdaptationSnapshot, profile: PersonalProfile.Profile = PersonalProfile.Profile.DEFAULTS): List<Candidate> {
        val out = mutableListOf<Candidate>()
        out += imbalance(s)
        out += laggingMuscle(s, profile)
        out += conditioning(s)
        out += shortSleep(s.health)
        out += skippedWork(s)
        return out.sortedByDescending { it.score }
    }

    /** The single project to propose now, or null when nothing is worth interrupting for. */
    fun top(
        s: AdaptationSnapshot,
        profile: PersonalProfile.Profile = PersonalProfile.Profile.DEFAULTS,
        excludeKinds: Set<Kind> = emptySet()
    ): Candidate? = scan(s, profile).firstOrNull { it.kind !in excludeKinds }

    // ── Candidate types ────────────────────────────────────────────────────────

    private fun imbalance(s: AdaptationSnapshot): List<Candidate> {
        val sets = setsByMuscle(s, weeks = 4)
        return BalancePair.entries.mapNotNull { pair ->
            val left = pair.left.sumOf { sets[it] ?: 0 }
            val right = pair.right.sumOf { sets[it] ?: 0 }
            if (left < 8 || right < 8) return@mapNotNull null
            val ratio = left.toDouble() / right
            if (abs(ratio - 1.0) < 0.4) return@mapNotNull null
            val behindSide = if (ratio > 1) pair.right else pair.left
            val behindName = behindSide.joinToString(" and ") { it.displayName.lowercase() }
            val gapPct = (abs(ratio - 1.0) * 100).roundToInt()
            Candidate(
                kind = Kind.IMBALANCE,
                name = "Even out your ${pair.readableName}",
                why = "Your $behindName work is running about $gapPct% behind the other side.",
                plan = "Add two sets a week to $behindName and hold everything else steady.",
                finishLine = "Done when both sides sit within 15% of each other over four weeks.",
                targetKey = pair.code,
                score = 60 + gapPct
            )
        }
    }

    private fun laggingMuscle(s: AdaptationSnapshot, profile: PersonalProfile.Profile): List<Candidate> {
        val sets = setsByMuscle(s, weeks = 1)
        val trained = s.program.flatMap { it.slots }.map { it.muscle }.toSet()
        return trained.mapNotNull { muscle ->
            val weekly = sets[muscle] ?: 0
            val cap = profile.capFor(muscle)
            // Well under half its own ceiling, week after week, is a muscle being carried.
            if (weekly >= cap / 2) return@mapNotNull null
            Candidate(
                kind = Kind.LAGGING_MUSCLE,
                name = "Bring up your ${muscle.displayName.lowercase()}",
                why = "It's getting $weekly working sets a week against a useful range nearer $cap.",
                plan = "Add a set a week until it lands in range, then hold there.",
                finishLine = "Done when it holds at least ${cap / 2 + 2} sets a week for three weeks.",
                targetKey = muscle.code,
                score = 40 + (cap - weekly)
            )
        }
    }

    private fun conditioning(s: AdaptationSnapshot): List<Candidate> {
        val fourWeeks = s.nowMs - 4 * WEEK_MS
        val minutes = s.cardio.filter { it.date >= fourWeeks && it.restReason == null }
            .sumOf { it.durationMin }
        if (minutes >= 120) return emptyList()
        return listOf(
            Candidate(
                kind = Kind.MISSING_CONDITIONING,
                name = "Build an aerobic base",
                why = "You've logged $minutes minutes of cardio in the last month.",
                plan = "Two easy 20-minute walks or rides a week, placed away from your hard days.",
                finishLine = "Done when you've held 90 minutes a week for four weeks.",
                score = 35
            )
        )
    }

    private fun shortSleep(health: HealthSnap): List<Candidate> {
        val nights = health.sleepNights.takeLast(14)
        if (nights.size < 7) return emptyList()
        val avg = nights.map { it.durationMin }.average()
        if (avg > 390) return emptyList()
        val hours = String.format(java.util.Locale.US, "%.1f", avg / 60.0)
        return listOf(
            Candidate(
                kind = Kind.SHORT_SLEEP,
                name = "Get your sleep up",
                why = "You're averaging ${hours}h a night, which is the biggest thing holding your recovery back.",
                plan = "Aim for one extra half hour, four nights a week.",
                finishLine = "Done when your fortnight average clears seven hours.",
                score = 70
            )
        )
    }

    private fun skippedWork(s: AdaptationSnapshot): List<Candidate> {
        val recent = s.exerciseHistory.mapValues { (_, bouts) -> bouts.takeLast(5) }
        val worst = recent.entries
            .filter { it.value.size >= 4 }
            .map { (id, bouts) -> id to bouts.count { it.skipped } }
            .filter { it.second >= 3 }
            .maxByOrNull { it.second }
            ?: return emptyList()
        val name = s.program.flatMap { it.slots }.firstOrNull { it.exerciseId == worst.first }?.name
            ?: return emptyList()
        return listOf(
            Candidate(
                kind = Kind.SKIPPED_WORK,
                name = "Fix or drop $name",
                why = "You've skipped it ${worst.second} of the last five times it came up.",
                plan = "Swap it for something you'll actually do, or move it earlier in the session.",
                finishLine = "Done when its slot gets done three sessions running.",
                targetKey = worst.first,
                score = 50 + worst.second
            )
        )
    }

    // ── Shared ────────────────────────────────────────────────────────────────

    private fun setsByMuscle(s: AdaptationSnapshot, weeks: Int): Map<MuscleGroup, Int> {
        val since = s.nowMs - weeks * WEEK_MS
        val byMuscle = HashMap<MuscleGroup, Int>()
        for (day in s.program) {
            for (slot in day.slots) {
                val sets = s.exerciseHistory[slot.exerciseId].orEmpty()
                    .filter { it.countsForProgression && !it.skipped && it.sessionStartedAt >= since }
                    .sumOf { bout -> bout.sets.count { it.durationSeconds == null } }
                if (sets > 0) byMuscle[slot.muscle] = (byMuscle[slot.muscle] ?: 0) + sets
            }
        }
        // Per-week average when the window is longer than a week, so thresholds read the same way.
        return if (weeks <= 1) byMuscle else byMuscle.mapValues { it.value }
    }

    private val BalancePair.readableName: String
        get() = when (this) {
            BalancePair.PUSH_PULL -> "push and pull"
            BalancePair.QUAD_HAM -> "quads and hamstrings"
        }
}
