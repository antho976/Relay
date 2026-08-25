package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.experiment.CardShape
import com.forge.app.ui.goals.GoalProgressLine
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.customGoalValueLine
import com.forge.app.ui.goals.goalCaption
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.MonoSectionAnchor
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** Size of one filmstrip photo cell — portrait 3:4, tall enough to actually read as a photo. */
private val StripCellWidth = 132.dp
private val StripCellHeight = 176.dp

/**
 * An empty photo slot, on the same rung the calendar draws a rest day — one page-wide answer to
 * "nothing here yet" instead of a second vocabulary for it. See this file's [GalleryStrip] header.
 */
private const val GHOST_ALPHA = 0.30f

/** Each cell a step fainter, so the strip reads as continuing off the edge rather than ending. */
private val GHOST_DEPTH = floatArrayOf(1f, 0.62f, 0.34f)

/**
 * The lead ghost's inner margin. Wider than the 8–9dp a filled cell insets its date by, because
 * that caption sits on a photograph and this one sits in an empty frame — with nothing above it,
 * the margin is the only thing giving the words a place to be.
 */
private val GHOST_PADDING = 14.dp

/** A goal tile — a lift target or an auto-tracked custom goal, unified for previewing. */
private sealed interface GoalTile {
    val fraction: Float
    val achieved: Boolean

    data class Lift(val g: GoalRepository.GoalProgress) : GoalTile {
        override val fraction get() = g.fraction
        override val achieved get() = g.achieved
    }

    data class Custom(val g: ExtendedGoalRepository.Progress) : GoalTile {
        override val fraction get() = g.fraction
        override val achieved get() = g.achieved
    }
}

/**
 * GOALS — the top goals as a stack of open progress lines (achieved-first / closest-first), mixing
 * lift targets and auto-tracked custom goals, each bar sweeping in on entrance. Home's teaser
 * (dropped from the Profile 2026-07-03) — capped at three; the header action opens the full Goals
 * screen, where goals are added and edited.
 */
@Composable
internal fun GoalLinesSection(
    goals: List<GoalRepository.GoalProgress>,
    customGoals: List<ExtendedGoalRepository.Progress>,
    onOpenGoals: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val total = goals.size + customGoals.size
    SectionHeader(
        "GOALS", muted,
        action = when {
            total == 0 -> null
            total > 3 -> "all $total →"
            else -> "view all →"
        },
        onAction = if (total > 0) onOpenGoals else null
    )
    if (total == 0) {
        Row(
            Modifier.fillMaxWidth().bounceClick { onOpenGoals() },
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("Set targets, track your lifts", style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text("→", style = MaterialTheme.typography.bodyMedium, color = accent)
        }
        return
    }
    // In-progress goals lead (closest-first), then achieved ones — so a full slate of reached goals
    // can't bury the active target you're actually working toward.
    val preview = (goals.map { GoalTile.Lift(it) } + customGoals.map { GoalTile.Custom(it) })
        .sortedWith(compareBy<GoalTile> { it.achieved }.thenByDescending { it.fraction })
        .take(3)
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(18.dp)) {
        preview.forEachIndexed { i, tile -> GoalLine(tile, i, onOpenGoals, onBg, muted, accent, outline) }
    }
}

@Composable
private fun GoalLine(
    tile: GoalTile,
    index: Int,
    onClick: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val settings = LocalForgeSettings.current
    val name: String
    val valueLine: String
    val caption: String?
    when (tile) {
        is GoalTile.Lift -> {
            name = tile.g.name
            valueLine = "${weightInputValue(tile.g.currentBestLb, settings.weightUnit)} / " +
                "${weightInputValue(tile.g.targetLb, settings.weightUnit)} ${unitLabel(settings.weightUnit)}"
            // A lift target has neither a window nor a baseline: it is done or it is not.
            caption = if (tile.g.achieved) "Reached" else null
        }
        is GoalTile.Custom -> {
            name = customGoalTitle(tile.g)
            valueLine = customGoalValueLine(tile.g, settings.weightUnit, settings.useMiles)
            caption = goalCaption(
                achieved = tile.g.achieved,
                metric = tile.g.metric,
                period = tile.g.period,
                baselineValue = tile.g.baselineValue,
                weightUnit = settings.weightUnit,
                nowMs = System.currentTimeMillis(),
            )
        }
    }
    // The shared goal line (ui/goals) — one visual language whether a goal shows here or on the
    // Goals screen; the sweep-in animation lives in the shared component.
    GoalProgressLine(
        title = name, valueLine = valueLine,
        fraction = tile.fraction, achieved = tile.achieved,
        onBg = onBg, muted = muted, accent = accent, outline = outline,
        caption = caption,
        onClick = onClick
    )
}

