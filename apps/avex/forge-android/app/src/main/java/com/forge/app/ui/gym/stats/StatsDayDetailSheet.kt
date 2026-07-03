package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.EditorialHairline
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.gym.history.CardioHistoryRow
import com.forge.app.ui.gym.history.HistoryItem
import com.forge.app.ui.gym.history.SessionRow
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * "What did I do that day?" — opened by tapping a lit day on the consistency heatmap. Renders the
 * day's gym sessions + cardio with the SAME rows as the History screen, and tapping a row drills
 * into the same detail screens History uses.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun StatsDayDetailSheet(
    detail: StatsDayDetail,
    onOpenSession: (Long) -> Unit,
    onOpenCardio: (Long) -> Unit,
    onDismiss: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val dayLine = remember(detail.date) {
        detail.date.format(DateTimeFormatter.ofPattern("EEEE · MMMM d, yyyy", Locale.getDefault())).uppercase()
    }
    val workouts = detail.items.count { it is HistoryItem.Workout }
    val cardio = detail.items.count { it is HistoryItem.Cardio }
    val summary = buildList {
        if (workouts > 0) add("$workouts workout${if (workouts == 1) "" else "s"}")
        if (cardio > 0) add("$cardio cardio session${if (cardio == 1) "" else "s"}")
    }.joinToString(" · ")

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = cs.background
    ) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 40.dp)) {
            Text(
                dayLine,
                style = MaterialTheme.typography.labelSmall,
                color = cs.onSurfaceVariant,
                fontSize = 9.sp,
                letterSpacing = 1.sp
            )
            Spacer(Modifier.height(4.dp))
            Text("That day.", style = MaterialTheme.typography.headlineSmall, color = cs.onBackground)
            if (summary.isNotEmpty()) {
                Spacer(Modifier.height(2.dp))
                Text(summary, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }
            Spacer(Modifier.height(14.dp))
            EditorialHairline(outline = cs.outline)
            if (detail.items.isEmpty()) {
                InlineEmptyHint(
                    "Nothing logged on this day.",
                    color = cs.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 16.dp)
                )
            } else {
                detail.items.forEach { item ->
                    when (item) {
                        is HistoryItem.Workout -> SessionRow(
                            session = item.session,
                            onClick = { onOpenSession(item.session.id) },
                            outline = cs.outline
                        )
                        is HistoryItem.Cardio -> CardioHistoryRow(
                            entry = item.entry,
                            onClick = { onOpenCardio(item.entry.id) },
                            outline = cs.outline
                        )
                    }
                }
            }
        }
    }
}
