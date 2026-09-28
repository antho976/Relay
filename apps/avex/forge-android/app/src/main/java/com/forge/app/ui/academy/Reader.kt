@file:OptIn(
    androidx.compose.material3.ExperimentalMaterial3Api::class,
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class
)

package com.forge.app.ui.academy

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.academy.LessonBlock
import com.forge.app.domain.academy.Source
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.ForgeMotion

/**
 * # The reader: one lesson on its own page
 *
 * Rebuilt 2026-09-26 with the Academy. The lesson opens on its drawing assembling itself (the same
 * one the contents list shows in miniature, now with its labels), then its title, its one-line
 * answer and its length. The body keeps the reading voice set on 2026-08-20: `bodyLarge` prose, real
 * TalkBack headings, the takeaway as a serif pull-quote. A lesson built on research closes with its
 * sources as plain text, and every lesson but the last points at the next one.
 */

/** Where the reader goes next, when the track it is in has an order. */
data class NextPiece(
    val id: String,
    val title: String,
    /** "Next in Training", or "Next chapter" at a chapter's end. */
    val lead: String,
    val minutes: Int
)

@Composable
fun ReaderScreen(
    onBack: () -> Unit,
    /** Which lesson's drawing heads the page. */
    lessonId: String,
    /** The line under the title: chapter and length. */
    meta: String,
    title: String,
    /** The one-line answer, in the aside voice. */
    deck: String,
    blocks: List<LessonBlock>,
    modifier: Modifier = Modifier,
    examples: Map<String, String> = emptyMap(),
    sources: List<Source> = emptyList(),
    next: NextPiece? = null,
    onReachedEnd: () -> Unit = {},
    onOpenNext: (String) -> Unit = {}
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    val listState = rememberLazyListState()
    // The tail spacer is the last item, so seeing it means the whole piece has cleared the fold.
    // Recording from a real scroll rather than from a dismissal is what keeps "read" honest.
    val reachedEnd by remember {
        derivedStateOf { listState.layoutInfo.visibleItemsInfo.any { it.key == KEY_TAIL } }
    }
    LaunchedEffect(listState) {
        snapshotFlow { reachedEnd }.collect { if (it) onReachedEnd() }
    }

    // Transparent while the page sits at the top, so the drawing reads as the head of the page; solid
    // the moment anything scrolls under it. Without that, lesson text slid under the clock and the
    // battery in the status bar (Antho, 2026-09-26).
    val scrolled by remember {
        derivedStateOf { listState.firstVisibleItemIndex > 0 || listState.firstVisibleItemScrollOffset > 0 }
    }
    val barFill by animateColorAsState(
        if (scrolled) MaterialTheme.colorScheme.background else Color.Transparent,
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "reader_bar"
    )
    Scaffold(
        topBar = {
            // §4.6: chrome only. The piece names itself in its own serif hero below.
            ForgeTopBar(onBack = onBack, modifier = Modifier.background(barFill))
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            state = listState,
            // The head pads itself below the bar rather than the list, so the drawing scrolls up
            // under the transparent bar instead of stopping short of it.
            modifier = modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = inner.calculateBottomPadding())
        ) {
            item("head") {
                Column(Modifier.fillMaxWidth().padding(horizontal = GUTTER)) {
                    Spacer(Modifier.height(inner.calculateTopPadding()))
                    LessonSketch(lessonId = lessonId)
                    Spacer(Modifier.height(24.dp))
                    // §11: a serif title takes no terminal period.
                    Text(
                        title,
                        style = MaterialTheme.typography.headlineLarge,
                        color = onBg,
                        modifier = Modifier.semantics { heading() }
                    )
                    Spacer(Modifier.height(12.dp))
                    Text(
                        deck,
                        style = MaterialTheme.typography.bodyLarge.copy(fontStyle = FontStyle.Italic),
                        color = muted
                    )
                    Spacer(Modifier.height(12.dp))
                    Text(meta, style = MaterialTheme.typography.labelMedium, color = muted.copy(alpha = 0.65f))
                    Spacer(Modifier.height(32.dp))
                }
            }

            // One item per block, so opening a lesson composes only the blocks on screen instead of
            // every paragraph, figure and explorer in the navigation frame.
            itemsIndexed(
                blocks,
                key = { i, _ -> "b$i" },
                contentType = { _, block -> block::class }
            ) { _, block ->
                Box(Modifier.fillMaxWidth().padding(horizontal = GUTTER)) {
                    LessonBlockView(block, examples)
                }
            }

            if (sources.isNotEmpty()) {
                item("sources") {
                    Column(Modifier.fillMaxWidth().padding(horizontal = GUTTER)) {
                        SourceList(sources)
                    }
                }
            }

            if (next != null) {
                item("next") {
                    Column(Modifier.fillMaxWidth().padding(horizontal = GUTTER)) {
                        Spacer(Modifier.height(40.dp))
                        NextBlock(next, onBg, muted) { onOpenNext(next.id) }
                    }
                }
            }

            item(KEY_TAIL) { Spacer(Modifier.height(72.dp)) }
        }
    }
}

/**
 * The way out of the end of a lesson: the next one, drawn the way the contents list draws it, so
 * the reader knows what they are tapping into.
 */
@Composable
private fun NextBlock(
    next: NextPiece,
    onBg: Color,
    muted: Color,
    onClick: () -> Unit
) {
    Column(Modifier.fillMaxWidth()) {
        Text(
            next.lead,
            style = MaterialTheme.typography.titleSmall,
            color = muted,
            modifier = Modifier.semantics { heading() }
        )
        Spacer(Modifier.height(12.dp))
        Row(
            Modifier.fillMaxWidth().bounceClick { onClick() }.padding(vertical = 4.dp),
            verticalAlignment = Alignment.Top
        ) {
            ThumbTile(next.id, Modifier.width(96.dp).padding(top = 2.dp))
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f)) {
                Text(next.title, style = MaterialTheme.typography.headlineSmall, color = onBg)
                Spacer(Modifier.height(8.dp))
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Text(
                        "${next.minutes} min",
                        style = MaterialTheme.typography.labelMedium,
                        color = muted.copy(alpha = 0.65f)
                    )
                    ReadCapsule()
                }
            }
        }
    }
}

/**
 * The "read" cue on a whole-card tap target: a small filled capsule drawn inside the card, so the
 * card reads as a button without a second, nested target (the card's own tap opens the lesson).
 * It replaced the accent `read →` text link (2026-09-27).
 */
@Composable
internal fun ReadCapsule() {
    Box(
        Modifier
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .clearAndSetSemantics { }
            .padding(horizontal = 12.dp, vertical = 4.dp)
    ) {
        Text("Read", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onBackground)
    }
}

private const val KEY_TAIL = "reader-tail"

private val GUTTER = 24.dp
