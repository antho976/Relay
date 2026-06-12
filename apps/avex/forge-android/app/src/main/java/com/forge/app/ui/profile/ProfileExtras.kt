package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.RecapRowData
import com.forge.app.data.repo.TrophyCell
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.emphasized
import com.forge.app.ui.trophies.components.TrophyIconBadge
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** THE MIRROR TEST — private physique photos, never leave the device. */
@Composable
internal fun MirrorTestSection(
    photos: List<ProgressPhoto>,
    fileFor: (ProgressPhoto) -> File,
    onAdd: () -> Unit,
    onView: (ProgressPhoto) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    ProfileBlock("THE MIRROR TEST", muted, accent, outline, action = "+ add", onAction = onAdd) {
        Text(
            "Private — these never leave your phone.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp
        )
        Spacer(Modifier.height(12.dp))
        // Cell 0 is the add tile; the rest are dated thumbnails.
        val cells = listOf<ProgressPhoto?>(null) + photos
        cells.chunked(3).forEach { row ->
            Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                row.forEach { photo ->
                    Column(Modifier.weight(1f)) {
                        if (photo == null) {
                            Box(
                                Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(8.dp))
                                    .border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(8.dp))
                                    .bounceClick { onAdd() },
                                contentAlignment = Alignment.Center
                            ) {
                                Icon(Icons.Filled.Add, contentDescription = "Add photo", tint = accent, modifier = Modifier.size(22.dp))
                            }
                            Spacer(Modifier.height(4.dp))
                            Text("TODAY", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, modifier = Modifier.padding(start = 2.dp))
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
                repeat(3 - row.size) { Box(Modifier.weight(1f)) }
            }
        }
    }
}

/** TROPHY CASE — icon grid; unlocked carry a check, in-progress show a ring. */
@Composable
internal fun TrophyCaseSection(
    grid: List<TrophyCell>,
    unlocked: Int,
    total: Int,
    closestTrophy: String?,
    onOpenTrophies: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    ProfileBlock("TROPHY CASE", muted, accent, outline, action = "$unlocked / $total  →", onAction = onOpenTrophies) {
        grid.chunked(6).forEach { row ->
            Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEach { cell ->
                    Box(Modifier.weight(1f), contentAlignment = Alignment.Center) { TrophyGridCell(cell, accent, outline) }
                }
                repeat(6 - row.size) { Box(Modifier.weight(1f)) }
            }
        }
        if (closestTrophy != null) {
            Spacer(Modifier.height(10.dp))
            Text("Closest: $closestTrophy", style = MaterialTheme.typography.bodySmall, color = onBg, fontStyle = FontStyle.Italic)
        }
    }
}

@Composable
private fun TrophyGridCell(cell: TrophyCell, accent: Color, outline: Color) {
    val bg = MaterialTheme.colorScheme.background
    val onBg = MaterialTheme.colorScheme.onBackground
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Box(contentAlignment = Alignment.Center) {
            if (!cell.unlocked && cell.progress > 0f) {
                Canvas(Modifier.size(42.dp)) {
                    drawArc(
                        color = accent.copy(alpha = 0.85f),
                        startAngle = -90f,
                        sweepAngle = cell.progress * 360f,
                        useCenter = false,
                        style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round)
                    )
                }
            }
            Box(contentAlignment = Alignment.BottomEnd) {
                TrophyIconBadge(icon = cell.icon, unlocked = cell.unlocked, size = 34.dp)
                if (cell.unlocked) {
                    Box(Modifier.size(14.dp).clip(CircleShape).background(bg), contentAlignment = Alignment.Center) {
                        Box(Modifier.size(11.dp).clip(CircleShape).background(onBg), contentAlignment = Alignment.Center) {
                            Icon(Icons.Filled.Check, contentDescription = null, tint = bg, modifier = Modifier.size(7.dp))
                        }
                    }
                }
            }
        }
        if (!cell.unlocked && cell.progress > 0f) {
            Spacer(Modifier.height(3.dp))
            Text("${(cell.progress * 100).toInt()}%", style = MaterialTheme.typography.labelSmall, color = outline, fontSize = 8.sp)
        }
    }
}

/** ON THE RECORD — month / year in review entry points. */
@Composable
internal fun OnTheRecordSection(
    recaps: List<RecapRowData>,
    onOpenRecaps: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    ProfileBlock("ON THE RECORD", muted, accent, outline) {
        if (recaps.isEmpty()) {
            Row(Modifier.fillMaxWidth().bounceClick { onOpenRecaps() }, horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Text("Month & year in review", style = MaterialTheme.typography.bodyMedium, color = onBg)
                Text("→", style = MaterialTheme.typography.bodyMedium, color = accent)
            }
        } else {
            recaps.forEach { r ->
                Row(
                    Modifier.fillMaxWidth().bounceClick { onOpenRecaps() }.padding(vertical = 8.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(r.title, style = MaterialTheme.typography.bodyMedium, color = emphasized(onBg))
                        Text(r.subtitle, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                    }
                    Text(if (r.isYear) "BUILDING →" else "READY →", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 9.sp)
                }
            }
        }
    }
}
