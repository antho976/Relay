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
    /** What makes this lesson appear, in the user's terms. */
    val unlockedBy: String,
    val blocks: List<LessonBlock>
)

enum class LessonTrack(val code: String, val displayName: String) {
    FUNDAMENTALS("fundamentals", "Fundamentals"),
    COACH("coach", "How the coach works"),
    PROGRAMMING("programming", "Programming"),
    SIGNALS("signals", "Signals"),
    ENGINE("engine", "Conditioning")
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
