package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.LocalForgeSettings
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** GALLERY — a 3-photo teaser; the full chronological grid + optional albums live behind "View all". */
@Composable
internal fun MirrorTestSection(
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
    ProfileBlock("GALLERY", muted, accent, outline, action = "+ add", onAction = onAdd) {
        Text(
            "Private — these never leave your phone.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp
        )
        Spacer(Modifier.height(12.dp))
        // Capped at the 3 most recent; if there are fewer, pad with plain empty frames (no "+").
        val shown = photos.take(3)
        val cells: List<ProgressPhoto?> = shown + List(3 - shown.size) { null }
        Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            cells.forEach { photo ->
                Column(Modifier.weight(1f)) {
                    if (photo == null) {
                        // Empty frame — a plain hairline slot, no "+".
                        Box(
                            Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(8.dp))
                                .border(1.dp, outline.copy(alpha = 0.25f), RoundedCornerShape(8.dp))
                                .bounceClick { onAdd() }
                        )
                        Spacer(Modifier.height(4.dp))
                        // Blank label keeps empty frames the same height as dated photo cells.
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
                            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, modifier = Modifier.padding(start = 2.dp)
                        )
                    }
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            if (photos.size > 3) "View all ${photos.size} photos →" else "View all photos →",
            style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp,
            modifier = Modifier.bounceClick { onViewAll() }.padding(start = 2.dp, top = 2.dp, bottom = 2.dp)
        )
    }
}

/**
 * GOALS — a preview of the top few set goals (achieved-first / closest-first), each with a thin
 * progress bar. The header "view all →" and the bottom "View all N goals →" both open the full
 * Goals screen (where goals are added / edited).
 */
@Composable
internal fun GoalsPreviewSection(
    goals: List<GoalRepository.GoalProgress>,
    onOpenGoals: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val useKg = LocalForgeSettings.current.useKg
    val preview = goals.take(3)
    ProfileBlock(
        "GOALS", muted, accent, outline,
        action = if (goals.isNotEmpty()) "view all →" else null,
        onAction = if (goals.isNotEmpty()) onOpenGoals else null
    ) {
        if (goals.isEmpty()) {
            Row(
                Modifier.fillMaxWidth().bounceClick { onOpenGoals() },
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("Set targets, track your lifts", style = MaterialTheme.typography.bodyMedium, color = onBg)
                Text("→", style = MaterialTheme.typography.bodyMedium, color = accent)
            }
        } else {
            preview.forEachIndexed { i, g ->
                if (i > 0) Spacer(Modifier.height(12.dp))
                Column(Modifier.fillMaxWidth().bounceClick { onOpenGoals() }) {
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(g.name, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        if (g.achieved) Text("reached ✓", style = MaterialTheme.typography.labelSmall, color = accent)
                        else Text("${(g.fraction * 100).toInt()}%", style = MaterialTheme.typography.labelSmall, color = muted)
                    }
                    Spacer(Modifier.height(6.dp))
                    Box(
                        Modifier.fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp))
                            .background(outline.copy(alpha = 0.25f))
                    ) {
                        Box(
                            Modifier.fillMaxWidth(g.fraction.coerceIn(0f, 1f)).height(3.dp)
                                .clip(RoundedCornerShape(2.dp)).background(if (g.achieved) accent else onBg)
                        )
                    }
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "${weightInputValue(g.currentBestLb, useKg)} / ${weightInputValue(g.targetLb, useKg)} ${unitLabel(useKg)}",
                        style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
                    )
                }
            }
            if (goals.size > preview.size) {
                Spacer(Modifier.height(10.dp))
                Text(
                    "View all ${goals.size} goals →",
                    style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp,
                    modifier = Modifier.bounceClick { onOpenGoals() }.padding(top = 2.dp, bottom = 2.dp)
                )
            }
        }
    }
}
