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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.gym.train.state.DayListItem
import com.forge.app.ui.common.parseAccentHex
import java.text.SimpleDateFormat
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun CompactCard(
    item: DayListItem,
    onClick: () -> Unit,
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    // parseAccentHex, not the throwing parser this used to call: the value crosses DataStore
    // (a user-set day colour) and the program_day.accent_hex column, so a restored backup or a
    // blank hex would have taken the whole Train tab down on composition. One parser, non-throwing
    // — the same one ForgeTheme and ForgeWidget already use for the same kind of value.
    val accent = parseAccentHex(item.customAccentHex ?: item.plan.accentHex)
    val surface = MaterialTheme.colorScheme.surface
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .height(96.dp)
            .clip(RoundedCornerShape(16.dp))
            .combinedClickable(onClick = onClick, onLongClick = onLongPress)
            .bounceClick { onClick() },
        color = Color.Transparent,
        tonalElevation = 2.dp
    ) {
        Box(
            Modifier.fillMaxWidth().background(surface).drawBehind {
                val radius = size.width * 0.55f
                drawRect(
                    brush = Brush.radialGradient(
                        colorStops = arrayOf(
                            0.0f to accent.copy(alpha = 0.38f),
                            0.50f to accent.copy(alpha = 0.12f),
                            1.0f to Color.Transparent
                        ),
                        center = Offset(x = size.width * 0.95f, y = size.height * 0.5f),
                        radius = radius,
                        tileMode = TileMode.Clamp
                    )
                )
            }
        ) {
            Row(Modifier.fillMaxWidth().fillMaxHeight(), verticalAlignment = Alignment.CenterVertically) {
                SpineStrip(accent = accent, word = item.plan.word)
                Column(
                    Modifier.weight(1f).padding(horizontal = 16.dp, vertical = 12.dp),
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
                Text("→", modifier = Modifier.padding(end = 16.dp), style = MaterialTheme.typography.titleMedium, color = accent.copy(alpha = 0.70f))
            }
        }
    }
}

@Composable
internal fun NextUpPill(accent: Color) {
    Surface(shape = RoundedCornerShape(6.dp), color = accent.copy(alpha = 0.18f)) {
        Text("NEXT UP", modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp), color = accent, fontWeight = FontWeight.Black, fontSize = 10.sp, letterSpacing = 1.5.sp)
    }
}

@Composable
internal fun SpineStrip(accent: Color, word: String) {
    Box(
        Modifier.width(44.dp).fillMaxHeight().background(accent.copy(alpha = 0.18f)),
        contentAlignment = Alignment.Center
    ) {
        Text(text = word, color = accent, fontWeight = FontWeight.Black, fontSize = 12.sp, letterSpacing = 3.sp, modifier = Modifier.graphicsLayer { rotationZ = -90f })
    }
}

@Composable
internal fun ActiveDot(color: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(Modifier.width(6.dp).height(6.dp).clip(RoundedCornerShape(50)).background(color))
        Text("ACTIVE", color = color, fontSize = 9.sp, fontWeight = FontWeight.Black, letterSpacing = 1.sp)
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
