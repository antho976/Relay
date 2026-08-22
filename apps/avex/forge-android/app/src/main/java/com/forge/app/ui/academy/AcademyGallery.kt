@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.academy

import androidx.annotation.DrawableRes
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.R
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.ArticleRegistry
import com.forge.app.domain.academy.readMinutes
import com.forge.app.ui.common.bounceClick

/**
 * # The Academy gallery — plates and chapters (2026-08-20)
 *
 * ## What it replaced
 *
 * The 2026-08-16 rebuild put every piece in a bordered, filled card borrowed from the Home
 * experiment's `SurfaceKit`, with the title printed over the picture under a scrim. Three things
 * were wrong with it and Antho named all three: it still *"reads as blocks"*, there is *"no sense
 * of where to start"*, and the reading itself was *"plain"*.
 *
 * The blocks were the cards. Thirty-five identical filled rectangles with a hairline round each is
 * a wall whatever you print inside them, and §1 had already banned exactly that shape — a box is a
 * promise of a tap, and when every piece of content on a page is boxed the promise stops carrying
 * information. The fill also fought the photographs, which are near-black at the edges and want the
 * page, not a lighter slab, behind them.
 *
 * ## The form now
 *
 * **A plate and its caption, straight on the page.** No card, no border, no scrim. The picture is
 * clipped to a 16dp radius (§7's photo rule) and the words sit UNDER it in the page's own type. The
 * consequences are the point:
 *
 *  - the title is never printed over an unpredictable part of a photograph, so contrast is fixed
 *    rather than hoped for;
 *  - the art no longer has to be composed for its slot. One 3:4 master crops to either shape
 *    because nothing is written on top of it (`docs/ACADEMY_ART.md` is the manifest);
 *  - a piece with no cover yet is not a hole. It is a caption with no plate above it, which reads
 *    as an index line rather than as a card someone forgot to fill.
 *
 * **Two shapes, in a five-beat rhythm** ([isLeadSlot]): a full-width 3:2 lead, then two-up 3:4
 * posters until the next lead. Longer poster runs and rarer, bigger plates than the old
 * every-third-tile beat, which had settled into a checkerboard.
 *
 * **Read is a tone, not a word.** An opened piece prints its title in `muted` instead of `onBg` —
 * the visited-link convention. The meta line dropped the word "read" with it, so the state has one
 * home (§4.3) and the eye can find what it has not read without reading anything.
 */

/**
 * Cover art, looked up by lesson or article id.
 *
 * A lookup with holes in it rather than a field on the content model: art lands piecemeal, and a
 * piece without a cover renders its caption alone. So the page never looks half-finished mid-way
 * through an art pass, and the domain never has to know that a UI layer has pictures.
 *
 * Adding art is one line here plus the file. `docs/ACADEMY_ART.md` holds the outstanding 29, with
 * the subject and the frame each one is shot for.
 */
object AcademyCovers {
    private val byId = mapOf(
        "fundamentals.what_a_program_is" to R.drawable.cover_what_a_program_is,
        "fundamentals.sets_reps_rpe" to R.drawable.cover_sets_reps_rpe,
        "fundamentals.form_vs_load" to R.drawable.cover_form_vs_load,
        "fundamentals.rest_and_recovery" to R.drawable.cover_rest_and_recovery,
        "fundamentals.how_the_coach_works" to R.drawable.cover_how_the_coach_works,
        "fundamentals.what_readiness_means" to R.drawable.cover_what_readiness_means,
    )

    @DrawableRes
    fun forId(id: String): Int? = byId[id]

    /** True when the gallery can show this piece as a picture rather than as an index line. */
    fun has(id: String): Boolean = byId.containsKey(id)
}

/** One thing you can read. Lessons and articles are the same object to the gallery. */
sealed interface GalleryItem {
    val id: String
    val title: String
    val deck: String
    val minutes: Int
    val read: Boolean

    /** True when the coach flagged this as relevant and it has not been opened — the poke. */
    val forYou: Boolean

