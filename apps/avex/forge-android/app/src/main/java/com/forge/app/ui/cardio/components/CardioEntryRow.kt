package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.ui.cardio.LocalCardioTypes
import com.forge.app.domain.units.formatDistance
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale

/**
 * One row in the "What I did" list — deliberately minimal: day, type, and a one-line summary
 * (duration · distance, or the rest reason). The full stats for a session live behind a tap, in
 * [CardioSessionDetailSheet]. Tap → open that detail; swipe left → delete.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CardioEntryRow(
    entry: CardioEntry,
    today: LocalDate,
    useMiles: Boolean,
    onRequestDelete: () -> Unit,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val activity = CardioActivity.resolve(entry.type, LocalCardioTypes.current)
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    val zone = ZoneId.systemDefault()
    val dayLabel = remember(entry.date, today) { entryDayLabel(entry.date, today, zone) }
    val summary = remember(entry, useMiles) { rowSummary(entry, activity.isRest, useMiles) }

    val dismissState = rememberSwipeToDismissBoxState(
        confirmValueChange = { value ->
            if (value == SwipeToDismissBoxValue.EndToStart) { onRequestDelete(); false } else false
        }
    )
    LaunchedEffect(dismissState.currentValue) {
        if (dismissState.currentValue != SwipeToDismissBoxValue.Settled) dismissState.reset()
    }
    // Only tint the delete background while a swipe is actually in progress — at rest the row reads
    // as the normal surface, never a permanent red block.
    val swiping = dismissState.targetValue != SwipeToDismissBoxValue.Settled

    SwipeToDismissBox(
        state = dismissState,
        enableDismissFromStartToEnd = false,
        backgroundContent = {
            Box(
                Modifier.fillMaxSize()
                    .background(if (swiping) MaterialTheme.colorScheme.error else Color.Transparent),
                contentAlignment = Alignment.CenterEnd
            ) {
                if (swiping) {
                    Text(
                        "Delete",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onError,
                        modifier = Modifier.padding(end = 24.dp)
                    )
                }
            }
        },
        modifier = modifier
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(MaterialTheme.colorScheme.background)
                .clickableLabeled("View session details", onClick = onClick)
                .padding(horizontal = 24.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                dayLabel,
                style = MaterialTheme.typography.labelMedium,
                color = muted,
                fontSize = 10.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.width(56.dp)
            )
            Icon(
                activity.icon,
                contentDescription = null,
                tint = onBg,
                modifier = Modifier.size(20.dp)
            )
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(activity.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg)
                if (summary.isNotBlank()) {
                    Text(
                        summary,
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontStyle = FontStyle.Italic,
                        fontSize = 10.sp
                    )
                }
            }
        }
    }
}

private fun entryDayLabel(dateMs: Long, today: LocalDate, zone: ZoneId): String {
    val entryDate = Instant.ofEpochMilli(dateMs).atZone(zone).toLocalDate()
    val isoWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
    val lastWeekStart = isoWeekStart.minusWeeks(1)
    val dayAbbr = entryDate.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault())
        .uppercase().take(3)
    return when {
        !entryDate.isBefore(isoWeekStart) -> dayAbbr
        !entryDate.isBefore(lastWeekStart) -> "LAST $dayAbbr"
        else -> {
            // Older rows carry the human date, never a raw ISO week number (§11: translate the machine).
            val fmt = DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())
            "$dayAbbr · ${entryDate.format(fmt).uppercase()}"
        }
    }
}

/** A one-line row summary — just duration + distance (or the rest reason). Everything else (pace,
 *  HR zone, calories, effort, note, GPS) is shown in the per-session detail, not crammed here. */
private fun rowSummary(entry: CardioEntry, isRest: Boolean, useMiles: Boolean): String {
    if (isRest) return CardioRestReason.fromCode(entry.restReason)?.displayName ?: "Rest day"
    return buildList {
        if (entry.durationMin > 0) add("${entry.durationMin} min")
        entry.distanceKm?.let { add(formatDistance(it, useMiles)) }
    }.joinToString(" · ")
}
