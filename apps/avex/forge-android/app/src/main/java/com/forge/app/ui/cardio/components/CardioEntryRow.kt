package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
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
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.TextStyle
import java.time.temporal.WeekFields
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CardioEntryRow(
    entry: CardioEntry,
    today: LocalDate,
    onRequestDelete: () -> Unit,
    modifier: Modifier = Modifier
) {
    val type = CardioType.fromCode(entry.type)
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    val zone = ZoneId.systemDefault()
    val dayLabel = remember(entry.date, today) { entryDayLabel(entry.date, today, zone) }
    val detail = buildDetail(entry, type)
    val effortLabel = CardioEffort.fromCode(entry.effort)?.displayName?.uppercase() ?: ""

    val dismissState = rememberSwipeToDismissBoxState(
        confirmValueChange = { value ->
            if (value == SwipeToDismissBoxValue.EndToStart) { onRequestDelete(); false } else false
        }
    )
    LaunchedEffect(dismissState.currentValue) {
        if (dismissState.currentValue != SwipeToDismissBoxValue.Settled) dismissState.reset()
    }

    SwipeToDismissBox(
        state = dismissState,
        enableDismissFromStartToEnd = false,
        backgroundContent = {
            Box(
                Modifier.fillMaxSize()
                    .background(MaterialTheme.colorScheme.error.copy(alpha = 0.8f)),
                contentAlignment = Alignment.CenterEnd
            ) {
                Icon(
                    Icons.Default.Delete,
                    contentDescription = "Delete",
                    tint = MaterialTheme.colorScheme.onError,
                    modifier = Modifier.padding(end = 24.dp)
                )
            }
        },
        modifier = modifier
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp, vertical = 14.dp),
            verticalAlignment = Alignment.Top,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                dayLabel,
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 9.sp,
                modifier = Modifier.width(64.dp).padding(top = 2.dp)
            )
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(type.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg)
                if (detail.isNotBlank()) {
                    Text(
                        detail,
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontStyle = FontStyle.Italic,
                        fontSize = 10.sp
                    )
                }
            }
            if (effortLabel.isNotBlank()) {
                Text(
                    effortLabel,
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.65f),
                    fontSize = 9.sp,
                    modifier = Modifier.padding(top = 2.dp)
                )
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
        !entryDate.isBefore(isoWeekStart) -> "$dayAbbr →"
        !entryDate.isBefore(lastWeekStart) -> "LAST $dayAbbr"
        else -> {
            val wk = entryDate.get(WeekFields.ISO.weekOfWeekBasedYear())
            "WK $wk · $dayAbbr"
        }
    }
}

private fun buildDetail(entry: CardioEntry, type: CardioType): String {
    if (type.isRest) return CardioRestReason.fromCode(entry.restReason)?.displayName ?: "Rest day"
    val parts = buildList {
        if (entry.durationMin > 0) add("${entry.durationMin} min")
        entry.distanceKm?.let { add("${"%.1f".format(it)} km") }
    }
    return parts.joinToString(" · ")
}
