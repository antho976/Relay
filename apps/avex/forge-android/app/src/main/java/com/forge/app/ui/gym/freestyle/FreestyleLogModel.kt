package com.forge.app.ui.gym.freestyle

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatHold
import com.forge.app.domain.units.parseHold
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.weightInputValue
import com.forge.app.program.CustomExerciseRegistry
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.ExercisePlan
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import com.forge.app.ui.common.rpeLabel
import java.util.Locale

/**
 * One set in the freestyle log. Every set in [FsExercise.sets] is a LOGGED set: the logger adds a
 * set only when the user commits it from the entry slab, so there are no half-typed rows in the
 * list. [weight], [reps] and [hold] keep the raw display-unit text exactly as typed.
 */
internal data class FsSet(
    val weight: String = "",
    val reps: String = "",
    val setType: String? = null,       // null | "warmup" | "drop", the mutually-exclusive shape
    val isAmrap: Boolean = false,
    val toFailure: Boolean = false,
    val rpe: Double? = null,
    /** Raw hold-time text for a timed-hold set (GYMAP-51), e.g. "1:30" or "45"; blank for a rep set. */
    val hold: String = ""
) {
    val hasTags: Boolean get() = setType != null || isAmrap || toFailure || rpe != null

    /** Terse mono badges for a set, matching the session-detail set-table words (GYMAP-46). */
    fun tagLabels(): List<String> = buildList {
        when (setType) { "warmup" -> add("WARM"); "drop" -> add("DROP") }
        if (isAmrap) add("AMRAP")
        if (toFailure) add("FAIL")
        rpe?.let { add("RPE ${rpeLabel(it)}") }
    }

    /** The same numbers with every tag cleared: what the next set starts from. */
    fun numbersOnly(): FsSet = FsSet(weight = weight, reps = reps, hold = hold)
}

internal data class FsExercise(
    val libId: String,
    val name: String,
    /**
     * Null only for a non-library, non-custom id (an importer `ext-…` row) whose muscle nothing
     * knows. It used to fall back to the first muscle, so those moves were logged as Chest and the
     * label was then written into the custom registry on save (audit 2026-09-26, 02).
     */
    val muscle: MuscleGroup?,
    val bodyweight: Boolean,
    /** Timed-hold exercise (GYMAP-51): sets log a held duration (mm:ss) instead of reps. */
    val timed: Boolean = false,
    /** User-created move, not a library one. Its name/muscle live here and in the draft/log row,
     *  since there's no [ExerciseLibrary] entry to re-derive them from. */
    val custom: Boolean = false,
    val sets: List<FsSet> = emptyList()
)

/**
 * The entry slab's state for one exercise: the numbers waiting to be logged, and which logged set
 * (if any) they are editing. Kept apart from [FsExercise] so an untouched slab can keep re-seeding
 * from the latest data instead of freezing whatever it showed first.
 */
internal data class FsEntry(val set: FsSet, val editing: Int? = null)

/**
 * Whether a set counts as logged: a rep set needs positive reps, a timed hold (GYMAP-51) needs a
 * hold time that parses to more than zero seconds; a hold's reps are legitimately zero. The one
 * predicate behind the Log button, the session total and the Save gate, so they can never disagree.
 */
internal fun isLoggedFreestyleSet(timed: Boolean, reps: String, hold: String): Boolean =
    if (timed) (parseHold(hold) ?: 0) > 0 else (reps.toIntOrNull() ?: 0) > 0

internal fun FsExercise.isLogged(set: FsSet): Boolean = isLoggedFreestyleSet(timed, set.reps, set.hold)

/** Volume of one set in lb (weight × reps); bodyweight moves and holds add no external load. */
internal fun FsExercise.setVolumeLb(set: FsSet, weightUnit: WeightUnit): Double {
    if (bodyweight || timed) return 0.0
    val reps = set.reps.toIntOrNull() ?: return 0.0
    val w = parseToLb(set.weight, weightUnit) ?: return 0.0
    return w * reps
}

internal fun FsExercise.volumeLb(weightUnit: WeightUnit): Double = sets.sumOf { setVolumeLb(it, weightUnit) }

/**
 * Epley estimated one-rep max in lb, or null for a set it cannot rate (a hold, bodyweight, no
 * weight). Used only to say whether a set out-lifted last time, never shown as a number here.
 */
