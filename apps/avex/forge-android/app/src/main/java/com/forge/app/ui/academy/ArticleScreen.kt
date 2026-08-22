package com.forge.app.ui.academy

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.InlineEmptyHint

/**
 * One Library article, on the reader both halves of the Academy share.
 *
 * What is left that is specific to an article: it closes with its sources, its kicker names the
 * level as well as the topic (an article runs from one minute to about thirty, so bracing the
 * reader is worth a word), and its "next" is the next piece on the same shelf rather than the next
 * step of a curriculum, because the Library has shelves and no curriculum.
 */
@Composable
fun ArticleScreen(
    onBack: () -> Unit,
    onOpenArticle: (String) -> Unit = {},
    viewModel: ArticleViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val article = state.article

    if (article == null) {
        // A retired or mistyped id resolves here rather than popping the back stack: a link from an
        // old coach reason should explain itself, not vanish (§12).
        Column(Modifier.fillMaxSize().padding(horizontal = 24.dp)) {
            Spacer(Modifier.height(96.dp))
            InlineEmptyHint("That article is no longer in the library", muted.copy(alpha = 0.65f))
        }
        return
    }

    ReaderScreen(
        onBack = onBack,
        cover = AcademyCovers.forId(article.id),
        kicker = "${article.topic.displayName} · ${article.level.displayName} · ${article.readMinutes} min",
        title = article.title,
        deck = article.deck,
        blocks = article.blocks,
        sources = article.sources,
        next = state.next,
        onReachedEnd = viewModel::onReachedEnd,
        onOpenNext = onOpenArticle
    )
}
