package com.forge.app.domain.academy

/**
 * A Library article — the Academy's open half.
 *
 * A deliberate sibling of [Lesson] rather than an extension of it, because the two disagree on
 * every axis that matters. A lesson is gated on a coach moment, is 1–3 minutes, and exists to
 * explain something the coach just did to you; an article is open from install, runs anywhere from
 * one minute to about thirty, and exists because the subject is worth knowing whether or not the
 * coach ever touches it. Folding articles into [Lesson] would force [Lesson.unlock] to be a lie on
 * every row.
 *
 * What they DO share is the renderer: articles are the same [LessonBlock] list, so both halves of
 * the Academy read in one voice and there is no second block vocabulary to maintain.
 *
 * Ids are namespaced `library.*`, which is what lets a coach reason's `lessonId` slot carry an
 * article without a parallel field on every call site — see [AcademyLink]. `ArticleRegistryTest`
 * enforces both the namespace and the absence of collisions with lesson ids.
 */
data class Article(
    /** Stable id, always `library.<slug>`. Also what a coach reason points at. */
    val id: String,
    val title: String,
    /** The one-line answer, shown on the row before the article is opened. Sentence case, no period. */
    val deck: String,
    /** Reading difficulty. A LABEL, never a filter and never a gate — everything is always open. */
    val level: ArticleLevel,
    /** The shelf it files under. First entry is the primary one the index groups by. */
    val topics: List<ArticleTopic>,
    val blocks: List<LessonBlock>,
    /**
     * What the article is built on. Never empty: an article without sources is an opinion, and the
     * whole premise of the Library is that it is not one. Rendered as plain text at the end, since
     * the app holds no INTERNET permission and never will.
     */
    val sources: List<Source>
) {
    val topic: ArticleTopic get() = topics.first()

    /**
     * Reading time, derived from the prose rather than authored.
     *
     * Authored minutes drift the moment anyone edits a paragraph, and nobody ever remembers to
     * update the number. 200 words per minute is the usual estimate for adult non-fiction reading;
     * it rounds to the nearest minute with a floor of one, so the shortest article still reads
     * "1 MIN" rather than "0 MIN".
     */
    // The rule itself lives on `List<LessonBlock>.readMinutes()` in Lesson.kt (2026-08-16): the
    // gallery shows lessons and articles side by side, and two neighbouring cards cannot state their
    // length by two different rules. Output is identical to the version that lived here.
    val readMinutes: Int get() = blocks.readMinutes()
}

/**
 * How hard an article is to read, as an indicator beside its length.
 *
 * Explicitly NOT a filter and explicitly NOT a ladder. Filters answer "what is this about", which
 * is a question a reader can actually have; "how hard is it" is a question they can only answer
 * after reading, and turning it into a gate would contradict the one rule the Library has, which is
 * that anyone can read anything whenever they want. The label exists so a reader can brace
 * themselves, not so the app can decide for them.
 */
enum class ArticleLevel(val code: String, val displayName: String) {
    /** Assumes nothing. The idea itself, and why it matters. */
    BASICS("basics", "Basics"),

    /** Assumes you train. The numbers and what to do differently on Monday. */
    APPLIED("applied", "Applied"),

    /** Mechanism, disagreement in the literature, and where the evidence runs out. */
    DEEP("deep", "Deep");

    companion object {
        fun fromCode(code: String): ArticleLevel? = entries.firstOrNull { it.code == code }
    }
}

/**
 * The Library's shelves, in index order.
 *
 * Eight of them against four seed articles is deliberate: the index renders only topics that
 * actually hold something ([ArticleRegistry.topicsWithContent]), so the shelf appears the day its
 * first article does. §12's rule is to design a section at its emptiest realistic state, and six
 * empty shelves on install is exactly the promise-nothing-keeps that rule exists to prevent.
 *
 * The split follows how readers actually look for training material rather than how a coach thinks
 * about it: "Training" as one shelf tested badly because hypertrophy, programming and technique are
 * three unrelated questions that happen to share a gym.
 */
enum class ArticleTopic(val code: String, val displayName: String) {
    HYPERTROPHY("hypertrophy", "Hypertrophy"),
    STRENGTH("strength", "Strength"),
    PROGRAMMING("programming", "Programming"),
    TECHNIQUE("technique", "Technique"),
    NUTRITION("nutrition", "Nutrition"),
    RECOVERY("recovery", "Recovery"),
    CONDITIONING("conditioning", "Conditioning"),
    MINDSET("mindset", "Mindset");

    companion object {
        fun fromCode(code: String): ArticleTopic? = entries.firstOrNull { it.code == code }
    }
}

/**
 * One reference behind an article.
 *
 * Structured rather than a pre-formatted string so the renderer owns the typography, and so the
 * authoring pass can be checked: a citation that cannot name its year is a citation nobody read.
 * [journal] is null for books and for the rare case where the source is a textbook chapter.
 */
data class Source(
    /** "Refalo M, et al." — surname, initial, and `et al.` past two authors. */
    val authors: String,
    val title: String,
    val journal: String?,
    val year: Int
)

/** What an [com.forge.app.data.db.entities.ArticleEvent] records. */
enum class ArticleEventKind(val code: String) {
    /** The reader opened it. */
    OPENED("opened"),

    /** The reader reached the end of it. */
    FINISHED("finished");

    companion object {
        fun fromCode(code: String): ArticleEventKind? = entries.firstOrNull { it.code == code }
    }
}
