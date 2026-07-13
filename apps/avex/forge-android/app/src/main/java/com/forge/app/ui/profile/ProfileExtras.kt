package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.monthsAgoPhrase
import com.forge.app.ui.goals.GoalProgressLine
import com.forge.app.ui.goals.customGoalTitle
import com.forge.app.ui.goals.customGoalValueLine
import com.forge.app.ui.overview.state.OnThisDayMemory
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
    Box(Modifier.padding(horizontal = 20.dp)) {
        // Mirrors GOALS (§4.2): the header link opens the full Gallery, folding in the count once
        // the strip stops showing them all; empty = nothing to view, so no link.
        SectionHeader(
            "GALLERY", muted,
            action = when {
                photos.isEmpty() -> null
                photos.size > 10 -> "all ${photos.size} →"
                else -> "view all →"
            },
            onAction = if (photos.isNotEmpty()) onViewAll else null
        )
    }
    if (photos.isEmpty()) {
        Column(Modifier.padding(horizontal = 20.dp)) {
            Box(
                Modifier.width(StripCellWidth).height(StripCellHeight)
                    .clip(RoundedCornerShape(16.dp))
                    .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(16.dp))
                    .bounceClick { onAdd() },
                contentAlignment = Alignment.Center
            ) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("＋", style = MaterialTheme.typography.headlineSmall, color = muted)
                    Spacer(Modifier.height(6.dp))
                    Text(
                        "FIRST PHOTO", style = MaterialTheme.typography.labelSmall,
                        color = muted, fontSize = 8.sp, letterSpacing = 1.5.sp
                    )
                }
            }
        }
        return
    }
    LazyRow(
        contentPadding = PaddingValues(horizontal = 20.dp),
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

/** ON THIS DAY — a single-line throwback to a workout from a previous month, set like a pull quote. */
@Composable
internal fun OnThisDaySection(memory: OnThisDayMemory, onBg: Color, muted: Color, accent: Color) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    SectionHeader("ON THIS DAY", muted)
    val ago = monthsAgoPhrase(memory.monthsAgo)
    Row(Modifier.height(IntrinsicSize.Min)) {
        Box(
            Modifier.width(2.dp).fillMaxHeight()
                .clip(RoundedCornerShape(50)).background(accent.copy(alpha = 0.6f))
        )
        Spacer(Modifier.width(12.dp))
        Text(
            "$ago you trained ${memory.dayName} · ${formatVolume(memory.totalVolumeLb, weightUnit)} ${unitLabel(weightUnit)}" +
                if (memory.prCount > 0) " · ${memory.prCount} PR${if (memory.prCount == 1) "" else "s"}" else "",
            style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic
        )
    }
}