internal fun estimatedMaxLb(weightLb: Double?, reps: Int): Double? {
    if (weightLb == null || weightLb <= 0.0 || reps <= 0) return null
    return weightLb * (1 + reps / 30.0)
}

/** The best estimated max among last time's sets, the bar a set has to clear to earn the mark. */
internal fun lastTimeBestLb(lastTime: List<LoggedSet>): Double? =
    lastTime.mapNotNull { estimatedMaxLb(it.weightLb, it.reps) }.maxOrNull()

/** True when [set] out-lifts every set from last time (a weighted rep set only). */
internal fun FsExercise.beatsLastTime(set: FsSet, bestLastLb: Double?, weightUnit: WeightUnit): Boolean {
    if (bodyweight || timed || bestLastLb == null) return false
    val est = estimatedMaxLb(parseToLb(set.weight, weightUnit), set.reps.toIntOrNull() ?: 0) ?: return false
    return est > bestLastLb + 0.01
}

/** A prior set as the entry slab's text, in the current unit. Legacy holds keep seconds in `reps`. */
internal fun LoggedSet.toEntrySet(ex: FsExercise, weightUnit: WeightUnit): FsSet = FsSet(
    weight = if (ex.bodyweight) "" else weightLb?.let { weightInputValue(it, weightUnit) } ?: "",
    reps = if (ex.timed) "" else reps.toString(),
    hold = if (ex.timed) formatHold(durationSeconds ?: reps) else ""
)

/**
 * What the entry slab shows before the user touches it: the set just logged (so a straight set is
 * one tap), else last time's opening set, else nothing. Last time's later sets stay one tap away
 * on the Last row rather than being guessed at.
 */
internal fun FsExercise.seedEntry(lastTime: List<LoggedSet>, weightUnit: WeightUnit): FsSet =
    sets.lastOrNull()?.numbersOnly()
        ?: lastTime.firstOrNull()?.toEntrySet(this, weightUnit)
        ?: FsSet()

/** One set as a reading: "60 kg × 8", "BW × 12", "1:30", "20 kg · 1:30". */
internal fun FsExercise.setReading(set: FsSet, unitLabel: String): String {
    val w = set.weight.trim()
    val load = if (bodyweight || w.isEmpty()) null else "$w $unitLabel"
    return if (timed) {
        val hold = parseHold(set.hold)?.let { formatHold(it) } ?: set.hold
        if (load == null) hold else "$load · $hold"
    } else {
        "${load ?: "BW"} × ${set.reps}"
    }
}

/** A prior logged set as a compact reading for the Last time row: "60×8", "BW×12", "1:30". */
internal fun LoggedSet.compactReading(timed: Boolean, weightUnit: WeightUnit): String =
    if (timed) formatHold(durationSeconds ?: reps)
    else "${weightLb?.let { weightInputValue(it, weightUnit) } ?: weightText.ifBlank { "BW" }}×$reps"

/** Last time's top set as a reading with its unit ("60 kg × 8", "BW × 12", "1:30"), or null with no history. */
internal fun List<LoggedSet>.topReading(timed: Boolean, weightUnit: WeightUnit): String? {
    val top = (if (timed) maxByOrNull { it.durationSeconds ?: it.reps }
    else maxWithOrNull(compareBy<LoggedSet> { it.weightLb ?: 0.0 }.thenBy { it.reps })) ?: return null
    if (timed) return formatHold(top.durationSeconds ?: top.reps)
    val load = top.weightLb?.let { "${weightInputValue(it, weightUnit)} ${weightUnit.label}" } ?: "BW"
    return "$load × ${top.reps}"
}

/** The heaviest logged set, for a folded card's one-line summary. */
internal fun FsExercise.topSet(weightUnit: WeightUnit): FsSet? =
    if (timed) sets.maxByOrNull { parseHold(it.hold) ?: 0 }
    else sets.maxWithOrNull(
        compareBy<FsSet> { parseToLb(it.weight, weightUnit) ?: 0.0 }.thenBy { it.reps.toIntOrNull() ?: 0 }
    )

