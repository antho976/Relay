package com.forge.app.program

/**
 * The one exercise-search rule, shared by the library picker and the freestyle browser.
 *
 * Search was a plain substring match on the name or the muscle, so the library's own shorthand
 * defeated it: "dumbbell" found nothing (26 names say "DB"), "romanian" missed Barbell RDL, "incline
 * bench" missed Incline DB Bench Press, "tricep extension" missed DB Overhead Tricep Ext. In the
 * freestyle browser an empty result offers to create the move, so a miss made a custom duplicate of a
 * library lift and split its history and records in two.
 *
 * Now every word of the query has to appear somewhere in the move's name, muscle or implement, each
 * word may be written the long way or the gym way (db, kb, rdl, ohp, ext...), and names that match
 * on their own sort ahead of moves found only by muscle or implement.
 */
object ExerciseSearch {

    /** Words that mean the same thing at the gym. Any member of a group finds the others. */
    private val GROUPS: List<Set<String>> = listOf(
        setOf("db", "dumbbell", "dumbbells"),
        setOf("kb", "kettlebell", "kettlebells"),
        setOf("bb", "barbell"),
        setOf("rdl", "romanian"),
        setOf("ohp", "overhead"),
        setOf("ext", "extension", "extensions"),
        setOf("tricep", "triceps"),
        setOf("bicep", "biceps"),
        setOf("skullcrusher", "skullcrushers", "skull"),
        setOf("pushdown", "pushdowns"),
        setOf("pulldown", "pulldowns"),
        setOf("pullup", "pullups"),
        setOf("chinup", "chinups"),
        setOf("pushup", "pushups"),
        setOf("ab", "abs", "core"),
        setOf("delt", "delts", "shoulder", "shoulders"),
        setOf("lat", "lats")
    )

    /** Implements worth searching by; benches and racks would match half the library. */
    private val IMPLEMENTS = setOf(
        Equipment.DUMBBELLS, Equipment.BARBELL, Equipment.KETTLEBELL, Equipment.CABLE,
        Equipment.SMITH_MACHINE, Equipment.EZ_BAR, Equipment.TRAP_BAR, Equipment.RESISTANCE_BAND
    )

    // Compiled once: [words] runs several times per exercise on every keystroke.
    private val SEPARATORS = Regex("[-.()/,]")
    private val UP = Regex("\\b(pull|chin|push) up")
    private val DOWN = Regex("\\b(pull|push) down")
    private val SKULL = Regex("\\bskull crusher")

    /** Lowercase words, with "-", "." and brackets read as spaces and "pull up" read as "pullup". */
    private fun words(text: String): List<String> {
        val spaced = text.lowercase()
            .replace(SEPARATORS, " ")
            .replace(UP, "$1up")
            .replace(DOWN, "$1down")
            .replace(SKULL, "skullcrusher")
        return spaced.split(' ').filter { it.isNotBlank() }
    }

    private fun alternatives(word: String): Set<String> =
        GROUPS.firstOrNull { word in it }?.minus(word) ?: emptySet()

    /**
     * True when every query word is in [haystack]: as typed, the start of a word (or inside one,
     * from three letters); as one of its gym synonyms, a whole word. A synonym matched as a prefix
     * made "lats" find every Lateral Raise through "lat".
     */
    private fun allWordsIn(query: List<String>, haystack: List<String>): Boolean =
        query.all { q ->
            haystack.any { it.startsWith(q) || (q.length >= 3 && it.contains(q)) } ||
                alternatives(q).any { alt -> alt in haystack }
        }

    /** 2 = the name alone matches, 1 = it matches with muscle and implement, 0 = no match. */
    fun score(def: ExerciseDef, query: String): Int {
        val q = words(query)
        if (q.isEmpty()) return 2
        val name = words(def.name)
        if (allWordsIn(q, name)) return 2
        val wider = name + words(def.muscle.displayName) +
            def.equipment.filter { it in IMPLEMENTS }.flatMap { words(it.display) }
        return if (allWordsIn(q, wider)) 1 else 0
    }

    /** [defs] matching [query], name matches first, otherwise in their given order. */
    fun filter(defs: List<ExerciseDef>, query: String): List<ExerciseDef> {
        if (query.isBlank()) return defs
        return defs.map { it to score(it, query) }
            .filter { it.second > 0 }
            .sortedByDescending { it.second }
            .map { it.first }
    }
}