    /**
     * The mono word that tells the two kinds apart, per Antho: one page, labelled differently.
     * Null on the majority kind — the label flags the exception rather than repeating the default.
     */
    val kindLabel: String?

    data class LessonTile(val state: AcademyRegistry.LessonState) : GalleryItem {
        override val id get() = state.lesson.id
        override val title get() = state.lesson.title
        override val deck get() = state.lesson.summary
        override val minutes get() = state.lesson.blocks.readMinutes()
        override val read get() = state.opened
        override val forYou get() = state.isNew
        override val kindLabel: String? get() = null
    }

    data class ArticleTile(val state: ArticleRegistry.ArticleState) : GalleryItem {
        override val id get() = state.article.id
        override val title get() = state.article.title
        override val deck get() = state.article.deck
        override val minutes get() = state.article.readMinutes
        override val read get() = state.finished
        // An article has no coach moment behind it. It is open by design and always was, so nothing
        // can flag it as newly relevant — the Library's whole promise is that it never nags.
        override val forYou get() = false
        override val kindLabel get() = "Article"
    }
}

/**
 * One chapter of the gallery.
 *
 * [blurb] is the track's own authored line ("What training is made of. Read in order, start to
 * finish."), which until now was written but never rendered anywhere. It is the cheapest possible
 * answer to "where do I start", so it sits under the chapter header as the aside voice.
 *
 * [numerals] runs alongside [items] and is filled only for Fundamentals, the one track authored in
 * a reading order. Elsewhere every entry is null, because a number in front of an unordered list is
 * a rank nobody wrote. It is a parallel list rather than a computed index on purpose: the page's
 * opening pointer is lifted OUT of its chapter, and its neighbours have to keep the positions they
 * were authored with rather than closing the gap.
 */
data class GallerySection(
    val label: String,
    val items: List<GalleryItem>,
    val numerals: List<String?> = List(items.size) { null },
    val blurb: String? = null
)

/** Which shape a piece takes at [index] within its chapter. Every fifth slot opens on a lead. */
fun isLeadSlot(index: Int): Boolean = index % 5 == 0

/** Full width and landscape, or half width and upright. */
enum class PlateShape { LEAD, POSTER }

private val LEAD_ASPECT = 3f / 2f
private val POSTER_ASPECT = 3f / 4f

/** §7's photo rule: pictures clip to 16, never to a card radius. */
private val PlateCorner = RoundedCornerShape(16.dp)

/**
 * How far a caption sits from the picture it belongs to.
 *
 * 8dp, against the 44dp that separates one piece from the next (`ENTRY_GAP` in `AcademyScreen`).
 * The ratio is the whole point and it was wrong at first: with 12 above and 28 below, a serif title
 * under a photograph read as a HEADING for the photograph beneath it, because a heading normally
 * introduces what follows it. Antho: *"nothing makes me think that text here is for that one"*.
 * Nothing bound them because the gaps were close enough to be ambiguous, and the type said the
 * opposite of what the layout meant. Proximity is the only device available here — a box would be a
 * card, a rule would be a hairline (§1) — so it has to be unmistakable: caption tight to its plate,
 * generous air before the next piece.
 */
private val CAPTION_GAP = 8.dp

/**
 * A cover, greyscaled at render time.
 *
 * Forced to greyscale rather than trusted to be monochrome, so a colour asset can never quietly
 * break the one-accent rule (§5). [dissolve] fades the picture out into the page over its lower
 * half by masking its own alpha, which means it dissolves into whatever is actually behind it —
 * the page gradient, or pure black on AMOLED — instead of into a colour this composable guessed.
 * Only the reader hero uses it; a dissolve inside a rounded plate would just look like a smudge.
 */
