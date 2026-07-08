package com.forge.app.ui.gym.history

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.formatVolume
import com.forge.app.program.Program
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.EditorialHairline
import com.forge.app.ui.theme.LocalForgeSettings
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The History list rows, shared beyond the History screen (the Stats day-detail sheet renders the
 * same rows so "a day" looks identical everywhere): open hairline-separated rows in the Home-screen
 * RECENT rhythm — mono date label over the name, right-aligned figures, no card shells.
 */

/**
 * One open gym-session row — no Surface card. A hairline divider sits below the row. Day name is a
 * small-caps mono meta label; volume + duration are right-aligned.
 */
@Composable
internal fun SessionRow(
    session: Session,
    onClick: () -> Unit,
    outline: Color,
    modifier: Modifier = Modifier
) {
    val cs = MaterialTheme.colorScheme
    val muted = cs.onSurfaceVariant
    val dayName = Program.dayDisplayName(session.dayKey)
    val durationMin = session.durationMinutes()
    val useKg = LocalForgeSettings.current.useKg
    val tags = session.tags.split(",").map { it.trim() }.filter { it.isNotEmpty() }

    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onClick() }
                .padding(vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                // Small-caps mono meta label for the date
                Text(
                    formatHistoryDate(session.startedAt).uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 9.sp,
                    letterSpacing = 0.5.sp
                )
                // Day name — body text directly on page
                Text(dayName, style = MaterialTheme.typography.bodyMedium, color = cs.onBackground)
                if (tags.isNotEmpty()) {
                    // Passive tag metadata renders bare (§1: no box without interactivity) — mono,
                    // muted, joined with the standard " · " meta separator.
                    Text(
                        tags.take(4).joinToString(" · ") { "#$it" },
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp,
                        letterSpacing = 0.5.sp
                    )
                }
            }
            // Right-aligned figures: volume primary, duration secondary
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                if (session.totalVolumeLb != null && session.totalVolumeLb > 0) {
                    Text(
                        formatVolume(session.totalVolumeLb, useKg),
                        style = MaterialTheme.typography.bodyMedium,
                        color = cs.onBackground
                    )
                    Text(
                        unitLabel(useKg).uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp
                    )
                }
                if (durationMin != null) {
                    Text(
                        "${durationMin}m",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp
                    )
                }
            }
        }
        EditorialHairline(outline = outline)
    }
}

/**
 * One open cardio row — no Surface card. Icon + activity name left; distance + duration right.
 * A hairline divider closes the row.
 */
@Composable
internal fun CardioHistoryRow(
    entry: CardioEntry,
    onClick: () -> Unit,
    outline: Color,
    modifier: Modifier = Modifier
) {
    val cs = MaterialTheme.colorScheme
    val muted = cs.onSurfaceVariant
    val type = CardioType.fromCode(entry.type)
    val useMiles = LocalForgeSettings.current.useMiles

    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onClick() }
                .padding(vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Row(
                modifier = Modifier.weight(1f),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                Icon(
                    type.icon,
                    contentDescription = null,
                    tint = muted,
                    modifier = Modifier.size(20.dp)
                )
                Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    // Small-caps mono date label
                    Text(
                        formatHistoryDate(entry.date).uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp,
                        letterSpacing = 0.5.sp
                    )
                    // Activity name directly on page
                    Text(
                        type.displayName,
                        style = MaterialTheme.typography.bodyMedium,
                        color = cs.onBackground
                    )
                }
            }
            // Right-aligned figures: distance primary, duration secondary
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                entry.distanceKm?.let {
                    Text(
                        formatDistance(it, useMiles),
                        style = MaterialTheme.typography.bodyMedium,
                        color = cs.onBackground
                    )
                }
                if (entry.durationMin > 0) {
                    Text(
                        "${entry.durationMin}m",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp
                    )
                }
            }
        }
        EditorialHairline(outline = outline)
    }
}

private val historyDateFormat = SimpleDateFormat("MMM d, yyyy", Locale.getDefault())
internal fun formatHistoryDate(epochMs: Long): String = historyDateFormat.format(Date(epochMs))
