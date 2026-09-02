package com.forge.app.ui.common

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
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
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.repo.CardioRepository
import com.forge.app.ui.gym.history.CardioHistoryRow
import com.forge.app.ui.gym.history.HistoryItem
import com.forge.app.ui.gym.history.SessionRow
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * # "What did I do that day?"
 *
 * One calendar day of training, and the sheet that shows it. Opened by tapping a lit day on a
 * consistency grid — Stats' adherence heatmap, and Profile's ACTIVITY calendar.
 *
 * It reuses [HistoryItem] and the History screen's own rows, so a day looks identical everywhere
 * it is opened from and every row drills into the same detail screens History uses.
 *
 * It lived in `ui/gym/stats` while Stats was the only grid you could tap. Profile's ACTIVITY
 * calendar became tappable too (2026-08-31), and the second caller is what moved it here — a
 * `StatsDayDetailSheet` imported into the profile package would have read as a mistake, and the
 * alternative (a second sheet over the same rows) is how two answers to one question start
 * drifting apart.
 */
data class DayLog(
    val date: LocalDate,
    val items: List<HistoryItem>
)

/**
 * Everything logged on [date] — gym sessions plus cardio, newest first.
 *
 * Two bounded queries rather than loading whole histories and filtering one day out of them. Both
 * grids call this from a `viewModelScope.launch` that CANCELS the previous tap's load: without
 * that, two taps in quick succession race, and the slower — therefore usually the earlier —
 * request writes last, so the sheet shows a different day from the cell that was just pressed. The
 * day with the most in it is the slow one, which makes the wrong answer the interesting-looking one.
 */
suspend fun loadDayLog(
    sessionDao: SessionDao,
    cardioRepo: CardioRepository,
    date: LocalDate,
    zone: ZoneId = ZoneId.systemDefault()
): DayLog {
    val fromMs = date.atStartOfDay(zone).toInstant().toEpochMilli()
    val toMs = date.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
    val workouts = sessionDao.finishedInRange(fromMs, toMs).map { HistoryItem.Workout(it) }
    val cardio = cardioRepo.entriesInRange(fromMs, toMs).map { HistoryItem.Cardio(it) }
    return DayLog(date, (workouts + cardio).sortedByDescending { it.dateMs })
}

/** The day's log as a sheet: the date, what it amounts to, then the History rows themselves. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DayLogSheet(
    log: DayLog,
    onOpenSession: (Long) -> Unit,
    onOpenCardio: (Long) -> Unit,
    onDismiss: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val dayLine = remember(log.date) {
        log.date.format(DateTimeFormatter.ofPattern("EEEE · MMMM d, yyyy", Locale.getDefault())).uppercase()
    }
    val workouts = log.items.count { it is HistoryItem.Workout }
    val cardio = log.items.count { it is HistoryItem.Cardio }
    val summary = buildList {
        if (workouts > 0) add("$workouts workout${if (workouts == 1) "" else "s"}")
        if (cardio > 0) add("$cardio cardio session${if (cardio == 1) "" else "s"}")
    }.joinToString(" · ")

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = cs.background
    ) {
        // A keyed LazyColumn, not a Column: the sheet is a bounded modal, and a plain Column inside
        // it neither scrolled nor grew, so a day with more rows than fit (a dozen compact ones, fewer
        // at 200% font) simply cut off everything past the fold. Now the later rows are reachable.
        LazyColumn(
            modifier = Modifier.fillMaxWidth(),
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, bottom = 40.dp)
        ) {
            item("header") {
                Column(Modifier.fillMaxWidth()) {
                    Text(
                        dayLine,
                        style = MaterialTheme.typography.labelSmall,
                        color = cs.onSurfaceVariant,
                        fontSize = 9.sp,
                        letterSpacing = 1.sp
                    )
                    Spacer(Modifier.height(4.dp))
                    Text("That day", style = MaterialTheme.typography.headlineSmall, color = cs.onBackground)
                    if (summary.isNotEmpty()) {
                        Spacer(Modifier.height(2.dp))
                        Text(summary, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
                    }
                    Spacer(Modifier.height(14.dp))
                    EditorialHairline(outline = cs.outline)
                }
            }
            if (log.items.isEmpty()) {
                item("empty") {
                    InlineEmptyHint(
                        "Nothing logged on this day.",
                        color = cs.onSurfaceVariant,
                        modifier = Modifier.padding(vertical = 16.dp)
                    )
                }
            } else {
                items(log.items, key = { it.key }) { item ->
                    when (item) {
                        // The rows carry no date of their own any more — the sheet's own day line
                        // above already named the day, and it named it once.
                        is HistoryItem.Workout -> SessionRow(
                            session = item.session,
                            onClick = { onOpenSession(item.session.id) }
                        )
                        is HistoryItem.Cardio -> CardioHistoryRow(
                            entry = item.entry,
                            onClick = { onOpenCardio(item.entry.id) }
                        )
                    }
                }
            }
        }
    }
}
