package com.forge.app.ui.programbuilder

import com.forge.app.data.db.entities.ProgramDay
import com.forge.app.data.db.entities.ProgramSlot

/** One exercise in the in-memory builder. [uid] is a stable UI/list key; [libId] is the persisted
 *  [com.forge.app.program.ExerciseLibrary] id. */
data class BuilderExercise(
    val uid: String,
    val libId: String,
    val name: String,
    val muscle: String,
    val sets: Int,
    val reps: String
)

/** One day in the in-memory builder. [key] is the stable program day id used for persistence + history
 *  attribution (kept across renames/reorders); [uid] is the transient list key. [word] is the original
 *  spine word from the loaded plan, kept as a fallback so editing a day whose archetype isn't one of
 *  the builder's [DAY_TYPES] doesn't blank its word on save. */
data class BuilderDay(
    val uid: String,
    val key: String,
    val name: String,
    val archetype: String,
    val accentHex: String,
    val exercises: List<BuilderExercise>,
    val word: String = ""
)

/** Day types — drive the spine word + the derived subtitle/warmup (via the repository's archetypeMeta). */
val DAY_TYPES: List<Triple<String, String, String>> = listOf(
    Triple("fb", "Full body", "FULL"),
    Triple("push", "Push", "PUSH"),
    Triple("pull", "Pull", "PULL"),
    Triple("legs", "Legs", "LEGS"),
    Triple("upper", "Upper", "UPPER"),
    Triple("lower", "Lower", "LOWER"),
    Triple("arms", "Arms", "ARMS")
)

/** Default day identity colours, cycled as days are added. */
val DAY_ACCENTS: List<String> = listOf("#E85D4A", "#D4A017", "#5B9279", "#7B6CB5", "#4A90D9", "#C0567A")

/** Max characters for a day name — keeps the big home/day-card title from overflowing the screen. */
const val MAX_DAY_NAME = 24

/** Normalize a stored archetype to its base type — stored keys can carry a split suffix (e.g.
 *  "upper-a"/"lower-b"), matching how the repository's archetypeMeta strips it. */
internal fun baseArchetype(archetype: String): String = archetype.substringBefore('-').lowercase()

internal fun wordFor(archetype: String): String =
    DAY_TYPES.firstOrNull { it.first == baseArchetype(archetype) }?.third ?: ""

internal fun typeLabel(archetype: String): String =
    DAY_TYPES.firstOrNull { it.first == baseArchetype(archetype) }?.second ?: "Full body"

/** Pure mapping from the edited model to persistable rows (positions = list order). Testable. */
fun List<BuilderDay>.toEntities(): Pair<List<ProgramDay>, List<ProgramSlot>> {
    val days = ArrayList<ProgramDay>(size)
    val slots = ArrayList<ProgramSlot>()
    forEachIndexed { i, d ->
        days += ProgramDay(
            id = d.key, position = i, name = d.name.trim().take(MAX_DAY_NAME).ifBlank { "Day ${i + 1}" },
            // Derive the word from the type, but keep the loaded plan's original word for an archetype
            // outside DAY_TYPES (wordFor returns "") so editing never blanks a day's spine word.
            word = wordFor(d.archetype).ifBlank { d.word }, accentHex = d.accentHex, archetype = d.archetype
        )
        d.exercises.forEachIndexed { j, e ->
            slots += ProgramSlot(
                id = "${d.key}-$j", dayId = d.key, position = j,
                exerciseLibId = e.libId, sets = e.sets, reps = e.reps
            )
        }
    }
    return days to slots
}

/** Move the element at [from] to index [to], returning a new list (drag-reorder helper). */
fun <T> List<T>.moved(from: Int, to: Int): List<T> {
    if (from == to || from !in indices || to !in indices) return this
    val m = toMutableList()
    m.add(to, m.removeAt(from))
    return m
}
