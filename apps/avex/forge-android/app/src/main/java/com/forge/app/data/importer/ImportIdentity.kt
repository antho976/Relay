package com.forge.app.data.importer

/**
 * What makes one imported workout the SAME workout as one already in the log.
 *
 * The duplicate guard compares a workout in the file against the workouts already stored in its
 * start window and skips it when they match. That comparison used to be an exact print of every
 * field the insert writes (M-03), and that made it duplicate history on the most common re-imports:
 *
 * - the print included fields the user edits after importing (an untracked flag, an exercise
 *   note), so re-importing the next Strong export added a second copy of every workout touched;
 * - it included the catalogue id the name MATCHER chose, so an app update that learned to match
 *   "Cable Fly" duplicated every session containing it;
 * - it included values the app derives (active time, per-set instants), which Avex's own export
 *   does not always reproduce, so importing that export onto the device that wrote it doubled
 *   overlapping sessions.
 *
 * So identity is built only from facts the SOURCE states and nothing on this device rewrites: the
 * start slot (the caller's window), the movement, and each performed set's reps, load, hold time,
 * RPE and warm-up flag. A corrected export whose only change is an annotation is now recognised as
 * the workout it annotates and skipped. An import merges; it never overwrites, so nothing on this
 * device is lost, but the correction is not applied either. Replacing in place needs a source
 * identity stored on the session row (see `docs/AUDIT_DEFERRED.md`, M-03).
 *
 * A movement is one set of [ExerciseIdentity.keys], the ids and names it could be stored under, and
 * two movements match when those sets overlap. A stored `ext-cable-fly` therefore still matches an
 * incoming "Cable Fly" that now resolves to the catalogue.
 *
 * Skipped and empty exercises carry no work and are left out on both sides, so a skip that one side
 * records and the other omits cannot make one workout look like two.
 */
internal data class SetIdentity(
    val reps: Int,
    /** Tenths of a pound: importers round to 0.1 lb, and a re-import of the same file is bit-identical. */
    val weightTenthsLb: Long?,
    val durationSeconds: Int?,
    val rpeTenths: Int?,
    val warmup: Boolean
) {
    companion object {
        fun of(reps: Int, weightLb: Double?, durationSeconds: Int?, rpe: Double?, warmup: Boolean) = SetIdentity(
            reps = reps,
            weightTenthsLb = weightLb?.takeIf { it > 0.0 }?.let { Math.round(it * 10.0) },
            durationSeconds = durationSeconds?.takeIf { it > 0 },
            rpeTenths = rpe?.takeIf { it in 1.0..10.0 }?.let { Math.round(it * 10.0).toInt() },
            warmup = warmup
        )
    }
}

internal data class ExerciseIdentity(
    val orderIndex: Int,
    /** Namespaced ("id:bench_press", "name:bench press") so an id can never equal a name. */
    val keys: Set<String>,
    val sets: List<SetIdentity>
) {
    companion object {
        fun keysOf(ids: List<String?>, names: List<String?>): Set<String> = buildSet {
            ids.forEach { id -> id?.trim()?.takeIf { it.isNotEmpty() }?.let { add("id:$it") } }
            names.forEach { n -> n?.trim()?.lowercase()?.takeIf { it.isNotEmpty() }?.let { add("name:$it") } }
        }
    }
}

internal data class WorkoutIdentity(val exercises: List<ExerciseIdentity>) {
    /** The exercises that carry work, in stored order (ties keep their given order). */
    private val performed: List<ExerciseIdentity>
        get() = exercises.filter { it.sets.isNotEmpty() }.sortedBy { it.orderIndex }

    fun sameWorkoutAs(other: WorkoutIdentity): Boolean {
        val mine = performed
        val theirs = other.performed
        return mine.size == theirs.size && mine.zip(theirs).all { (a, b) ->
            a.sets == b.sets && a.keys.any { it in b.keys }
        }
    }

    /**
     * Whether this incoming workout is a fuller read of [stored], a copy an earlier build's importer
     * wrote with less in it. Re-importing is exactly what a user does once an importer is fixed, and
     * those copies no longer matched, so every one was added a second time. Three losses are
     * recognised:
     *
     * - a set stored with no load where this read has one (Hevy lb exports read only `weight_kg`);
     * - a hold-only exercise stored that this read doesn't have (Strong and Hevy cardio rows, and
     *   Strong's Rest Timer, came in as timed holds);
     * - a hold-only exercise this read has that wasn't stored (FitNotes dropped timed holds).
     *
     * Everything else must agree exactly, and at least one of those must differ.
     */
    fun repairs(stored: WorkoutIdentity): Boolean {
        if (sameWorkoutAs(stored)) return false
        val theirsAll = stored.performed
        val mineAll = performed
        val mine = mineAll.filterNot { it.holdOnly && theirsAll.none { s -> s.sharesKeyWith(it) } }
        val theirs = theirsAll.filterNot { it.holdOnly && mineAll.none { m -> m.sharesKeyWith(it) } }
        return mine.isNotEmpty() && mine.size == theirs.size && mine.zip(theirs).all { (a, b) ->
            a.sharesKeyWith(b) && a.sets.size == b.sets.size && a.sets.zip(b.sets).all { (x, y) ->
                x == y || (y.weightTenthsLb == null && x.copy(weightTenthsLb = null) == y)
            }
        }
    }

    private val ExerciseIdentity.holdOnly: Boolean get() = sets.all { it.durationSeconds != null }

    private fun ExerciseIdentity.sharesKeyWith(other: ExerciseIdentity) = keys.any { it in other.keys }
}
