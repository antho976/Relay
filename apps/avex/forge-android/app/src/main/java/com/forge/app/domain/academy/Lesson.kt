package com.forge.app.domain.academy

/**
 * A lesson's content model (Coach v3 A2 / plan Mechanics M5).
 *
 * Structured blocks, not markdown: the app has no markdown renderer and 33 short lessons don't
 * justify adding one. Blocks also give the design system real control over each voice, and let an
 * [LessonBlock.Example] interpolate the reader's OWN numbers — which is the entire point of the
 * later "your numbers" track.
 *
 * Content ships in-app and offline like everything else. Lessons cite nothing and link nowhere
 * (there is no internet permission); the sources behind each one live in `docs/ACADEMY_LESSONS.md`
 * for the author, not the reader.
 */
data class Lesson(
    /** Stable id, e.g. "coach.strength_on_a_cut" — also what `reason.lessonId` points at. */
    val id: String,
    val track: LessonTrack,
    /** One short line, sentence case, no terminal period (a title, not prose). */
    val title: String,
    /** The one-line answer, shown on the card before the lesson is opened. */
    val summary: String,
    /** What opens this lesson — see [LessonUnlock]. */
    val unlock: LessonUnlock,
    val blocks: List<LessonBlock>
)

/**
 * What opens a lesson, written for the reader rather than for the ledger.
 *
 * These used to be one string naming the internal moment — "Your first placement-driven
 * prescription", "The first time a personal volume cap changes an allocation". Accurate, and
 * useless: a locked row told you nothing you could act on, and half of them were vocabulary the
 * lesson itself exists to teach (DESIGN §11, translate the machine).
 *
 * The split that fixes it is [byYou]. Some lessons are gated on something the reader can go and do
 * today, and those get an imperative [label] — "Log a set". The rest are gated on the coach having
 * seen enough to make a move, and pretending otherwise would be a lie dressed as a task; those name
 * the moment and let [detail] say what has to accumulate first. A reader should never be left
 * guessing which kind they are looking at.
 */
data class LessonUnlock(
    /** The locked row's line: an imperative when [byYou], otherwise the moment ("When …"). */
    val label: String,
    /** One line of what to actually do, or what has to happen first. Never restates [label]. */
    val detail: String,
    /** True when the reader can trigger it today; false when it is the coach's move. */
    val byYou: Boolean
)

/**
 * The five tracks the plan groups lessons into. Order here is the reading order the Academy shows,
 * which is NOT a course index: only [FUNDAMENTALS] is sequential (it doubles as the cold-start
 * directive), and everything else unlocks the first time its coach moment fires. The tracks exist
 * to group, not to march through — `docs/ACADEMY_LESSONS.md`, "just-in-time, not curriculum-first".
 */
enum class LessonTrack(val code: String, val displayName: String, val blurb: String) {
    FUNDAMENTALS("fundamentals", "Fundamentals", "What training is made of. Read in order, start to finish."),
    COACH("coach", "How the coach works", "What it decides, and how to overrule any of it."),
    PROGRAMMING("programming", "Programming", "Blocks, phases, and the numbers it learns about you."),
    SIGNALS("signals", "Signals", "What your body is telling it, and how much that counts."),
    ENGINE("engine", "Conditioning", "Cardio in service of lifting, never instead of it.")
}

sealed interface LessonBlock {
    /** A short section anchor inside a lesson. */
    data class Heading(val text: String) : LessonBlock

    data class Paragraph(val text: String) : LessonBlock

    data class Bullets(val items: List<String>) : LessonBlock

    /** The one thing to take away — rendered as the lesson's emphasis, one per lesson at most. */
    data class Callout(val text: String) : LessonBlock

    /**
     * A slot for the reader's own data, resolved at render time by the surface that has it
     * ([key] is looked up in a simple map). Falls back to [fallback] when the number isn't
     * available yet, so a lesson is never blocked on data.
     */
    data class Example(val key: String, val label: String, val fallback: String) : LessonBlock
}

/**
 * Reading time for a run of blocks, derived from the prose rather than authored.
 *
 * Authored minutes drift the moment anyone edits a paragraph and nobody remembers to update the
 * number. 200 words per minute is the usual estimate for adult non-fiction; it rounds to the nearest
 * minute with a floor of one, so the shortest piece reads "1 MIN" rather than "0 MIN".
 *
 * Lifted out of `Article` (2026-08-16) when the gallery started showing lessons and articles side by
 * side: two pieces of content next to each other cannot state their length by two different rules.
 * `Article.readMinutes` delegates here, so its output is unchanged.
 */
fun List<LessonBlock>.readMinutes(): Int {
    fun String.words(): Int = split(' ', '\n').count { it.isNotBlank() }
    val words = sumOf { block ->
        when (block) {
            is LessonBlock.Heading -> block.text.words()
            is LessonBlock.Paragraph -> block.text.words()
            is LessonBlock.Bullets -> block.items.sumOf { it.words() }
            is LessonBlock.Callout -> block.text.words()
            // An Example renders a number and its label, not prose worth timing.
            is LessonBlock.Example -> 0
        }
    }
    return ((words + WORDS_PER_MINUTE / 2) / WORDS_PER_MINUTE).coerceAtLeast(1)
}

private const val WORDS_PER_MINUTE = 200

/** What a [com.forge.app.data.db.entities.LessonEvent] records. */
enum class LessonEventKind(val code: String) {
    /** The lesson's coach/app moment fired for the first time — it now exists for this user. */
    UNLOCKED("unlocked"),

    /** The user opened it. */
    OPENED("opened"),

    /** The user reached the end of it. */
    COMPLETED("completed");

    companion object {
        fun fromCode(code: String): LessonEventKind? = entries.firstOrNull { it.code == code }
    }
}