@Composable
fun Plate(
    @DrawableRes cover: Int,
    aspect: Float,
    modifier: Modifier = Modifier,
    dissolve: Boolean = false
) {
    Image(
        painter = painterResource(cover),
        contentDescription = null,   // the caption beneath it already speaks (§14)
        contentScale = ContentScale.Crop,
        colorFilter = ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0f) }),
        modifier = modifier
            .fillMaxWidth()
            .aspectRatio(aspect)
            .then(
                if (dissolve) {
                    Modifier
                        .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                        .drawWithContent {
                            drawContent()
                            drawRect(
                                brush = Brush.verticalGradient(
                                    0.35f to Color.Black,
                                    1.0f to Color.Transparent
                                ),
                                blendMode = BlendMode.DstIn
                            )
                        }
                } else {
                    Modifier.clip(PlateCorner)
                }
            )
    )
}

/**
 * One piece of the gallery: its plate, then its caption.
 *
 * The whole column is the tap target — one target per piece, never a nested one (§14). There is no
 * border and no fill, so the press bounce is the affordance, which is what `bounceClick` is for.
 */
@Composable
fun PieceEntry(
    item: GalleryItem,
    shape: PlateShape,
    onBg: Color,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier,
    /** "03" inside an ordered chapter, null everywhere else. */
    numeral: String? = null,
    /**
     * True when a neighbour in this row HAS a cover, so this one holds the plate's space open even
     * though it has no picture yet.
     *
     * Without it the two captions in a mixed row start at different heights and the eye reads the
     * unplated one first, which in Fundamentals means reading 04 before 03. The reserved space is
     * empty air, never a filled placeholder: 29 of the 35 pieces are still waiting for art, and a
     * page of grey slabs would be the "wall of blocks" complaint again in a new costume.
     */
    reservePlate: Boolean = false,
    /** Names the chapter in the meta line. Only the page's opening pointer needs it. */
    chapter: String? = null,
    onClick: () -> Unit
) {
    val cover = AcademyCovers.forId(item.id)
    val lead = shape == PlateShape.LEAD

    Column(modifier.bounceClick { onClick() }) {
        if (cover != null) {
            Plate(cover, if (lead) LEAD_ASPECT else POSTER_ASPECT)
            Spacer(Modifier.height(CAPTION_GAP))
        } else if (reservePlate) {
            Box(Modifier.fillMaxWidth().aspectRatio(POSTER_ASPECT))
            Spacer(Modifier.height(CAPTION_GAP))
        }

        PieceMeta(item, numeral, chapter, muted, accent)
        Spacer(Modifier.height(6.dp))

        // No maxLines: a title is the whole pitch on a poster, so truncating one would leave the
        // piece saying nothing at all (§14).
        Text(
            item.title,
            style = if (lead) MaterialTheme.typography.headlineMedium
            else MaterialTheme.typography.headlineSmall,
            // Read is a tone, not a word: an opened piece steps back to muted, the way a visited
            // link does, and the meta line no longer has to say so.
            color = if (item.read) muted else onBg
        )

        // The deck rides the LEAD, and any unplated piece that is NOT holding a plate's space open.
        // On a plated poster the picture fills the slot, so the size difference means something: a
        // bigger plate is one that tells you more. On an unplated one in an unplated row, the deck
        // takes the space the picture would have had. Next to a plated neighbour it does not, since
        // the reserved space is already keeping the two captions on the same line.
        if (lead || (cover == null && !reservePlate)) {
            Spacer(Modifier.height(6.dp))
            Text(
                item.deck,
                style = MaterialTheme.typography.bodyMedium,
                color = if (item.read) muted.copy(alpha = 0.65f) else muted
            )
        }
    }
}

/**
 * The mono line above a title: what kind, how long, and where it sits in an ordered chapter.
 *
 * The accent dot is the only place the gallery spends colour, and it is painted for the exception
 * only (§8) — a coach moment that fired and has not been read. Every other piece reserves nothing:
 * the line simply starts at the gutter.
 */