/** Nudge a display-unit weight string by [delta] (kept ≥0, '.'-decimal so it re-parses in any locale). */
internal fun stepWeightText(cur: String, delta: Double): String {
    val next = ((cur.toDoubleOrNull() ?: 0.0) + delta).coerceAtLeast(0.0)
    return if (next % 1.0 == 0.0) next.toInt().toString() else String.format(Locale.US, "%.1f", next)
}

internal fun stepRepsText(cur: String, delta: Int): String =
    ((cur.toIntOrNull() ?: 0) + delta).coerceAtLeast(0).toString()

/** Step a hold by [deltaSec], written back as mm:ss. */
internal fun stepHoldText(cur: String, deltaSec: Int): String =
    holdText(((parseHold(cur) ?: 0) + deltaSec).coerceAtLeast(0))

/** The weight stepper's increment in the display unit. Half a stone keeps a single-decimal field clean. */
internal fun weightStepFor(unit: WeightUnit): Double = when (unit) {
    WeightUnit.KG -> 2.5
    WeightUnit.ST -> 0.5
    else -> 5.0
}

/** Keep digits + a single colon (mm:ss) for a hold field (GYMAP-51). A stray second colon would make
 *  [parseHold] reject the value and the set would never count. */
internal fun sanitizeHoldText(input: String): String {
    val filtered = input.filter { it.isDigit() || it == ':' }
    val firstColon = filtered.indexOf(':')
    val collapsed = if (firstColon < 0) filtered
    else filtered.substring(0, firstColon + 1) + filtered.substring(firstColon + 1).replace(":", "")
    return collapsed.take(5)
}

/** Seconds as the hold field wants them (GYMAP-51): mm:ss. */
internal fun holdText(seconds: Int): String = "%d:%02d".format(seconds / 60, seconds % 60)

/** mm:ss, or h:mm:ss past an hour: the running session clock and the rest readout. */
internal fun formatElapsed(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    val h = s / 3600; val m = (s % 3600) / 60; val sec = s % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, sec) else "%d:%02d".format(m, sec)
}

/** A library move as a fresh log row. */
internal fun fsExerciseFor(libId: String): FsExercise? {
    val def = ExerciseLibrary.byId(libId) ?: return null
    return FsExercise(
        libId = def.id,
        name = def.name,
        muscle = def.muscle,
        bodyweight = def.unit == ExerciseUnit.BODYWEIGHT,
        timed = def.timed
    )
}

/**
 * A non-library id's program slot: the active program first, then the seed split, so a seed move
 * (ua1…lb6) that a regenerate rotated out keeps its real muscle and unit. Deliberately NOT
 * [Program.exercise], whose last fallback is the custom registry, where earlier builds registered
 * imported and seed moves as Chest (audit 2026-09-26, 02).
 */
internal fun nonLibraryPlan(libId: String): ExercisePlan? =
    Program.days.firstNotNullOfOrNull { d -> d.exercises.firstOrNull { it.id == libId } }
        ?: Program.seedDays.firstNotNullOfOrNull { d -> d.exercises.firstOrNull { it.id == libId } }

/**
 * The muscle of a move with no library entry. A user-created move's muscle lives in the registry
 * (first muscle only when even that was lost, as before); any other id takes its program slot's, or
 * stays null rather than guessing: a guessed Chest is what fed imported rows into chest volume.
 */
internal fun nonLibraryMuscle(libId: String): MuscleGroup? =
    if (isCustomExerciseId(libId)) CustomExerciseRegistry.muscle(libId) ?: MuscleGroup.entries.first()
    else nonLibraryPlan(libId)?.muscle

/** Snapshot the current log into a resumable draft (raw typed text is preserved verbatim, stamped
 *  with the unit it was typed in, see [FreestyleDraft.unitLabel]). */
internal fun draftFrom(
    items: List<FsExercise>,
    openedAtMs: Long,
    weightUnit: WeightUnit,
    draftId: String
): FreestyleDraft =
    FreestyleDraft(
        openedAtMs = openedAtMs,
        draftId = draftId,
        unitLabel = weightUnit.label,
        exercises = items.map { ex ->
            FreestyleDraftExercise(
                libId = ex.libId,
                sets = ex.sets.map {
                    FreestyleDraftSet(it.weight, it.reps, it.setType, it.isAmrap, it.toFailure, it.rpe, it.hold)
                },
                name = ex.name,
                muscleCode = ex.muscle?.code,
                bodyweight = ex.bodyweight,
                timed = ex.timed
            )
        }
    )

