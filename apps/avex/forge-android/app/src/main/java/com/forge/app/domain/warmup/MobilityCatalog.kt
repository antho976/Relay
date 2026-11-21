package com.forge.app.domain.warmup

import com.forge.app.program.MuscleGroup

/**
 * The dynamic drills the mobilise phase draws from, keyed by the muscle group that earns them.
 *
 * Two rules decide what is in here, and both are load-bearing:
 *
 * 1. **Dynamic only.** Static stretching held before lifting measurably lowers force and power output
 *    (Simic 2013, meta-analysis of 104 studies; the loss scales with hold duration and is largest past
 *    60s). A warmup that costs you reps is not a warmup, so nothing in this catalog is a held stretch.
 * 2. **Only joints this session loads.** Drilling shoulders before a leg day spends the user's
 *    patience on a joint that is not about to be asked for anything. Every entry is claimed by the
 *    muscle group it actually prepares, and [forMuscles] returns nothing for groups not trained.
 *
 * The small single-joint groups (biceps, triceps, rear delts, calves) deliberately have no entry:
 * they are warmed adequately by the raise phase plus the ramp sets on the compound that precedes
 * them, and giving each one its own drill is how a warmup turns into a second workout.
 */
internal object MobilityCatalog {

    /**
     * Drills per muscle group. Order within a list is priority: [forMuscles] takes the first that
     * has not already been picked, so overlapping groups (quads and glutes both wanting leg swings)
     * contribute distinct drills rather than the same one twice.
     */
    private val byMuscle: Map<MuscleGroup, List<WarmupDrill>> = mapOf(
        MuscleGroup.CHEST to listOf(
            drill("scap-pushup", "Scapular push-ups", "10 reps", "Wakes the serratus that stabilises every press", 40),
            drill("arm-swing-cross", "Cross-body arm swings", "10 each side", "Takes the shoulder through its pressing range", 30)
        ),
        MuscleGroup.BACK to listOf(
            drill("cat-cow", "Cat cow", "8 slow reps", "Frees the mid-back that rows pull from", 40),
            drill("scap-hang", "Scapular hangs", "8 reps", "Trains the shoulder blade to move before you load it", 40)
        ),
        MuscleGroup.SHOULDERS to listOf(
            drill("arm-circles", "Arm circles", "10 forward, 10 back", "Warms the joint through its full circle", 30),
            drill("dislocates", "Towel dislocates", "10 reps", "Opens overhead range with a wide grip", 40)
        ),
        MuscleGroup.QUADS to listOf(
            drill("bw-squat", "Bodyweight squats", "10 slow reps", "Rehearses the exact pattern you are about to load", 45),
            drill("leg-swing-lateral", "Lateral leg swings", "10 each leg", "Loosens the hip for a deeper squat", 40)
        ),
        MuscleGroup.HAMSTRINGS to listOf(
            drill("leg-swing-front", "Leg swings, front to back", "10 each leg", "Lengthens the hamstring without holding a stretch", 40),
            drill("hinge-drill", "Unloaded hip hinge", "10 reps", "Grooves the hinge before you add weight", 40)
        ),
        MuscleGroup.GLUTES to listOf(
            drill("glute-bridge", "Glute bridges", "12 reps", "Switches the glutes on so the low back does less", 45),
            drill("hip-circles", "Standing hip circles", "8 each side", "Frees the hip in the plane lunges use", 40)
        ),
        MuscleGroup.CORE to listOf(
            drill("dead-bug", "Dead bugs", "8 each side", "Braces the trunk that every heavy set relies on", 45)
        )
    )

    private fun drill(id: String, name: String, prescription: String, why: String, seconds: Int) =
        WarmupDrill(id, WarmupPhase.MOBILIZE, name, prescription, why, seconds)

    /**
     * Up to [limit] distinct drills for the muscle groups this session trains, in the order the
     * groups were passed (the caller ranks them by planned set count, so the most-worked joint is
     * prepared first and is the one that survives the cap).
     *
     * The cap is low on purpose. Warmup effect is real but bounded, and the thing the user actually
     * needs is the ramp on the bar, not a mobility circuit. Two drills cover the joints a session
     * leans on without turning a two-minute task into a routine of its own.
     */
    fun forMuscles(muscles: List<MuscleGroup>, limit: Int = 2): List<WarmupDrill> {
        val picked = LinkedHashMap<String, WarmupDrill>()
        // Round-robin by rank: every trained group gets its first-choice drill before any group gets
        // a second one, so a day training three muscles never spends the whole cap on the first.
        var depth = 0
        while (picked.size < limit && depth < MAX_DEPTH) {
            var addedThisPass = false
            for (muscle in muscles) {
                if (picked.size >= limit) break
                val candidate = byMuscle[muscle]?.getOrNull(depth) ?: continue
                if (picked.putIfAbsent(candidate.id, candidate) == null) addedThisPass = true
            }
            if (!addedThisPass) break
            depth++
        }
        return picked.values.toList()
    }

    /** Deepest per-muscle drill list, so the round-robin above always terminates. */
    private val MAX_DEPTH = byMuscle.values.maxOfOrNull { it.size } ?: 0
}
