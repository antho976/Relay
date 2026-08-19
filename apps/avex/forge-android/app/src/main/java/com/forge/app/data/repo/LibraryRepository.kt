package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.ArticleEventDao
import com.forge.app.data.db.entities.ArticleEvent
import com.forge.app.domain.academy.Article
import com.forge.app.domain.academy.ArticleEventKind
import com.forge.app.domain.academy.ArticleRegistry
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The Library's data layer: an append-only ledger in, derived state out.
 *
 * Deliberately much thinner than [AcademyRepository], and the missing half is the point. The
 * Academy repository carries `syncCoachMoments`, because a lesson has to be earned before it
 * exists. Nothing here has to be earned, so there is no unlock path, no idempotent moment firing,
 * and no coach coupling. The only writes are the two the reader causes, and both are recorded once.
 */
@Singleton
class LibraryRepository @Inject constructor(
    private val articleEventDao: ArticleEventDao,
    private val clock: Clock
) {

    fun observeStates(): Flow<List<ArticleRegistry.ArticleState>> =
        articleEventDao.observeAll().map { ArticleRegistry.stateFrom(it) }

    suspend fun states(): List<ArticleRegistry.ArticleState> =
        ArticleRegistry.stateFrom(articleEventDao.all())

    suspend fun stateOf(articleId: String): ArticleRegistry.ArticleState? =
        ArticleRegistry.stateOf(articleId, articleEventDao.all())

    fun article(id: String): Article? = ArticleRegistry.article(id)

    /** The reader opened it. Recorded once; re-reads do not append. */
    suspend fun markOpened(articleId: String) = record(articleId, ArticleEventKind.OPENED)

    /**
     * The reader reached the end. Recorded once.
     *
     * "Reached the end" means the last block actually scrolled into view, not that the screen was
     * dismissed. A lesson sheet can honestly treat closing as finishing because a lesson is three
     * paragraphs on one screen; an article cannot, and counting a bounce as a read would quietly
     * corrupt the only signal the Library keeps.
     */
    suspend fun markFinished(articleId: String) = record(articleId, ArticleEventKind.FINISHED)

    private suspend fun record(articleId: String, kind: ArticleEventKind) {
        if (ArticleRegistry.article(articleId) == null) return
        if (articleEventDao.has(articleId, kind.code)) return
        articleEventDao.insert(
            ArticleEvent(articleId = articleId, kind = kind.code, atMs = clock.nowMs())
        )
    }
}
