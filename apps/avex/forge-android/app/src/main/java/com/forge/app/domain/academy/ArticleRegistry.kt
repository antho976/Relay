package com.forge.app.domain.academy

import com.forge.app.data.db.entities.ArticleEvent

/**
 * The Library catalogue — every article that ships, and the reader's standing in it.
 *
 * Mirrors [AcademyRegistry] on purpose, including the rule that matters most: **state is
 * recomputed, never mutated**. [stateFrom] folds the append-only `article_event` ledger the same
 * way `AcademyRegistry.stateFrom` folds `lesson_event`, so read state cannot drift or double-count.
 *
 * Where it deliberately differs is unlocking, because there isn't any. Every article is readable
 * from install by anyone. There is no gate, no order, no percentage and no score — the plan's ban
 * on gamifying the Academy ("learning is not gamified engagement bait") applies here at least as
 * hard as it does to lessons, because a library that scores you is a course wearing a disguise.
 */
object ArticleRegistry {

    /** The id namespace every article shares. Guarantees no collision with a lesson id. */
    const val ID_PREFIX = "library."

    /** Every article that currently ships, in authoring order within each topic. */
    val articles: List<Article> =
        LibraryHypertrophy.all + LibraryRecovery.all + LibraryNutrition.all

    fun article(id: String): Article? = articles.firstOrNull { it.id == id }

    fun byTopic(topic: ArticleTopic): List<Article> = articles.filter { topic in it.topics }

    /**
     * Topics that actually hold something, in enum order — the index's filter row.
     *
     * A shelf appears the day its first article does. The alternative (render all eight always)
     * opens the Library with six empty rows on install, which §12 names outright: design the
     * section at its emptiest realistic state, not its fullest.
     */
    fun topicsWithContent(): List<ArticleTopic> =
        ArticleTopic.entries.filter { topic -> articles.any { topic in it.topics } }

    /** One article's state for this reader, derived from the ledger. */
    data class ArticleState(
        val article: Article,
        val opened: Boolean,
        val finished: Boolean,
        val openedAtMs: Long?
    )

    /**
     * Fold the ledger into per-article state. Unknown ids are ignored rather than dropped from
     * history, so an article can be renamed or retired without corrupting the record.
     */
    fun stateFrom(events: List<ArticleEvent>): List<ArticleState> {
        val byArticle = events.groupBy { it.articleId }
        return articles.map { article ->
            val own = byArticle[article.id].orEmpty()
            ArticleState(
                article = article,
                opened = own.any { it.kind == ArticleEventKind.OPENED.code },
                finished = own.any { it.kind == ArticleEventKind.FINISHED.code },
                openedAtMs = own.filter { it.kind == ArticleEventKind.OPENED.code }.minOfOrNull { it.atMs }
            )
        }
    }

    fun stateOf(articleId: String, events: List<ArticleEvent>): ArticleState? =
        stateFrom(events).firstOrNull { it.article.id == articleId }

    /**
     * The audit `ArticleRegistryTest` runs: an article with no sources fails.
     *
     * This is the Library's equivalent of [AcademyRegistry.orphanLessons] and it guards the same
     * kind of rot. A lesson without a moment is content in search of a coach; an article without a
     * source is an opinion in search of a citation, and the entire premise here is that it is not
     * one.
     */
    fun unsourcedArticles(): List<Article> = articles.filter { it.sources.isEmpty() }
}

/**
 * Resolves an Academy link to whichever half of the Academy holds it.
 *
 * Coach reasons, directives, signals and conditioning prescriptions all carry a nullable
 * `lessonId`. Rather than growing a parallel `articleId` on every one of those (five domain types
 * and a database column, all to express the same idea), the id namespace does the work: lessons are
 * `fundamentals.*` / `coach.*` / `programming.*` / `signals.*` / `engine.*`, articles are
 * `library.*`. An existing link slot can therefore point at an article with no schema change and no
 * call-site churn, which is what makes the Library part of the system rather than a shelf beside it.
 *
 * `ArticleRegistryTest` asserts the two id spaces stay disjoint, so this can never become ambiguous.
 */
sealed interface AcademyLink {
    data class ToLesson(val lesson: Lesson) : AcademyLink
    data class ToArticle(val article: Article) : AcademyLink

    companion object {
        /** Null when the id names nothing that ships — retired content must not crash a reason. */
        fun resolve(id: String?): AcademyLink? {
            if (id == null) return null
            return if (id.startsWith(ArticleRegistry.ID_PREFIX)) {
                ArticleRegistry.article(id)?.let(::ToArticle)
            } else {
                AcademyRegistry.lesson(id)?.let(::ToLesson)
            }
        }

        /** The title to show on a link, whichever half it points at. */
        fun titleOf(id: String?): String? = when (val link = resolve(id)) {
            is ToLesson -> link.lesson.title
            is ToArticle -> link.article.title
            null -> null
        }
    }
}
