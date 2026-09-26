package com.forge.app.domain.academy

/**
 * A lesson's content model (Coach v3 A2 / plan Mechanics M5).
 *
 * Structured blocks, not markdown: the app has no markdown renderer and 12 short lessons don't
 * justify adding one. Blocks also give the design system real control over each voice, and let an
 * [LessonBlock.Example] interpolate the reader's OWN numbers — which is the entire point of the
 * later "your numbers" track.
 *
 * Content ships in-app and offline like everything else. Lessons link nowhere (there is no
 * internet permission); a lesson built on research carries its [Lesson.sources] as plain text.
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
    val blocks: List<LessonBlock>,
    /**
     * The research a lesson leans on, printed as plain text at the end of the reader. Empty for the
     * lessons that explain what the coach does rather than what the literature says.
     */
    val sources: List<Source> = emptyList()
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
 * The Academy's three chapters, in the order the page shows them (2026-09-26).
 *
 * Nine sections of 35 pieces became three chapters of 12 lessons. [TRAINING] is the only chapter
 * written to be read in order, and it doubles as the cold-start curriculum.
 */
enum class LessonTrack(val code: String, val displayName: String) {
    TRAINING("training", "Training"),
    COACH("coach", "Your coach"),
    CARDIO("cardio", "Cardio")
}

/**
 * One reference behind a lesson.
 *
 * Structured rather than a pre-formatted string so the reader owns the typography. [journal] is
 * null for books. Rendered as plain text, never a link: the app holds no INTERNET permission.
 */
data class Source(
    /** "Refalo M, et al.": surname, initial, and `et al.` past two authors. */
    val authors: String,
    val title: String,
    val journal: String?,
    val year: Int
)

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

    /**
     * A figure: an animated or interactive diagram of the idea the paragraph around it explains,
     * drawn by the UI layer from [key] (`AcademyFigures`). [caption] is the one line under it, and
     * the only part of the figure the domain owns, so it goes through the same voice rules as prose.
     */
    data class Figure(val key: String, val caption: String) : LessonBlock
}

/**
 * Reading time for a run of blocks, derived from the prose rather than authored.
 *
 * Authored minutes drift the moment anyone edits a paragraph and nobody remembers to update the
 * number. 200 words per minute is the usual estimate for adult non-fiction; it rounds to the nearest
 * minute with a floor of one, so the shortest piece reads "1 MIN" rather than "0 MIN".
 *
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
            // A figure takes about as long to take in as a short paragraph.
            is LessonBlock.Figure -> 40 + block.caption.words()
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
