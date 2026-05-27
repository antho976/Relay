package com.forge.app.ui.cardio

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.cardio.CardioType
import com.forge.app.ui.cardio.components.CardioEntryRow
import com.forge.app.ui.cardio.components.CardioLogSheet
import com.forge.app.ui.cardio.state.CardioUiState
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.WeekFields
import java.util.Locale

@Composable
fun CardioScreen(
    onBack: () -> Unit,
    viewModel: CardioViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    if (state.sheetOpen) {
        CardioLogSheet(
            onDismiss = viewModel::closeSheet,
            onLog = viewModel::logEntry
        )
    } else {
        CardioListContent(
            state = state,
            onBack = onBack,
            onOpenLog = viewModel::openSheet,
            onRequestDelete = viewModel::requestDelete
        )
    }

    if (state.pendingDeleteId != null) {
        AlertDialog(
            onDismissRequest = viewModel::cancelDelete,
            title = { Text("Delete entry?") },
            text = { Text("This can't be undone.") },
            confirmButton = { TextButton(onClick = viewModel::confirmDelete) { Text("Delete") } },
            dismissButton = { TextButton(onClick = viewModel::cancelDelete) { Text("Cancel") } }
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CardioListContent(
    state: CardioUiState,
    onBack: () -> Unit,
    onOpenLog: () -> Unit,
    onRequestDelete: (Long) -> Unit
) {
    val zone = ZoneId.systemDefault()
    val today = LocalDate.now(zone)
    val weekNum = today.get(WeekFields.ISO.weekOfWeekBasedYear())
    val isoWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
    val isoWeekEnd = isoWeekStart.plusDays(6)
    val weekLabel = remember(weekNum) {
        val fmt = DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())
        "${isoWeekStart.format(fmt).uppercase()} — ${isoWeekEnd.format(fmt).uppercase()}"
    }
    val todayDow = today.dayOfWeek.value - 1
    val isoWeekStartMs = remember(isoWeekStart) {
        isoWeekStart.atStartOfDay(zone).toInstant().toEpochMilli()
    }
    val weekEntries = remember(state.entries, isoWeekStartMs) {
        state.entries.filter { it.date >= isoWeekStartMs && it.type != CardioType.REST.code }
    }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Text("•", style = MaterialTheme.typography.bodyMedium, color = muted)
                        Text("Forge", style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic)
                    }
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                actions = {
                    Text(
                        "CARDIO",
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 2.sp,
                        color = muted,
                        modifier = Modifier.padding(end = 16.dp)
                    )
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(bottom = 56.dp)
        ) {
            item("hero") {
                CardioHero(
                    weekMinutes = state.weekMinutes,
                    weekNum = weekNum,
                    weekLabel = weekLabel,
                    weekEntries = weekEntries,
                    zone = zone,
                    onBg = onBg,
                    muted = muted
                )
            }

            item("week-row") {
                WeekBoxRow(
                    dailyMinutes = state.weekDailyMinutes,
                    todayDow = todayDow,
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )
            }

            item("log-action") {
                LogTodayRow(onOpenLog = onOpenLog, onBg = onBg, muted = muted, outline = outline)
                Column(Modifier.padding(horizontal = 24.dp)) {
                    HorizontalDivider(color = outline.copy(alpha = 0.25f))
                    Spacer(Modifier.height(16.dp))
                }
            }

            if (state.filteredEntries.isNotEmpty()) {
                item("history-title") {
                    Text(
                        "What I did",
                        style = MaterialTheme.typography.headlineSmall,
                        color = onBg,
                        fontStyle = FontStyle.Italic,
                        modifier = Modifier.padding(horizontal = 24.dp)
                    )
                    Spacer(Modifier.height(12.dp))
                }
                items(state.filteredEntries, key = { it.id }) { entry ->
                    CardioEntryRow(
                        entry = entry,
                        today = today,
                        onRequestDelete = { onRequestDelete(entry.id) }
                    )
                    HorizontalDivider(
                        modifier = Modifier.padding(horizontal = 24.dp),
                        color = outline.copy(alpha = 0.18f)
                    )
                }
            } else if (!state.isLoading) {
                item("empty") {
                    Text(
                        "No activity logged yet.",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted.copy(alpha = 0.55f),
                        fontStyle = FontStyle.Italic,
                        modifier = Modifier.padding(horizontal = 24.dp)
                    )
                }
            }
        }
    }
}

