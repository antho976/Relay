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
        "1arm" to listOf("single", "arm"), "1leg" to listOf("single", "leg"),
        "one" to listOf("single")
    )

    /** Curated direct mappings for cross-app names that token-overlap can't safely resolve. Written
     *  in natural word order for readability, then run through [canonicalKey] so lookups (which key on
     *  the sorted token set) always hit — a key left in prose order would silently never match. */
    private val CURATED_NAMES: Map<String, String> = mapOf(
        "barbell squat" to "back-squat",
        "squat" to "back-squat",
        "deadlift" to "conventional-deadlift",
        "barbell deadlift" to "conventional-deadlift",
        "military press" to "barbell-overhead-press",
        "overhead press" to "barbell-overhead-press",
        "romanian deadlift" to "barbell-rdl",
        "chinup" to "chin-up",
        "pullup" to "pull-up",
        "pushup" to "push-up",
        "flat barbell bench press" to "barbell-bench-press",
        "barbell hip thrust" to "barbell-hip-thrust",
        "calf raise" to "standing-calf-raise",
        // "seated leg curl" is deliberately absent: the plate-count "leg-curl" is not it, and the
        // library's own WEIGHT-unit "Seated Leg Curl" resolves by exact name.
        // Bare "Bench Press" / "Incline Bench Press" are the barbell lifts in every app that exports
        // them without an equipment suffix (FitNotes, most spreadsheets), and the fuzzy pass cannot
        // reach them: the barbell library names score 0.667 and 0.5 against these. Curating them the
        // way "squat" and "deadlift" already are stops the equipment guard below from leaving the two
        // most-logged lifts in the gym unmatched.
        "bench press" to "barbell-bench-press",
        "incline bench press" to "incline-barbell-bench"
        // "crunch" is deliberately absent: the catalogue's only crunches are the weighted cable and
        // high-pulley machine variants, and a plain bodyweight crunch — the most common ab entry in
        // every gym app — is not either of them. It imports under its own name instead.
        // "shoulder press" is absent for the same reason: unqualified, it is usually a dumbbell press
        // in the apps that export it that way, so mapping it to the barbell lift asserted equipment
        // the export never stated.
    )

    /** [CURATED_NAMES] keyed by [canonicalKey]. */
    private val CURATED: Map<String, String> = CURATED_NAMES.mapKeys { (name, _) -> canonicalKey(name) }
        // Never a plate-count station: those are owner-only, and an imported lb value filed under one
        // reads back as a plate count.
        .filterValues { ExerciseLibrary.byId(it)?.curatedOnly != true }

    /**
     * The movements an import may resolve to. The owner-only plate-count stations ([ExerciseDef.curatedOnly])
     * are excluded: the pickers already hide them, and a barbell-era lb value filed under one is shown
     * back as a plate count. Such a name stays unmatched and imports under its own label.
     */
    private val matchable by lazy { ExerciseLibrary.all.filter { !it.curatedOnly } }

    /** Library id keyed by canonical token-set string, built once. */
    private val libraryByKey: Map<String, String> by lazy {
        // First writer wins so the earlier (default/best) library entry keeps an ambiguous key.
        val map = LinkedHashMap<String, String>()
        matchable.forEach { def -> map.putIfAbsent(canonicalKey(def.name), def.id) }
        map
    }

    /** Library (id, token-set) pairs for the fuzzy pass. */
    private val libraryTokens: List<Pair<String, Set<String>>> by lazy {
        matchable.map { it.id to tokenize(it.name) }
    }

    // ── Pre-guard resolution, for the duplicate guard only (see [legacyMatch]) ──────────────────

    private val LEGACY_TOKEN_SYNONYMS: Map<String, List<String>> by lazy { TOKEN_SYNONYMS - "one" }

    /** The curated table as it was: unfiltered, and still mapping "seated leg curl" to leg-curl. */
    private val LEGACY_CURATED: Map<String, String> by lazy {
        (CURATED_NAMES + ("seated leg curl" to "leg-curl"))
            .mapKeys { (name, _) -> canonicalKey(name, LEGACY_TOKEN_SYNONYMS) }
    }

    private val legacyLibraryByKey: Map<String, String> by lazy {
        val map = LinkedHashMap<String, String>()
        ExerciseLibrary.all.forEach { def -> map.putIfAbsent(canonicalKey(def.name, LEGACY_TOKEN_SYNONYMS), def.id) }
        map
    }

    private val legacyLibraryTokens: List<Pair<String, Set<String>>> by lazy {
        ExerciseLibrary.all.map { it.id to tokenize(it.name, LEGACY_TOKEN_SYNONYMS) }
    }

    /** The catalogue id for [name], or null when nothing is a confident match (kept under its own name). */
    fun match(name: String): String? =
        resolve(tokenize(name), CURATED, libraryByKey, libraryTokens, guardModifiers = true)

    /**
     * What builds before the plate-count exclusion and the [MOVEMENT_MODIFIERS] guard resolved [name]
     * to: the whole library, the old curated table ("seated leg curl" to the plate-count leg-curl), no
     * "one" synonym, and no modifier guard. Rows earlier imports wrote are stored under this id, so the
     * duplicate guard keys on it as well; it is for recognising those rows ONLY, never for a stored id.
     */
    fun legacyMatch(name: String): String? =
        resolve(tokenize(name, LEGACY_TOKEN_SYNONYMS), LEGACY_CURATED, legacyLibraryByKey, legacyLibraryTokens, guardModifiers = false)

    private fun resolve(
        tokens: Set<String>,
        curated: Map<String, String>,
        byKey: Map<String, String>,
        library: List<Pair<String, Set<String>>>,
        guardModifiers: Boolean
    ): String? {
        if (tokens.isEmpty()) return null
        val key = tokens.sorted().joinToString(" ")
        curated[key]?.let { return it }
        byKey[key]?.let { return it }

        // Fuzzy: highest Jaccard over the library, accepted only above a safe floor, never across an
        // equipment mismatch, and never on a tie.
        var bestId: String? = null
        var bestScore = 0.0
        var bestTies = 0
        for ((id, libTokens) in library) {
            val inter = tokens.count { it in libTokens }
            if (inter == 0) continue
            if (assertsEquipment(tokens, libTokens)) continue
            if (guardModifiers && dropsModifier(tokens, libTokens)) continue
            // "Lat Pulldown (Cable)" against "Lat Pulldown": the source is the library name plus an
            // equipment qualifier and nothing else, so it is an exact match once the qualifier is
            // dropped — not a 0.667 fuzzy one that the floor below would reject. Extra tokens that
            // are NOT equipment ("Reverse Fly (Dumbbell)" over "DB Fly") change the movement and get
            // no such treatment.
            val equipmentQualifiedOnly =
                inter == libTokens.size && tokens.all { it in libTokens || it in EQUIPMENT_TOKENS }
            val union = (tokens.size + libTokens.size - inter).toDouble()
            val score = if (equipmentQualifiedOnly) 1.0 else inter / union
            when {
                score > bestScore -> { bestScore = score; bestId = id; bestTies = 1 }
                score == bestScore && id != bestId -> bestTies++
            }
        }
        // A tie is genuine ambiguity — declaration order is not evidence, and picking by it is how
        // "Bicep Curl" landed on a machine variant. Import under the original name instead.
        if (bestTies > 1) return null
        return if (bestScore >= JACCARD_FLOOR) bestId else null
    }

    /**
     * True when the library name names equipment the source name never did — "Bench Press" against
     * "DB Bench Press", "Reverse Fly (Dumbbell)" against "DB Fly". Token overlap alone rates those
     * 0.667, so without this guard the matcher INVENTS the equipment and files three years of 245 lb
     * barbell benching under a dumbbell lift (or, in the Fly case, moves rear-delt volume into CHEST).
     * The reverse direction is fine: a source name may be more specific than the library's.
     */
    private fun assertsEquipment(sourceTokens: Set<String>, libTokens: Set<String>): Boolean =
        libTokens.any { it in EQUIPMENT_TOKENS && it !in sourceTokens }

    /**
     * True when the source name carries a word that makes it a different lift ("Decline Bench Press",
     * "Sumo Deadlift") and the library name does not. Jaccard alone rates one extra word at 0.75, so
     * without this the decline press merged into the flat bench's PRs, e1RM and prefill history.
     */
    private fun dropsModifier(sourceTokens: Set<String>, libTokens: Set<String>): Boolean =
        sourceTokens.any { it in MOVEMENT_MODIFIERS && it !in libTokens }

    /** Canonical, order-independent key: sorted distinct tokens joined by spaces. */
    private fun canonicalKey(name: String, synonyms: Map<String, List<String>> = TOKEN_SYNONYMS): String =
        tokenize(name, synonyms).sorted().joinToString(" ")

    /** Normalise to a distinct token set: lowercase, flatten punctuation/parens, expand synonyms. */
    private fun tokenize(name: String, synonyms: Map<String, List<String>> = TOKEN_SYNONYMS): Set<String> {
        val cleaned = name.lowercase()
            .replace('(', ' ').replace(')', ' ')
            .map { if (it.isLetterOrDigit()) it else ' ' }
            .joinToString("")
        return cleaned.split(' ')
            .asSequence()
            .filter { it.isNotBlank() }
            .flatMap { token -> (synonyms[token] ?: listOf(token)).asSequence() }
            .toSet()
    }

    /** Equipment words that must not be inferred — only carried across from the source name. */
    private val EQUIPMENT_TOKENS = setOf(
        "barbell", "dumbbell", "cable", "machine", "bodyweight", "smith", "kettlebell", "band"
    )

    /** Words that change WHICH lift a name is, as opposed to equipment or plural noise. */
    private val MOVEMENT_MODIFIERS = setOf(
        "decline", "incline", "reverse", "sumo", "deficit", "pause", "paused", "front", "single",
        "unilateral", "close", "wide", "narrow", "hammer"
    )

    /**
     * Above 0.667. A two-token source name shares both tokens with a three-token library name at
     * exactly 2/3, which is the shape "<equipment> + <the user's name>" — the single biggest source
     * of wrong matches. Genuine three-of-four overlaps score 0.75 and still pass.
     */
    private const val JACCARD_FLOOR = 0.72
}
