package com.forge.app.domain.academy

import com.forge.app.data.db.entities.ArticleEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Library's contract, audited here so it cannot quietly stop being true.
 *
 * The load-bearing one is [everyArticleIsSourced]. The Library's entire premise is that it is
 * research condensed rather than opinion typed confidently, and the only mechanical guard on that
 * is refusing to ship an article with an empty source list.
 */
class ArticleRegistryTest {

    private fun event(id: String, kind: ArticleEventKind, at: Long) =
        ArticleEvent(articleId = id, kind = kind.code, atMs = at)

    private val protein = "library.protein_intake"

    // ── The audit ──────────────────────────────────────────────────────────────

    @Test
    fun everyArticleIsSourced() {
        assertTrue(
            "unsourced articles: ${ArticleRegistry.unsourcedArticles().map { it.id }}",
            ArticleRegistry.unsourcedArticles().isEmpty()
        )
    }

    @Test
    fun everySourceIsCitable() {
        ArticleRegistry.articles.forEach { article ->
            article.sources.forEach { source ->
                assertTrue("${article.id} source needs authors", source.authors.isNotBlank())
                assertTrue("${article.id} source needs a title", source.title.isNotBlank())
                // A citation that cannot name its year is a citation nobody actually read.
                assertTrue(
                    "${article.id} source year looks wrong: ${source.year}",
                    source.year in 1950..2100
                )
            }
        }
    }

    @Test
    fun everyArticleHasIdentityAndContent() {
        ArticleRegistry.articles.forEach { a ->
            assertTrue("${a.id} needs a title", a.title.isNotBlank())
            assertTrue("${a.id} needs a deck", a.deck.isNotBlank())
            assertTrue("${a.id} needs blocks", a.blocks.isNotEmpty())
            assertTrue("${a.id} needs at least one topic", a.topics.isNotEmpty())
            // §11: a title is a title, not prose. No terminal period on either line.
            assertFalse("${a.id} title takes no period", a.title.endsWith("."))
            assertFalse("${a.id} deck takes no period", a.deck.endsWith("."))
        }
    }

    /**
     * The id grammar that lets a coach reason's `lessonId` slot carry an article without a parallel
     * field, and that lets `Routes.article` interpolate an id straight into a nav path.
     */
    @Test
    fun articleIdsAreNamespacedAndPathSafe() {
        val grammar = Regex("""^library\.[a-z0-9_]+$""")
        ArticleRegistry.articles.forEach { a ->
            assertTrue("${a.id} must match $grammar", grammar.matches(a.id))
        }
    }

    @Test
    fun articleIdsAreUniqueAndNeverCollideWithLessonIds() {
        val ids = ArticleRegistry.articles.map { it.id }
        assertEquals("duplicate article ids", ids.size, ids.distinct().size)
        val lessonIds = AcademyRegistry.lessons.map { it.id }.toSet()
        assertTrue(
            "article ids collide with lesson ids: ${ids.filter { it in lessonIds }}",
            ids.none { it in lessonIds }
        )
    }

    /** No lesson may wander into the Library's namespace, or [AcademyLink] becomes ambiguous. */
    @Test
    fun noLessonUsesTheLibraryNamespace() {
        val trespassers = AcademyRegistry.lessons.filter { it.id.startsWith(ArticleRegistry.ID_PREFIX) }
        assertTrue("lessons in the library namespace: ${trespassers.map { it.id }}", trespassers.isEmpty())
    }

    // ── Reading time ───────────────────────────────────────────────────────────

    @Test
    fun readTimeIsDerivedAndNeverZero() {
        ArticleRegistry.articles.forEach { a ->
            assertTrue("${a.id} must read as at least a minute", a.readMinutes >= 1)
            // The stated ceiling: past about thirty minutes it is a book, not a lesson.
            assertTrue("${a.id} runs long at ${a.readMinutes} min", a.readMinutes <= 30)
        }
    }

    // ── Derived state ──────────────────────────────────────────────────────────

    @Test
    fun stateIsRecomputedFromTheLedger() {
        val state = ArticleRegistry.stateOf(protein, listOf(event(protein, ArticleEventKind.OPENED, 10)))
        assertNotNull(state)
        assertTrue(state!!.opened)
        assertFalse(state.finished)
        assertEquals(10L, state.openedAtMs)
    }

    @Test
    fun finishingIsSeparateFromOpening() {
        val events = listOf(
            event(protein, ArticleEventKind.OPENED, 10),
            event(protein, ArticleEventKind.FINISHED, 90)
        )
        val state = ArticleRegistry.stateOf(protein, events)!!
        assertTrue(state.opened)
        assertTrue(state.finished)
    }

    @Test
    fun anUnreadArticleIsStillListed() {
        // Nothing is gated, so an empty ledger must still yield every article.
        val states = ArticleRegistry.stateFrom(emptyList())
        assertEquals(ArticleRegistry.articles.size, states.size)
        assertTrue(states.none { it.opened })
    }

    @Test
    fun unknownIdsInTheLedgerAreIgnoredNotFatal() {
        val states = ArticleRegistry.stateFrom(listOf(event("library.retired_thing", ArticleEventKind.OPENED, 1)))
        assertEquals(ArticleRegistry.articles.size, states.size)
        assertTrue(states.none { it.opened })
    }

    // ── The index ──────────────────────────────────────────────────────────────

    @Test
    fun onlyTopicsWithContentAreOffered() {
        val offered = ArticleRegistry.topicsWithContent()
        assertTrue("a topic with no articles was offered", offered.all { ArticleRegistry.byTopic(it).isNotEmpty() })
        // Order follows the enum, which is the index's reading order.
        assertEquals(offered, offered.sortedBy { it.ordinal })
    }

    @Test
    fun everyTopicOfferedIsReachableFromItsArticles() {
        ArticleRegistry.articles.forEach { a ->
            a.topics.forEach { topic ->
                assertTrue("${a.id} is missing from ${topic.code}", a in ArticleRegistry.byTopic(topic))
            }
        }
    }

    // ── The link resolver ──────────────────────────────────────────────────────

    @Test
    fun academyLinkResolvesBothHalves() {
        assertTrue(AcademyLink.resolve(protein) is AcademyLink.ToArticle)
        assertTrue(AcademyLink.resolve("coach.strength_on_a_cut") is AcademyLink.ToLesson)
    }

    @Test
    fun academyLinkSurvivesRetiredAndAbsentIds() {
        assertNull(AcademyLink.resolve(null))
        assertNull(AcademyLink.resolve("library.retired_thing"))
        assertNull(AcademyLink.resolve("coach.retired_thing"))
        assertNull(AcademyLink.titleOf("library.retired_thing"))
    }

    @Test
    fun academyLinkTitlesWhicheverHalfItPointsAt() {
        assertEquals(ArticleRegistry.article(protein)!!.title, AcademyLink.titleOf(protein))
        assertEquals(
            AcademyRegistry.lesson("coach.strength_on_a_cut")!!.title,
            AcademyLink.titleOf("coach.strength_on_a_cut")
        )
    }
}