/**
 * GALLERY — a full-bleed horizontal filmstrip of the latest photos, echoing the cover photo's
 * edge-to-edge treatment instead of boxing thumbnails in a card. Dates sit on the photos over a
 * bottom scrim. The section must be composed OUTSIDE the page's side margins — it applies its own
 * padding to the header and lets the photos run to the screen edge.
 *
 * ## The pictures ARE the link (2026-08-22)
 *
 * The header used to carry a "view all →" / "gallery →" action, and every cell in the strip had a
 * different job depending on what you had: a photo opened a viewer dialog, the lead empty cell
 * opened the add-photo chooser, the other empty cells did nothing at all. Antho: *"remove the
 * gallery text, you should enter it by clicking the gallery pictures, and nothing in gallery should
 * be gated behind having a picture or not."*
 *
 * So every cell now does the one thing, photos or not: it opens the Gallery. That kills three
 * problems at once. The link is redundant when the thing beside it is already the link. The strip
 * stops being a control panel where adjacent identical-looking cells behave differently (§2③). And
 * the empty state stops being a lesser version of the section — the zero-shape is now the same
 * affordance as the filled one, which is what "not gated" means in layout terms.
 *
 * Viewing a single photo, adding one, albums and the guided camera all live in the Gallery, which
 * is where you now land. One home for each (§4.3).
 *
 * ## The ghost strip stopped being a row of cards (2026-08-24)
 *
 * Its empty state was the last boxed thing on the page: three cells with a card gradient inside a
 * 1dp border, carrying the "+ First photo" line as their contents. That was written down as the one
 * deliberate exception to the de-boxing, and once the ACTIVITY grid went full-width and took the
 * accent it stopped surviving the comparison — Antho: *"looks out of place compared to the new UI
 * in the page."* A bordered slab is a promise of a surface, and this page has no surfaces left.
 *
 * So the ghost cells are drawn the way the calendar draws a rest day: one quiet fill on the
 * `outline` rung, no border, corners on the card radius, each successive cell a step fainter so the
 * strip still reads as continuing into photos not taken yet. The page now has ONE shape for "this
 * slot is empty", used by both sections that have one.
 *
 * The invitation came out of the first cell and became a line under the strip, on the gutter, which
 * is where BODY's "Log your first" already puts the same offer. Text inside a bordered box was the
 * card's last argument for existing; on a line it needs no container, and it is legible against the
 * page instead of against a fill.
 */
