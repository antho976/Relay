package com.forge.app.ui.academy

import androidx.annotation.DrawableRes
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.R
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.ArticleRegistry
import com.forge.app.domain.academy.readMinutes
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.experiment.CardShape
import com.forge.app.ui.experiment.SurfacePalette

/**
 * # The Academy gallery (2026-08-16)
 *
 * ## What it replaced, and why
 *
 * The hub was a list of five tracks, each showing a dot rail and an "n OF m", leading to a track
 * screen, leading to a lesson. Antho's verdict: *"too crowded and behind 50 sub menus, and the worst
 * thing is it feels like achievement, not a hub to knowledge."*
 *
 * Three separate causes, all fixed here:
 *
 * 1. **It was 87% locked.** 27 of 31 lessons were gated on coach moments. A mostly-locked inventory
 *    can only read as an achievement tree. The gate is gone (see [AcademyViewModel.UiState]).
 * 2. **It reported progress twice per track** — a dot rail AND a count — and progress was the loudest
 *    thing on a page that is supposed to be about reading.
 * 3. **Three levels to reach a lesson.** The track screen is deleted; tracks are section headers now.
 *
 * ## The form
 *
 * Grouped sections, two tiles per row, vertical throughout. Antho asked for "between B and C" of the
 * three sketches — B was per-track horizontal shelves, C a single column of large cards. Two-up
 * tiles under a section header is the literal midpoint, and it avoids the horizontal scroll he had
 * already rejected on Home's goals.
 *
 * A tile is deliberately typographic. There is no cover art in this app yet, and inventing a colour
 * block per card would be decoration standing in for an image. The tile is sized so a cover can drop
 * in above the title later without the layout moving.
 */

/**
 * How many flagged lessons the top of the page shows. ONE.
 *
 * A shelf of them is a queue, and a queue is the achievement feeling coming back in by the side door
 * — it would sit at the top of the page counting down at you. Antho asked for "a little poke saying
 * hey, this is relevant for you right now", singular. The bell count and the tab badge already carry
 * the number when more than one has fired, and each of those lessons still wears its FOR YOU mark
 * down in its own track.
 */
const val FOR_YOU_CAP = 1

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
 * One labelled group of the gallery.
 *
 * [featured] renders its single item full width instead of as a two-up tile. Only the poke uses it,
 * and it exists because the poke must not look like the shelf below it: with both at tile width, the
 * flagged lesson appeared as two identical cards stacked four inches apart, which reads as a
 * rendering bug rather than as a highlight.
 */
data class GallerySection(
    val label: String,
    val items: List<GalleryItem>,
    val featured: Boolean = false
)

/**
 * Cover art, looked up by lesson or article id.
 *
 * A lookup with holes in it rather than a field on the content model: art lands piecemeal, and a
 * piece without a cover renders a purely typographic tile that sits happily beside the shot ones. So
 * the page never looks half-finished mid-way through an art pass, and the domain never has to know
 * that a UI layer has pictures.
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
}

/**
 * Which shape a tile takes at [index] within its section: every third slot is wide, the two between
 * are posters.
 *
 * The rhythm is what stops an all-pictures gallery flattening back into a wall. Give every tile a
 * cover at the same size and they are all equally loud again, which is the "just feels like blocks"
 * problem in a new costume — the size variation has to keep doing the work that the presence or
 * absence of a cover used to do.
 *
 * It also lets the art be matched to the slot rather than the other way round: detailed compositions
 * (a page of handwriting) go wide, where they can be read; simple graphic ones (chalk marks, a bar in
 * a rack) go poster, where they survive being 165dp across.
 */
fun isWideSlot(index: Int): Boolean = index % 3 == 0

/**
 * One tile, in the two shapes the mosaic uses.
 *
 * ## Why it looks like this
 *
 * The first gallery was eighteen identically-weighted rectangles — Antho: *"it just feels like
 * blocks"*. A gallery works by having focal points and rhythm, and images are only the cheapest way
 * to buy both. This version buys them with **scale rhythm** ([wide] vs poster, see [isWideSlot]) and
 * with **the title as the art** — serif, large, sitting on the picture rather than beside it.
 *
 * The ghosted serif numeral that did that job before the art arrived is **gone** (Antho, 2026-08-16).
 * It was buying exactly the per-tile identity a photograph now buys, and a numeral plus a photo plus
 * a serif title is three things competing inside a 165dp box.
 *
 * ## Where the darkness has to be
 *
 * The two shapes place their text differently, so their scrims run in different directions, and so
 * the art is shot to match:
 *
 * - **wide** — text down the start edge, so a HORIZONTAL scrim and a subject shot on the END side.
 * - **poster** — text along the foot, so a bottom-up VERTICAL scrim and a subject shot in the upper
 *   two thirds.
 *
 * Covers are forced to greyscale at render time rather than trusted to be monochrome, so a colour
 * asset can never quietly break the one-accent rule.
 */
