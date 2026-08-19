package com.forge.app.ui.academy

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.domain.academy.ArticleRegistry
import com.forge.app.domain.academy.ArticleTopic
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.forgeItemMotion

/**
 * The Library index (DESIGN §3, list archetype) — every article, always open.
 *
 * A [LazyListScope] extension rather than a screen because it shares the Academy hub's scroll and
 * its one serif hero: two independently scrolling panes behind a pill row would read as two screens
 * wearing one name.
 *
 * List-archetype discipline holds throughout. Search first, trim rows, a light stagger and nothing
 * else. No charts, no figures, no entrance cascade, and no serif of its own.
 */
internal fun LazyListScope.libraryPane(
    state: LibraryViewModel.UiState,
    viewModel: LibraryViewModel,
    onOpenArticle: (String) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    item("library-search") {
        Spacer(Modifier.height(16.dp))
        // §13: interactive, therefore bordered. The placeholder may dim below the muted floor —
        // it is a ghost affordance rather than content (§5).
        OutlinedTextField(
            value = state.query,
            onValueChange = viewModel::onQueryChange,
            placeholder = { Text("Search the library", color = muted.copy(alpha = 0.5f)) },
            singleLine = true,
            shape = RoundedCornerShape(50),
            colors = OutlinedTextFieldDefaults.colors(
                focusedBorderColor = accent,
                unfocusedBorderColor = outline.copy(alpha = 0.35f),
                focusedTextColor = onBg,
                unfocusedTextColor = onBg
            ),
            modifier = Modifier.fillMaxWidth()
        )
    }

    if (state.topics.isNotEmpty()) {
        item("library-topics") {
            Spacer(Modifier.height(12.dp))
            // Scrollable rather than wrapped: the shelf list grows with the catalogue, and a pill
            // row that reflows to three lines stops reading as one control. Only topics that hold
            // an article are offered, so this row is never a menu of empty rooms.
            Row(
                Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                SegmentPill(
                    text = "All",
                    selected = state.topic == null,
                    onClick = { viewModel.onTopicSelected(null) },
                    accent = accent, onBg = onBg, muted = muted, outline = outline
                )
                state.topics.forEach { topic ->
                    SegmentPill(
                        text = topic.displayName,
                        selected = state.topic == topic,
                        onClick = { viewModel.onTopicSelected(topic) },
                        accent = accent, onBg = onBg, muted = muted, outline = outline
                    )
                }
            }
        }
    }

    val rows = state.visible
    if (rows.isEmpty()) {
        item("library-empty") {
            Spacer(Modifier.height(24.dp))
            // §12: neither a search miss nor an empty catalogue has a zero-SHAPE to draw, which is
            // the last-resort case InlineEmptyHint exists for. The two say different things, since
            // one is the reader's doing and the other is ours.
            InlineEmptyHint(
                if (state.filtered) "Nothing here matches that" else "The library is still being written",
                muted.copy(alpha = 0.65f)
            )
        }
    } else {
        items(rows, key = { it.article.id }) { row ->
            // §9: lists take a light stagger, never the overview's entrance cascade.
            ArticleRow(
                state = row,
                // §4.3, one home: inside a topic filter the shelf name is already on the pill, so
                // repeating it on every row would be the same fact twice on one screen.
                showTopic = state.topic == null,
                onBg = onBg, muted = muted,
                onClick = { onOpenArticle(row.article.id) },
                modifier = forgeItemMotion()
            )
            Spacer(Modifier.height(6.dp))
        }
    }

    item("library-tail") { Spacer(Modifier.height(56.dp)) }
}

/**
 * One article row: title, deck, and a mono meta line.
 *
 * The level rides in the meta line rather than as right-meta, which keeps §8's rule intact (right
 * meta is a count or a reading, never a state word) and lets the three facts a reader actually
 * sorts on sit together in one voice: what shelf, how hard, how long.
 */
@Composable
private fun ArticleRow(
    state: ArticleRegistry.ArticleState,
    showTopic: Boolean,
    onBg: Color,
    muted: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val article = state.article
    val meta = buildList {
        if (showTopic) add(article.topic.displayName)
        add(article.level.displayName)
        add("${article.readMinutes} min")
        if (state.finished) add("read")
    }.joinToString(" · ")

    Column(
        modifier
            .fillMaxWidth()
            // §2③: the whole row is the tap target, never a control nested inside it.
            .clickableLabeled("Read: ${article.title}") { onClick() }
            .padding(vertical = 10.dp)
    ) {
        // §14: no maxLines on content. A long title wraps rather than truncating.
        Text(article.title, style = MaterialTheme.typography.titleSmall, color = onBg)
        Spacer(Modifier.height(3.dp))
        Text(article.deck, style = MaterialTheme.typography.bodySmall, color = muted)
        Spacer(Modifier.height(6.dp))
        Text(
            meta.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.65f)
        )
    }
}

/** The Library's own eyebrow, so the shared hero can name what the current lens holds. */
internal fun libraryEyebrow(state: LibraryViewModel.UiState): String {
    val shown = state.visible.size
    return if (state.filtered) "$shown OF ${state.articles.size} ARTICLES"
    else "${state.articles.size} ARTICLES"
}