@Composable
private fun PieceMeta(
    item: GalleryItem,
    numeral: String?,
    chapter: String?,
    muted: Color,
    accent: Color
) {
    val meta = buildList {
        if (item.forYou) add("For you")
        chapter?.let { add(it) }
        numeral?.let { add(it) }
        // "LESSON" is dropped: 31 of the 35 pieces are lessons, so the word appeared on almost
        // every piece and stopped being information. The minority carries the label instead.
        item.kindLabel?.let { add(it) }
        add("${item.minutes} min")
    }.joinToString(" · ").uppercase()

    Row(verticalAlignment = Alignment.CenterVertically) {
        if (item.forYou) {
            Box(Modifier.size(6.dp).clip(CircleShape).background(accent))
            Spacer(Modifier.width(7.dp))
        }
        Text(
            meta,
            style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.65f)
        )
    }
}

/**
 * A row of two posters, or one and the gap where its partner would be.
 *
 * Top-aligned rather than height-matched: the plates line up at the top edge, which is the line the
 * eye reads, and the captions below them are free to run to different depths. An odd piece keeps
 * its half width — a lone full-width entry mid-chapter would read as a lead and break the rhythm.
 */
@Composable
fun PosterRow(
    pair: List<GalleryItem>,
    onBg: Color,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier,
    numerals: List<String?> = List(pair.size) { null },
    onOpen: (GalleryItem) -> Unit
) {
    // One plated piece in the row makes the other hold a plate's worth of space, so both captions
    // sit on the same line and the pair reads left to right.
    val mixed = pair.any { AcademyCovers.has(it.id) }
    Row(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.Top
    ) {
        pair.forEachIndexed { i, item ->
            PieceEntry(
                item = item,
                shape = PlateShape.POSTER,
                onBg = onBg,
                muted = muted,
                accent = accent,
                modifier = Modifier.weight(1f),
                numeral = numerals.getOrNull(i),
                reservePlate = mixed
            ) { onOpen(item) }
        }
        if (pair.size == 1) Spacer(Modifier.weight(1f))
    }
}

/**
 * The page's opening pointer: the one piece to read next, set as type rather than as a picture.
 *
 * It deliberately carries NO plate. The piece it names also appears in its own chapter below, with
 * its own cover, and printing the same photograph twice within a screen and a half is the "one card
 * rendered twice" bug the 2026-08-16 pass already hit. Words at the top, pictures underneath: the
 * two registers do different jobs and neither repeats the other.
 */
@Composable
fun StartHereBlock(
    kicker: String,
    item: GalleryItem,
    chapter: String,
    numeral: String?,
    onBg: Color,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    Column(modifier.fillMaxWidth().bounceClick { onClick() }) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (item.forYou) {
                Box(Modifier.size(6.dp).clip(CircleShape).background(accent))
                Spacer(Modifier.width(7.dp))
            }
            Text(
                kicker.uppercase(),
                style = MaterialTheme.typography.labelLarge,
                color = muted
            )
        }
        Spacer(Modifier.height(10.dp))
        Text(item.title, style = MaterialTheme.typography.headlineMedium, color = onBg)
        Spacer(Modifier.height(8.dp))
        Text(
            item.deck,
            style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
            color = muted
        )
        Spacer(Modifier.height(12.dp))
        // Flowed, not a Row: at 200% font scale the meta line and the action do not fit on one
        // line, and a Row would break the action across two of them (§14).
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Text(
                listOfNotNull(chapter, numeral, "${item.minutes} min").joinToString(" · ").uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.65f)
            )
            // §2③: navigation is a mono accent action. Drawn, not separately clickable — the whole
            // block is the one tap target.
            Text("read →", style = MaterialTheme.typography.labelSmall, color = accent)
        }
    }
}

/** The chapter header: its name, and the line the track was authored with. */
@Composable
fun ChapterHeader(
    label: String,
    blurb: String?,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier
) {
    Column(modifier.fillMaxWidth()) {
        com.forge.app.ui.common.EditorialHeader(label = label, muted = muted, accent = accent)
        if (blurb != null) {
            Spacer(Modifier.height(8.dp))
            Text(
                blurb,
                style = MaterialTheme.typography.bodySmall.copy(fontStyle = FontStyle.Italic),
                color = muted.copy(alpha = 0.65f),
                modifier = Modifier.padding(end = 24.dp)
            )
        }
    }
}