/**
 * Rebuild the in-memory log from a draft, re-deriving name/muscle/bodyweight from the library and
 * dropping any exercise whose library id no longer exists. A custom move carries its own identity
 * in the draft, so it restores from those fields instead.
 *
 * Weight text is re-expressed in [weightUnit] when the draft was typed in a different one: the draft
 * stores raw display-unit text, so without this a "100" drafted in lb came back as 100 kg after a
 * unit change and was saved as 220 lb.
 *
 * Drafts written by the previous logger could hold empty placeholder rows (it kept one blank row
 * per exercise); those are dropped here, since every set in the new log is a logged one.
 */
internal fun draftToItems(draft: FreestyleDraft, weightUnit: WeightUnit): List<FsExercise> =
    draft.exercises.distinctBy { it.libId }.mapNotNull { de ->
        val sets = de.sets.map {
            FsSet(draft.weightTextIn(it.weight, weightUnit), it.reps, it.setType, it.isAmrap, it.toFailure, it.rpe, it.hold)
        }
        val def = ExerciseLibrary.byId(de.libId)
        val ex = if (def == null) {
            FsExercise(
                libId = de.libId,
                name = de.name?.takeIf { it.isNotBlank() } ?: de.libId,
                // Only a user-created move's drafted muscle is its own; any other id is re-derived,
                // since a draft from an earlier build carries the Chest it was wrongly given.
                muscle = if (isCustomExerciseId(de.libId)) {
                    de.muscleCode?.let { MuscleGroup.fromCode(it) } ?: nonLibraryMuscle(de.libId)
                } else nonLibraryMuscle(de.libId),
                bodyweight = de.bodyweight ?: false,
                timed = de.timed ?: de.sets.any { it.hold.isNotBlank() },
                custom = true
            )
        } else {
            FsExercise(
                libId = def.id,
                name = def.name,
                muscle = def.muscle,
                bodyweight = de.bodyweight ?: (def.unit == ExerciseUnit.BODYWEIGHT),
                timed = de.timed ?: def.timed
            )
        }
        ex.copy(sets = sets.filter { ex.isLogged(it) })
    }

/**
 * Map a picked past-session template into the log (GYMAP-48): sets pre-filled from that session and
 * converted to the display unit, name/muscle/bodyweight re-derived from the library, and any move no
 * longer in the library dropped unless it is a custom move carrying its own name.
 */
internal fun List<FreestyleTemplateExercise>.toItems(weightUnit: WeightUnit): List<FsExercise> =
    distinctBy { it.libId }.mapNotNull { te ->
        val def = ExerciseLibrary.byId(te.libId)
        val ex = if (def == null) {
            // A custom move has no catalogue entry, so its shape comes off the rows it was logged on.
            // Assuming "weighted rep exercise" brought a bodyweight movement back with a weight field
            // and a timed hold back as reps.
            // Imported (`ext-…`) and seed-split ids land here too: they take their program slot's
            // muscle and unit, or no muscle at all, never a guessed Chest (audit 2026-09-26, 02).
            val name = te.customName?.takeIf { it.isNotBlank() } ?: return@mapNotNull null
            val unit = te.unitCode?.let { ExerciseUnit.fromCode(it) } ?: nonLibraryPlan(te.libId)?.unit
            FsExercise(
                libId = te.libId,
                name = name,
                muscle = te.muscleCode?.let { MuscleGroup.fromCode(it) } ?: nonLibraryMuscle(te.libId),
                bodyweight = unit == ExerciseUnit.BODYWEIGHT,
                timed = te.sets.any { it.durationSeconds != null },
                custom = true
            )
        } else {
            FsExercise(
                libId = def.id,
                name = def.name,
                muscle = def.muscle,
                bodyweight = def.unit == ExerciseUnit.BODYWEIGHT,
                timed = def.timed
            )
        }
        val sets = te.sets.map { s ->
            FsSet(
                weight = if (ex.bodyweight) "" else s.weightLb?.let { weightInputValue(it, weightUnit) } ?: "",
                reps = if (ex.timed) "" else s.reps.toString(),
                hold = s.durationSeconds?.let { holdText(it) } ?: ""
            )
        }
        ex.copy(sets = sets.filter { ex.isLogged(it) })
    }
