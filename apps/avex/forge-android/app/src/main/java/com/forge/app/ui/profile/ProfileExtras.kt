package com.forge.app.ui.profile

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.monthsAgoPhrase
import com.forge.app.ui.overview.state.OnThisDayMemory
import com.forge.app.ui.theme.LocalForgeSettings
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * GOALS — the top set goals as a grid of progress-ring tiles (achieved-first / closest-first). Each
 * tile previews the ring + name + current/target; the header and footer links open the full Goals
 * screen, where goals are added and edited.
 */
@Composable
internal fun GoalTilesSection(
    goals: List<GoalRepository.GoalProgress>,
    onOpenGoals: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    SectionHeader(
        "GOALS", muted, accent,
        action = if (goals.isNotEmpty()) "view all →" else null,
        onAction = if (goals.isNotEmpty()) onOpenGoals else null
    )
    if (goals.isEmpty()) {
        ProfileCard(onClick = onOpenGoals) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Text("Set targets, track your lifts", style = MaterialTheme.typography.bodyMedium, color = onBg)
                Text("→", style = MaterialTheme.typography.bodyMedium, color = accent)
            }
        }
        return
    }
    val preview = goals.take(4)
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        preview.chunked(2).forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                row.forEach { g -> GoalRingTile(g, onOpenGoals, onBg, muted, accent, outline, Modifier.weight(1f)) }
                if (row.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
    if (goals.size > preview.size) {
        Spacer(Modifier.height(10.dp))
        Text(
            "View all ${goals.size} goals →",
            style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp,
            modifier = Modifier.bounceClick { onOpenGoals() }.padding(start = 2.dp, top = 2.dp, bottom = 2.dp)
        )
    }
}

@Composable
private fun GoalRingTile(
    g: GoalRepository.GoalProgress,
    onClick: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    val useKg = LocalForgeSettings.current.useKg
    ProfileCard(modifier = modifier.height(140.dp), onClick = onClick, padding = 14.dp) {
        Column(
            Modifier.fillMaxSize(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            ProgressRing(
                fraction = g.fraction.coerceIn(0f, 1f),
                color = if (g.achieved) accent else onBg,
                trackColor = outline.copy(alpha = 0.3f),
                modifier = Modifier.size(54.dp)
            ) {
                if (g.achieved) Text("✓", style = MaterialTheme.typography.titleMedium, color = accent)
                else Text("${(g.fraction * 100).toInt()}%", style = MaterialTheme.typography.titleSmall, color = onBg)
            }
            Spacer(Modifier.height(10.dp))
            Text(
                g.name, style = MaterialTheme.typography.bodySmall, color = onBg,
                maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(2.dp))
            Text(
                "${weightInputValue(g.currentBestLb, useKg)} / ${weightInputValue(g.targetLb, useKg)} ${unitLabel(useKg)}",
                style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp,
                maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}

/** GALLERY — a 3-photo teaser on a card; the full grid lives behind "View all". */
@Composable
internal fun GalleryCard(
    photos: List<ProgressPhoto>,
    fileFor: (ProgressPhoto) -> File,
    onAdd: () -> Unit,
    onView: (ProgressPhoto) -> Unit,
    onViewAll: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    SectionHeader("GALLERY", muted, accent, action = "+ add", onAction = onAdd)
    ProfileCard {
        Text(
            "Private — these never leave your phone.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp
        )
        Spacer(Modifier.height(12.dp))
        val shown = photos.take(3)
        val cells: List<ProgressPhoto?> = shown + List(3 - shown.size) { null }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            cells.forEach { photo ->
                Column(Modifier.weight(1f)) {
                    if (photo == null) {
                        Box(
                            Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(8.dp))
                                .border(1.dp, outline.copy(alpha = 0.25f), RoundedCornerShape(8.dp))
                                .bounceClick { onAdd() }
                        )
                        Spacer(Modifier.height(4.dp))
                        Text("", style = MaterialTheme.typography.labelSmall, fontSize = 8.sp)
                    } else {
                        Box(
                            Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(8.dp)).bounceClick { onView(photo) }
                        ) {
                            ProgressPhotoImage(fileFor(photo), Modifier.fillMaxWidth().aspectRatio(1f), reqPx = 300)
                        }
                        Spacer(Modifier.height(4.dp))
                        Text(
                            SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(photo.takenAtMs)).uppercase(),
                            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp,
                            modifier = Modifier.padding(start = 2.dp)
                        )
                    }
                }
            }
        }
        Spacer(Modifier.height(10.dp))
        Text(
            if (photos.size > 3) "View all ${photos.size} photos →" else "View all photos →",
            style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp,
            modifier = Modifier.bounceClick { onViewAll() }.padding(start = 2.dp, top = 2.dp, bottom = 2.dp)
        )
    }
}

/** ON THIS DAY — a single-line throwback to a workout from a previous month. */
@Composable
internal fun OnThisDayCard(memory: OnThisDayMemory, onBg: Color, muted: Color, accent: Color) {
    val useKg = LocalForgeSettings.current.useKg
    SectionHeader("ON THIS DAY", muted, accent)
    ProfileCard {
        val ago = monthsAgoPhrase(memory.monthsAgo)
        Text(
            "$ago you trained ${memory.dayName} — ${formatVolume(memory.totalVolumeLb, useKg)} ${unitLabel(useKg)}" +
                if (memory.prCount > 0) " · ${memory.prCount} PR${if (memory.prCount == 1) "" else "s"}" else "",
            style = MaterialTheme.typography.bodyMedium, color = onBg
        )
    }
}
