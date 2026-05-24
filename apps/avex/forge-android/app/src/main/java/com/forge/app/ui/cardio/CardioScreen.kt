package com.forge.app.ui.cardio

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import com.forge.app.domain.cardio.CardioType
import com.forge.app.ui.cardio.components.CardioEntryRow
import com.forge.app.ui.cardio.components.CardioLogSheet
import com.forge.app.ui.cardio.components.WeeklyCardioCard
import com.forge.app.ui.cardio.state.PaceTrendPoint

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CardioScreen(
    onBack: () -> Unit,
    viewModel: CardioViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("CARDIO", style = MaterialTheme.typography.headlineLarge) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    titleContentColor = MaterialTheme.colorScheme.onBackground
                )
            )
        },
        containerColor = MaterialTheme.colorScheme.background
    ) { inner ->
        Box(Modifier.fillMaxSize().padding(inner)) {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                item(key = "summary") {
                    WeeklyCardioCard(
                        minutes = state.weekMinutes,
                        entries = state.weekEntryCount,
                        weekDailyMinutes = state.weekDailyMinutes
                    )
                }
                // Lifetime distance + pace trend (#79, #78)
                if (state.lifetimeDistanceKm > 0) {
                    item(key = "lifetime") {
                        CardioAnalyticsCard(
                            lifetimeKm = state.lifetimeDistanceKm,
                            paceTrend = state.paceTrend
                        )
                    }
                }
                item(key = "filter") {
                    TypeFilterRow(
                        selected = state.selectedTypeFilter,
                        onSelect = viewModel::setTypeFilter
                    )
                }
                item(key = "add") {
                    Button(
                        onClick = viewModel::openSheet,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 4.dp, bottom = 4.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = MaterialTheme.colorScheme.primary
                        )
                    ) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                        Text("  Log cardio")
                    }
                }
                if (state.filteredEntries.isEmpty() && !state.isLoading) {
                    item(key = "empty") {
                        EmptyState()
                    }
                }
                items(items = state.filteredEntries, key = { it.id }) { entry ->
                    CardioEntryRow(
                        entry = entry,
                        onRequestDelete = { viewModel.requestDelete(entry.id) }
                    )
                }
            }
        }
    }

    if (state.sheetOpen) {
        CardioLogSheet(
            onDismiss = viewModel::closeSheet,
            onLog = viewModel::logEntry
        )
    }

    val pendingDelete = state.pendingDeleteId
    if (pendingDelete != null) {
        DeleteConfirmDialog(
            onConfirm = viewModel::confirmDelete,
            onDismiss = viewModel::cancelDelete
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TypeFilterRow(selected: String?, onSelect: (String?) -> Unit) {
    Row(
        modifier = androidx.compose.ui.Modifier
            .horizontalScroll(rememberScrollState())
            .padding(bottom = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        FilterChip(
            selected = selected == null,
            onClick = { onSelect(null) },
            label = { Text("All") },
            colors = FilterChipDefaults.filterChipColors(
                selectedContainerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f),
                selectedLabelColor = MaterialTheme.colorScheme.primary
            )
        )
        CardioType.entries.forEach { type ->
            FilterChip(
                selected = selected == type.code,
                onClick = { onSelect(type.code) },
                label = { Text(type.displayName) },
                colors = FilterChipDefaults.filterChipColors(
                    selectedContainerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f),
                    selectedLabelColor = MaterialTheme.colorScheme.primary
                )
            )
        }
    }
}

@Composable
private fun EmptyState() {
    com.forge.app.ui.common.EmptyState(
        emoji = "🏃",
        title = "No cardio yet.",
        subtitle = "That's on you."
    )
}

@Composable
private fun CardioAnalyticsCard(lifetimeKm: Double, paceTrend: List<PaceTrendPoint>) {
    val primary = MaterialTheme.colorScheme.primary
    val surfaceVar = MaterialTheme.colorScheme.surfaceVariant
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(surfaceVar, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Text("CARDIO ANALYTICS", style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant, fontWeight = FontWeight.SemiBold)
        Text(
            "Lifetime: ${"%.1f".format(lifetimeKm)} km",
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onSurface,
            fontWeight = FontWeight.Black
        )
        if (paceTrend.size >= 2) {
            Text("RUN PACE TREND · min/km", style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            val minPace = paceTrend.minOf { it.paceMinPerKm }
            val maxPace = paceTrend.maxOf { it.paceMinPerKm }.coerceAtLeast(minPace + 0.01)
            val range = maxPace - minPace
            Canvas(modifier = Modifier.fillMaxWidth().height(60.dp).clip(RoundedCornerShape(4.dp))) {
                val step = size.width / (paceTrend.size - 1).coerceAtLeast(1)
                val path = Path()
                paceTrend.forEachIndexed { i, pt ->
                    val x = i * step
                    val y = size.height - ((pt.paceMinPerKm - minPace) / range * size.height).toFloat()
                    if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
                }
                drawPath(path, color = primary, style = Stroke(width = 3f))
                // dots
                paceTrend.forEachIndexed { i, pt ->
                    val x = i * step
                    val y = size.height - ((pt.paceMinPerKm - minPace) / range * size.height).toFloat()
                    drawCircle(primary, radius = 4f, center = Offset(x, y))
                }
            }
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text("${"%.1f".format(paceTrend.first().paceMinPerKm)} min/km",
                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text("${"%.1f".format(paceTrend.last().paceMinPerKm)} min/km",
                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun DeleteConfirmDialog(
    onConfirm: () -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Delete entry?") },
        text = { Text("This can't be undone.") },
        confirmButton = {
            TextButton(onClick = onConfirm) { Text("Delete") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}

