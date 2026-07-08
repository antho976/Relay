package com.forge.app.data.importer

import com.forge.app.program.ExerciseLibrary

/**
 * Maps an exercise NAME from another app's export to a Forge catalogue id (#GYMAP-17).
 *
 * Other apps name the same movement a dozen ways — "Bench Press (Barbell)" (Strong), "Bench Press
 * (Barbell)" (Hevy), "Barbell Bench Press" (FitNotes), "BB Bench" (a spreadsheet). We normalise both
 * the incoming name and every library name to a canonical TOKEN SET (lowercase, punctuation and
 * equipment-parentheses flattened, abbreviations expanded — "db"→"dumbbell", "bb"→"barbell",
 * "rdl"→"romanian deadlift") and match on set equality, then on token overlap (Jaccard) above a
 * threshold, with a small curated table for names that don't normalise cleanly ("military press",
 * bare "squat"/"deadlift").
 *
 * A miss is NOT data loss: an unmatched name is still imported under its original text (via the
 * logged exercise's swapped name), it just isn't linked to a catalogue movement for stats.
 */
object ExerciseNameMatcher {

    /** Token synonyms/abbreviation expansions applied to BOTH sides so they meet in the middle. */
    private val TOKEN_SYNONYMS: Map<String, List<String>> = mapOf(
        "db" to listOf("dumbbell"), "dbs" to listOf("dumbbell"), "dumbbells" to listOf("dumbbell"),
        "bb" to listOf("barbell"), "barbells" to listOf("barbell"),
        "kb" to listOf("kettlebell"), "kettlebells" to listOf("kettlebell"),
        "bw" to listOf("bodyweight"),
        "ohp" to listOf("overhead", "press"),
        "rdl" to listOf("romanian", "deadlift"),
        "sldl" to listOf("stiff", "leg", "deadlift"),
        "ext" to listOf("extension"), "extensions" to listOf("extension"),
        "raises" to listOf("raise"), "curls" to listOf("curl"), "rows" to listOf("row"),
        "presses" to listOf("press"), "squats" to listOf("squat"), "flyes" to listOf("fly"),
        "flys" to listOf("fly"), "dips" to listOf("dip"), "lunges" to listOf("lunge"),
        "tricep" to listOf("triceps"), "bicep" to listOf("biceps"),
        "pulldowns" to listOf("pulldown"), "pushdowns" to listOf("pushdown"),
        "1arm" to listOf("single", "arm"), "1leg" to listOf("single", "leg")
    )

    /** Curated direct mappings for cross-app names that token-overlap can't safely resolve. Written
     *  in natural word order for readability, then run through [canonicalKey] so lookups (which key on
     *  the sorted token set) always hit — a key left in prose order would silently never match. */
    private val CURATED: Map<String, String> = mapOf(
        "barbell squat" to "back-squat",
        "squat" to "back-squat",
        "deadlift" to "conventional-deadlift",
        "barbell deadlift" to "conventional-deadlift",
        "military press" to "barbell-overhead-press",
        "overhead press" to "barbell-overhead-press",
        "shoulder press" to "barbell-overhead-press",
        "romanian deadlift" to "barbell-rdl",
        "chinup" to "chin-up",
        "pullup" to "pull-up",
        "pushup" to "push-up",
        "flat barbell bench press" to "barbell-bench-press",
        "barbell hip thrust" to "barbell-hip-thrust",
        "calf raise" to "standing-calf-raise",
        "seated leg curl" to "leg-curl",
        "crunch" to "cable-crunch"
    ).mapKeys { (name, _) -> canonicalKey(name) }

    /** Library id keyed by canonical token-set string, built once. */
    private val libraryByKey: Map<String, String> by lazy {
        // First writer wins so the earlier (default/best) library entry keeps an ambiguous key.
        val map = LinkedHashMap<String, String>()
        ExerciseLibrary.all.forEach { def -> map.putIfAbsent(canonicalKey(def.name), def.id) }
        map
    }

    /** Library (id, token-set) pairs for the fuzzy pass. */
    private val libraryTokens: List<Pair<String, Set<String>>> by lazy {
        ExerciseLibrary.all.map { it.id to tokenize(it.name) }
    }

    /** The catalogue id for [name], or null when nothing is a confident match (kept under its own name). */
    fun match(name: String): String? {
        // Tokenize once and reuse for both the exact key and the fuzzy pass.
        val tokens = tokenize(name)
        if (tokens.isEmpty()) return null
        val key = tokens.sorted().joinToString(" ")
        CURATED[key]?.let { return it }
        libraryByKey[key]?.let { return it }

        // Fuzzy: highest Jaccard over the library, accepted only above a safe floor.
        var bestId: String? = null
        var bestScore = 0.0
        for ((id, libTokens) in libraryTokens) {
            val inter = tokens.count { it in libTokens }
            if (inter == 0) continue
            val union = (tokens.size + libTokens.size - inter).toDouble()
            val score = inter / union
            if (score > bestScore) { bestScore = score; bestId = id }
        }
        return if (bestScore >= JACCARD_FLOOR) bestId else null
    }

    /** Canonical, order-independent key: sorted distinct tokens joined by spaces. */
    private fun canonicalKey(name: String): String = tokenize(name).sorted().joinToString(" ")

    /** Normalise to a distinct token set: lowercase, flatten punctuation/parens, expand synonyms. */
    private fun tokenize(name: String): Set<String> {
        val cleaned = name.lowercase()
            .replace('(', ' ').replace(')', ' ')
            .map { if (it.isLetterOrDigit()) it else ' ' }
            .joinToString("")
        return cleaned.split(' ')
            .asSequence()
            .filter { it.isNotBlank() }
            .flatMap { token -> (TOKEN_SYNONYMS[token] ?: listOf(token)).asSequence() }
            .toSet()
    }

    private const val JACCARD_FLOOR = 0.66
}
