package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.goals.GoalProgressLine
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.customGoalValueLine
import com.forge.app.ui.theme.LocalForgeSettings
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** Size of one filmstrip photo cell — portrait 3:4, tall enough to actually read as a photo. */
private val StripCellWidth = 132.dp
private val StripCellHeight = 176.dp

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
    when (tile) {
        is GoalTile.Lift -> {
            name = tile.g.name
            valueLine = "${weightInputValue(tile.g.currentBestLb, settings.weightUnit)} / " +
                "${weightInputValue(tile.g.targetLb, settings.weightUnit)} ${unitLabel(settings.weightUnit)}"
        }
        is GoalTile.Custom -> {
            name = customGoalTitle(tile.g)
            valueLine = customGoalValueLine(tile.g, settings.weightUnit, settings.useMiles)
        }
    }
    // The shared goal line (ui/goals) — one visual language whether a goal shows here or on the
    // Goals screen; the sweep-in animation lives in the shared component.
    GoalProgressLine(
        title = name, valueLine = valueLine,
        fraction = tile.fraction, achieved = tile.achieved, index = index,
        onBg = onBg, muted = muted, accent = accent, outline = outline,
        onClick = onClick
    )
}

/**
 * GALLERY — a full-bleed horizontal filmstrip of the latest photos, echoing the cover photo's
 * edge-to-edge treatment instead of boxing thumbnails in a card. Dates sit on the photos over a
 * bottom scrim; the section header carries the "view all →" link into the full Gallery — the strip
 * is the preview beside it (§4.2), so there's one view-all home, not a second tail cell. Adding a
 * photo lives inside the Gallery (or the empty-state "first photo" cell here). The section must be
 * composed OUTSIDE the page's side margins — it applies its own padding to the header and lets the
 * photos run to the screen edge.
 */
@Composable
internal fun GalleryStrip(
    photos: List<ProgressPhoto>,
    fileFor: (ProgressPhoto) -> File,
    onAdd: () -> Unit,
    onView: (ProgressPhoto) -> Unit,
    onViewAll: () -> Unit,
    muted: Color,
    outline: Color
) {
    Box(Modifier.padding(horizontal = 24.dp)) {
        // Mirrors GOALS (§4.2): the header link opens the full Gallery, folding in the count once
        // the strip stops showing them all.
        //
        // The link stays at zero. It used to disappear when there were no photos — "nothing to
        // preview, no link" (§4.2) — but the Gallery is where importing, the guided camera and the
        // albums live, so with no photos the screen became unreachable entirely. §4.2's rule is
        // about not linking to an empty destination; here the destination is where you GO to make it
        // non-empty, and the ghost strip below is a real preview of it.
        SectionHeader(
            "GALLERY", muted,
            action = when {
                photos.isEmpty() -> "gallery →"
                photos.size > 10 -> "all ${photos.size} →"
                else -> "view all →"
            },
            onAction = onViewAll
        )
    }
    if (photos.isEmpty()) {
        // Empty is drawn (§12), and the zero-shape is the STRIP — a run of ghost cells that runs off
        // the edge exactly as the real filmstrip does, not a single boxed frame. One frame alone read
        // as a lone empty container rather than as this section with nothing in it yet.
        Row(
            Modifier.padding(start = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            repeat(3) { i ->
                // The trailing cells recede, so the strip reads as continuing rather than as three
                // equal empty boxes competing for attention.
                //
                // Off the MUTED ramp, not the outline one. These frames are the section's whole mark
                // at zero, and at outline 0.35/0.25 the border measured 1.10:1 against the page on
                // device — a box you cannot see is not an empty state, it is a blank strip (§12). The
                // lead cell takes §12's `muted@0.55`, which is also what §1 asks for: it is the one
                // cell you can actually tap, so it is the one that earns a visible border.
                val fade = muted.copy(alpha = if (i == 0) 0.55f else 0.30f)
                Box(
                    Modifier.width(StripCellWidth).height(StripCellHeight)
                        .clip(RoundedCornerShape(16.dp))
                        .border(1.dp, fade, RoundedCornerShape(16.dp))
                        .then(if (i == 0) Modifier.bounceClick { onAdd() } else Modifier),
                    contentAlignment = Alignment.Center
                ) {
                    if (i == 0) {
                        // §14: the accent carries the glyph, the words stay on onBg. Accent text
                        // measures 2.35:1 on Pearl, and this prompt is the only content in the cell.
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                "+ ",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.primary
                            )
                            Text(
                                "first photo",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onBackground
                            )
                        }
                    }
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
            StripPhotoCell(photo, fileFor(photo), onView)
        }
    }
}

/** One filmstrip photo — portrait crop, date overlaid on a soft bottom scrim. */
@Composable
private fun StripPhotoCell(photo: ProgressPhoto, file: File, onView: (ProgressPhoto) -> Unit) {
    Box(
        Modifier.width(StripCellWidth).height(StripCellHeight)
            .clip(RoundedCornerShape(16.dp))
            .bounceClick { onView(photo) }
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