@Composable
internal fun GalleryStrip(
    photos: List<ProgressPhoto>,
    fileFor: (ProgressPhoto) -> File,
    onOpenGallery: () -> Unit,
    muted: Color,
    locked: Boolean = false,
) {
    Box(Modifier.padding(horizontal = 24.dp)) {
        // Label only. `SectionAnchor` with no action renders exactly the same mono anchor the other
        // sections use, so GALLERY finally sits on the page's own rung instead of being the one
        // header with a trailing accent word.
        Text(
            "GALLERY",
            style = MonoSectionAnchor,
            color = muted,
            modifier = Modifier.semantics { heading() }
        )
    }
    Spacer(Modifier.height(12.dp))
    if (locked || photos.isEmpty()) {
        // Empty is drawn (§12), and the zero-shape is the STRIP — a run of ghost cells that runs off
        // the edge exactly as the real filmstrip does, not a single boxed frame. One frame alone read
        // as a lone empty container rather than as this section with nothing in it yet.
        //
        // All three are tappable, and all three open the Gallery. Previously only the lead cell did
        // anything and it opened a different destination, so the strip taught you that two of its
        // cells were dead — the exact gating Antho called out.
        Row(
            Modifier.padding(start = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            repeat(3) { i ->
                Box(
                    Modifier.width(StripCellWidth).height(StripCellHeight)
                        .clip(CardShape)
                        .background(MaterialTheme.colorScheme.outline.copy(alpha = GHOST_ALPHA * GHOST_DEPTH[i]))
                        .bounceClick { onOpenGallery() }
                        .padding(GHOST_PADDING)
                ) {
                    if (i == 0) GhostInvitation(muted, locked, Modifier.align(Alignment.BottomStart))
                }
            }
        }
        return
    }
    LazyRow(
        contentPadding = PaddingValues(horizontal = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        items(photos.take(10), key = { it.fileName }) { photo ->
            StripPhotoCell(photo, fileFor(photo), onOpenGallery)
        }
    }
}

/**
 * The offer, written inside the first empty frame.
 *
 * ## Where it goes, and why the bottom
 *
 * A filled cell captions itself at the bottom-left — the date, and the pose chip up in the corner.
 * The empty cell is that same cell with the photograph missing, so its words sit exactly where a
 * filled one's words sit. Anything else (centred, floating, a plus in the middle of the frame)
 * makes the zero-state a different object from the thing it stands in for, which is the failure §12
 * is actually about.
 *
 * ## Why two voices and not one
 *
 * The first line is the action and the second is how to take the shot, and they are different kinds
 * of sentence. Mono uppercase is this app's label voice — it is what the date on the filled cell
 * beside it is set in, and what every anchor down the page is set in — so the action inherits the
 * strip's own lettering. The tip is a spoken instruction, so it stays sans and drops to `muted`:
 * one rung down in tone and one voice over in face, which is the whole hierarchy in two lines.
 *
 * The action is `labelMedium`, not the 9sp the first pass hand-set it at. Against a 12sp sans tip
 * a 9sp label loses — the thing you are being offered read smaller than the note about how to
 * shoot it, which is the hierarchy backwards. On the 11sp rung the uppercase mono and the brighter
 * tone put the offer back on top, and both lines now take their size from the scale rather than
 * from a call site (§6).
 *
 * The previous version put both on the page UNDER the strip, and before that crammed them into a
 * bordered card at `bodySmall` where "Same pose, same light." wrapped mid-phrase against a 104dp
 * measure. At 9sp mono over sans the block is three short lines that break where the comma already
 * breaks, and it occupies the bottom quarter of the frame instead of a third of it.
 *
 * §14: the accent carries the `+` glyph only. Accent text measures 2.35:1 on Pearl, so the words
 * themselves stay on `onBackground` and the mark is never the only thing saying "add".
 */
@Composable
private fun GhostInvitation(muted: Color, locked: Boolean, modifier: Modifier = Modifier) {
    Column(modifier) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (!locked) {
                Text("+", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.width(5.dp))
            }
            Text(
                if (locked) "UNLOCK PHOTOS" else "FIRST PHOTO",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onBackground
            )
        }
        Spacer(Modifier.height(7.dp))
        Text(
            if (locked) "Gallery locked" else "Same pose,\nsame light.",
            style = MaterialTheme.typography.bodySmall,
            // Plain muted, not the 0.7 on-card floor: there is no card fill under this any more,
            // and on the page 0.65 measures 4.54:1 and passes.
            color = muted
        )
    }
}

/** One filmstrip photo — portrait crop, date overlaid on a soft bottom scrim. */
@Composable
private fun StripPhotoCell(photo: ProgressPhoto, file: File, onOpenGallery: () -> Unit) {
    Box(
        Modifier.width(StripCellWidth).height(StripCellHeight)
            .clip(CardShape)
            // Opens the Gallery, not an in-place viewer. The strip is a preview of a destination,
            // and a preview whose cells lead somewhere other than the thing they preview is a trap.
            .bounceClick { onOpenGallery() }
    ) {
        ProgressPhotoImage(file, Modifier.fillMaxSize(), reqPx = 480)
        Box(
            Modifier.matchParentSize().background(
                Brush.verticalGradient(0.62f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.55f))
            )
        )
        PhotoPose.fromKey(photo.pose)?.let { pose ->
            Text(
                pose.label.uppercase(),
                style = MaterialTheme.typography.labelSmall, color = Color.White.copy(alpha = 0.92f), fontSize = 8.sp,
                modifier = Modifier.align(Alignment.TopStart).padding(8.dp)
                    .clip(RoundedCornerShape(4.dp)).background(Color.Black.copy(alpha = 0.4f))
                    .padding(horizontal = 5.dp, vertical = 2.dp)
            )
        }
        Text(
            SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(photo.takenAtMs)).uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = Color.White.copy(alpha = 0.92f), fontSize = 8.sp,
            modifier = Modifier.align(Alignment.BottomStart).padding(horizontal = 9.dp, vertical = 8.dp)
        )
    }
}

// ON THIS DAY was cut from the profile 2026-07-24: Home already renders the same throwback
// (OverviewScreen's OnThisDayCard), and a mark that only repeats another screen's answer is cut,
// not copied (§4.3). It was also the page's one prose-only section, hung off a decorative accent
// rule (§1: a line exists only as data).
