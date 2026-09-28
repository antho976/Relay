package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.layout
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.heightIn
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape
import com.forge.app.ui.gym.train.state.DayListItem
import com.forge.app.ui.common.parseAccentHex
import java.text.SimpleDateFormat
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import java.util.Date
import java.util.Locale

@Composable
internal fun CompactCard(
    item: DayListItem,
    onClick: () -> Unit,
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
    corners: Corners = Corners(true, true, true, true)
) {
    // parseAccentHex, not the throwing parser this used to call: the value crosses DataStore
    // (a user-set day colour) and the program_day.accent_hex column, so a restored backup or a
    // blank hex would have taken the whole Train tab down on composition. One parser, non-throwing
    // — the same one ForgeTheme and ForgeWidget already use for the same kind of value.
    val accent = parseAccentHex(item.customAccentHex ?: item.plan.accentHex)
    val shape = memberShape(corners, selected = false)
    Box(
        modifier
            .fillMaxWidth()
            // Min, never fixed: the card grows with font scale and the spine grows with it.
            .heightIn(min = 88.dp)
            .memberFill(shape, selected = false)
            .bounceCombinedClick(
                onClickLabel = "Open ${item.displayName}",
                onLongClickLabel = "Options for ${item.displayName}",
                onLongClick = onLongPress,
                onClick = onClick
            ),
        contentAlignment = Alignment.CenterStart
    ) {
        Box(Modifier.matchParentSize()) { SpineStrip(accent = accent, word = item.plan.word) }
        Row(Modifier.fillMaxWidth().padding(start = SPINE_W), verticalAlignment = Alignment.CenterVertically) {
        Column(
            Modifier.weight(1f).padding(horizontal = ROW_H, vertical = 14.dp),
            verticalArrangement = Arrangement.spacedBy(3.dp)
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(item.displayName, style = MaterialTheme.typography.titleLarge, color = MaterialTheme.colorScheme.onSurface)
                if (item.isActive) ActiveDot(accent)
            }
            Text(
                buildString {
                    append(item.lastFinishedAt?.let { formatRelative(it) } ?: "Never trained")
                    append(" · ${item.exerciseCount} exercises")
                    val mins = item.estimatedMinutes
                        ?: com.forge.app.program.SessionEstimate.estimateMinutes(item.plan)
                    if (mins > 0) append(" · ~$mins min")
                },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        Text("→", modifier = Modifier.padding(end = ROW_H), style = MaterialTheme.typography.titleMedium, color = accent.copy(alpha = 0.70f))
        }
    }
}

@Composable
internal fun NextUpPill(accent: Color) {
    Box(
        Modifier
            .clip(RoundedCornerShape(6.dp))
            .background(accent.copy(alpha = 0.15f))
    ) {
        Text(
            "NEXT UP",
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
            style = MaterialTheme.typography.labelSmall,
            color = accent,
            fontWeight = FontWeight.Black,
            letterSpacing = 1.5.sp
        )
    }
}

/** The day-colour spine down a day card's leading edge. */
internal val SPINE_W = 44.dp

@Composable
internal fun SpineStrip(accent: Color, word: String) {
    Box(
        Modifier.width(SPINE_W).fillMaxHeight().background(accent.copy(alpha = 0.15f)),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = word,
            color = accent,
            style = MaterialTheme.typography.labelMedium,
            fontWeight = FontWeight.Black,
            letterSpacing = 3.sp,
            maxLines = 1,
            // Measured unrotated at its natural width, then laid out with width and height swapped,
            // so the rotated word reads whole instead of wrapping inside the 44dp strip.
            modifier = Modifier
                .layout { measurable, _ ->
                    val p = measurable.measure(Constraints())
                    layout(p.height, p.width) {
                        p.place((p.height - p.width) / 2, (p.width - p.height) / 2)
                    }
                }
                .graphicsLayer { rotationZ = -90f }
        )
    }
}

@Composable
internal fun ActiveDot(color: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(Modifier.width(6.dp).height(6.dp).clip(RoundedCornerShape(50)).background(color))
        Text("ACTIVE", color = color, style = MaterialTheme.typography.labelSmall, fontWeight = FontWeight.Black, letterSpacing = 1.sp)
    }
}

/**
 * Built per call, from the CURRENT default locale.
 *
 * As a top-level `val` this froze `Locale.getDefault()` at class load, so a user who changed their
 * phone's language kept seeing dates in the old locale's format until the process restarted. It is
 * one small allocation on a branch that only runs for sessions a week or more old — cheaper than
 * the bug. (`SimpleDateFormat` is not thread-safe either, so a shared instance was a hazard as
 * well as a staleness one.)
 */
private fun dateFormat() = SimpleDateFormat("MMM d", Locale.getDefault())

/**
 * "Last trained" for a day card, in CALENDAR days — the same reading OverviewUiStateMapper's
 * relativeDay gives the same session.
 *
 * Bucketing elapsed milliseconds made this disagree with that surface exactly where it matters: a
 * session finished Tuesday 22:30, opened Wednesday 08:00, is 9.5 hours old, so it read "Today"
 * while the Overview read "YESTERDAY". A user who believes they have already trained today skips
 * the session.
 */
internal fun formatRelative(epochMs: Long): String {
    val zone = ZoneId.systemDefault()
    val date = Instant.ofEpochMilli(epochMs).atZone(zone).toLocalDate()
    val daysAgo = ChronoUnit.DAYS.between(date, LocalDate.now(zone))
    return when {
        daysAgo <= 0L -> "Today"
        daysAgo == 1L -> "Yesterday"
        daysAgo < 7L -> "$daysAgo days ago"
        else -> dateFormat().format(Date(epochMs))
    }
}
