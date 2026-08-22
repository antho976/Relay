@file:OptIn(
    androidx.compose.material3.ExperimentalMaterial3Api::class,
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class
)

package com.forge.app.ui.academy

import androidx.compose.foundation.background
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
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.academy.LessonBlock
import com.forge.app.domain.academy.Source
import com.forge.app.ui.common.bounceClick

/**
 * # The reader — one page for both halves of the Academy (2026-08-20)
 *
 * A lesson used to open in a `ModalBottomSheet` and an article as a full screen: the same block
 * vocabulary, the same voice, two different readers. The sheet lost every time. It capped the
 * reading at a sheet's height, it could not carry a cover, it recorded completion from a DISMISSAL
 * (so a bounce counted as a read, which corrupts the only signal the ledger keeps), and it made the
 * Library feel like the serious half of a page that was supposed to be one library.
 *
 * So both are this screen now, and the differences that remain are the ones that are actually real:
 * an article closes with its sources, a lesson can interpolate the reader's own numbers, and only a
 * lesson inside an ordered track has a next piece to point at.
 *
 * ## The hero
 *
 * The cover bleeds the full width and runs up under the transparent top bar, then **dissolves into
 * the page** rather than ending on an edge (see [Plate]'s `dissolve`). Nothing is printed over the
 * picture, so the type below it is on the page's own ground at full contrast, and the photograph is
 * never asked to be a background and a subject at once. A piece with no cover yet simply starts at
 * its kicker: the page has one fewer beat, not a hole where a beat should be.
 *
 * ## The measure
 *
 * Prose is `bodyLarge` (16sp), not the 14sp the sheet used. That single step is most of what Antho
 * meant by *"the reading itself is plain"* — 14sp is a row label's size, and a page of it reads as
 * an interface rather than as something written. Everything else follows: paragraph air at 14dp, a
 * real TalkBack heading per section anchor, and the takeaway set as a serif pull-quote instead of
 * boxed in an accent wash (a box around passive content is §1's central ban, and the callout was
 * quietly breaking it).
 */

/** Where the reader goes next, when the track it is in has an order. */
data class NextPiece(
    val id: String,
    val title: String,
    /** The mono line above it: "Next in Fundamentals" where an order exists, "More in ..." where
     *  the chapter is a shelf rather than a sequence. */
    val lead: String,
    val minutes: Int
)

@Composable
fun ReaderScreen(
    onBack: () -> Unit,
    /** Cover drawable, or null while this piece is still waiting for art. */
    cover: Int?,
    /** The mono line above the title: chapter, position, length. */
    kicker: String,
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
    val accent = MaterialTheme.colorScheme.primary
    val ground = MaterialTheme.colorScheme.background

    val listState = rememberLazyListState()
    // The tail spacer is the last item, so seeing it means the whole piece has cleared the fold.
    // Recording from a real scroll rather than from a dismissal is what keeps "read" honest.
    val reachedEnd by remember {
        derivedStateOf { listState.layoutInfo.visibleItemsInfo.any { it.key == KEY_TAIL } }
    }
    LaunchedEffect(listState) {
        snapshotFlow { reachedEnd }.collect { if (it) onReachedEnd() }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6: chrome only. The piece names itself in its own serif hero below.
                title = {},
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                            tint = if (cover != null) onBg else muted
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            state = listState,
            // No top inset: the cover scrolls up under the transparent bar on purpose. Every other
            // item pads itself to the 24dp gutter, since a full-bleed picture cannot live inside
            // one content padding with the prose.
            modifier = modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = inner.calculateBottomPadding())
        ) {
            if (cover != null) {
                item("cover") {
                    Box(Modifier.fillMaxWidth()) {
                        Plate(cover, aspect = COVER_ASPECT, dissolve = true)
                        // The back arrow sits on the picture, and a photograph cannot promise to be
                        // dark in its top corner. A short wash under the chrome keeps it legible
                        // whatever the art does; gradients interpolate freely (§5).
                        Box(
                            Modifier
                                .fillMaxWidth()
                                .height(CHROME_WASH)
                                .background(
                                    Brush.verticalGradient(
                                        0f to ground,
                                        1f to Color.Transparent
                                    )
                                )
                        )
                    }
                }
            } else {
                item("nocover") { Spacer(Modifier.height(inner.calculateTopPadding())) }
            }

            item("head") {
                Column(Modifier.fillMaxWidth().padding(horizontal = GUTTER)) {
                    Spacer(Modifier.height(if (cover != null) 4.dp else 12.dp))
                    Text(
                        kicker.uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = muted.copy(alpha = 0.65f)
                    )
                    Spacer(Modifier.height(10.dp))
                    // §11: a serif title takes no terminal period.
                    Text(title, style = MaterialTheme.typography.headlineLarge, color = onBg)
                    Spacer(Modifier.height(12.dp))
                    // §11: italic is the aside voice. The deck is the piece's one-line answer, so
                    // it sits above the body rather than repeating inside it.
                    Text(
                        deck,
                        style = MaterialTheme.typography.bodyLarge.copy(fontStyle = FontStyle.Italic),
                        color = muted
                    )
                    Spacer(Modifier.height(28.dp))
                }
            }

            item("body") {
                Column(Modifier.fillMaxWidth().padding(horizontal = GUTTER)) {
                    BlockBody(blocks, examples)
                    if (sources.isNotEmpty()) SourceList(sources)
                }
            }

            if (next != null) {
                item("next") {
                    Column(Modifier.fillMaxWidth().padding(horizontal = GUTTER)) {
                        Spacer(Modifier.height(40.dp))
                        NextBlock(next, onBg, muted, accent) { onOpenNext(next.id) }
                    }
                }
            }

            item(KEY_TAIL) { Spacer(Modifier.height(72.dp)) }
        }
    }
}

/**
 * The link out of the end of a piece.
 *
 * §4.2 allows a link only beside a trim of its destination, which is exactly what this is: the next
 * lesson's own title and length, not a bare "next". It appears only where a real order exists, so
 * the reader is never pointed at a piece the app picked arbitrarily.
 */
@Composable
private fun NextBlock(
    next: NextPiece,
    onBg: Color,
    muted: Color,
    accent: Color,
    onClick: () -> Unit
) {
    Column(Modifier.fillMaxWidth().bounceClick { onClick() }) {
        Text(
            next.lead.uppercase(),
            style = MaterialTheme.typography.labelLarge,
            color = muted
        )
        Spacer(Modifier.height(10.dp))
        Text(next.title, style = MaterialTheme.typography.headlineSmall, color = onBg)
        Spacer(Modifier.height(10.dp))
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Text(
                "${next.minutes} min".uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.65f)
            )
            Text("read →", style = MaterialTheme.typography.labelSmall, color = accent)
        }
    }
}

private const val KEY_TAIL = "reader-tail"

/** 4:3, deep enough to carry a subject and still leave the title above the fold. */
private const val COVER_ASPECT = 4f / 3f

private val GUTTER = 24.dp

/** How far the chrome wash reaches down the cover — status bar plus the top bar's own height. */
private val CHROME_WASH = 132.dp