@Composable
fun GalleryTile(
    item: GalleryItem,
    palette: SurfacePalette,
    onBg: Color,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier,
    /** Full-width landscape when true, half-width poster when false. */
    wide: Boolean = false,
    /** False inside the poke's own section, where the header already says FOR YOU. */
    showForYouMark: Boolean = true,
    onClick: () -> Unit
) {
    val cover = AcademyCovers.forId(item.id)
    val meta = buildList {
        // "LESSON" is dropped: 31 of the 35 pieces are lessons, so the word appeared on almost every
        // tile and stopped being information. The minority carries the label instead — flag the
        // exception, never the default. Which still answers "one page, labelled differently".
        item.kindLabel?.let { add(it) }
        add("${item.minutes} min")
        if (item.read) add("read")
    }.joinToString(" · ")

    Box(
        modifier
            .clip(CardShape)
            .background(palette.card)
            .border(1.dp, palette.hairline, CardShape)
            .bounceClick { onClick() }
            // heightIn, never height: the title has to be able to grow at 200% (§14). These are the
            // shapes at default scale, not a cage.
            .heightIn(min = if (wide) 176.dp else 210.dp)
    ) {
        if (cover != null) {
            Image(
                painter = painterResource(cover),
                contentDescription = null,   // the title over it already speaks (§14)
                contentScale = ContentScale.Crop,
                colorFilter = ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0f) }),
                modifier = Modifier.matchParentSize()
            )
            Box(
                Modifier
                    .matchParentSize()
                    .background(
                        if (wide) {
                            Brush.horizontalGradient(
                                0.0f to palette.card,
                                0.62f to palette.card.copy(alpha = 0.7f),
                                1.0f to palette.card.copy(alpha = 0.25f)
                            )
                        } else {
                            Brush.verticalGradient(
                                0.0f to Color.Transparent,
                                0.45f to palette.card.copy(alpha = 0.25f),
                                0.72f to palette.card.copy(alpha = 0.7f),
                                1.0f to palette.card
                            )
                        }
                    )
            )
        }

        Column(
            Modifier
                // Bottom-aligned so the title sits on the darkest part of the picture. When the text
                // outgrows the tile at 200% the box grows with it and this degrades to top-aligned,
                // which is the right failure.
                .align(Alignment.BottomStart)
                .padding(if (wide) 18.dp else 14.dp)
        ) {
            if (item.forYou && showForYouMark) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(6.dp).clip(CircleShape).background(accent))
                    Spacer(Modifier.width(6.dp))
                    Text("FOR YOU", style = MaterialTheme.typography.labelSmall, color = onBg)
                }
                Spacer(Modifier.height(10.dp))
            }

            // No maxLines: on a poster tile the title is the whole pitch, so truncating it would
            // leave the tile saying nothing at all.
            Text(
                item.title,
                style = if (wide) MaterialTheme.typography.headlineMedium
                else MaterialTheme.typography.headlineSmall,
                color = onBg
            )

            // The deck rides the WIDE tile only — that is what makes the size difference mean
            // something: a bigger tile is one that tells you more, not one that shouts louder.
            if (wide) {
                Spacer(Modifier.height(8.dp))
                Text(
                    item.deck,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (item.read) muted.copy(alpha = 0.65f) else muted,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis
                )
            }

            Spacer(Modifier.height(10.dp))
            Text(
                meta.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.65f)
            )
        }
    }
}

/**
 * A row of two poster tiles, or one and the gap where its partner would be.
 *
 * `IntrinsicSize.Min` so both match while still growing with their content and the font scale. An odd
 * item keeps its half width rather than stretching — a lone full-width tile mid-section would read as
 * a wide slot and break the rhythm.
 */
@Composable
fun GalleryRow(
    pair: List<GalleryItem>,
    palette: SurfacePalette,
    onBg: Color,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier,
    showForYouMark: Boolean = true,
    onOpen: (GalleryItem) -> Unit
) {
    Row(
        modifier.fillMaxWidth().height(IntrinsicSize.Min),
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        pair.forEach { item ->
            GalleryTile(
                item = item,
                palette = palette,
                onBg = onBg,
                muted = muted,
                accent = accent,
                modifier = Modifier.weight(1f).fillMaxHeight(),
                wide = false,
                showForYouMark = showForYouMark
            ) { onOpen(item) }
        }
        if (pair.size == 1) Spacer(Modifier.weight(1f))
    }
}
