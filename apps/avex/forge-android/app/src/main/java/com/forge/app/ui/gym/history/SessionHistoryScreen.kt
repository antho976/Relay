package com.forge.app.ui.gym.history

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import com.forge.app.ui.common.forgeItemMotion
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.domain.mood.Mood
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.Program
import com.forge.app.ui.common.EmptyState
import com.forge.app.ui.theme.LocalForgeSettings
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionHistoryScreen(
    onBack: () -> Unit,
    onOpenSession: (Long) -> Unit,
    viewModel: SessionHistoryViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val useKg = LocalForgeSettings.current.useKg

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("HISTORY", style = MaterialTheme.typography.headlineLarge) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, null) } },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(modifier = Modifier.fillMaxSize().padding(inner)) {
            // Filter row
            LazyRow(
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                item {
                    FilterChip(
                        selected = state.moodFilter == null,
                        onClick = { viewModel.setMoodFilter(null) },
                        label = { Text("All moods") },
                        colors = filterChipColors()
                    )
                }
                items(Mood.entries) { mood ->
                    FilterChip(
                        selected = state.moodFilter == mood,
                        onClick = { viewModel.setMoodFilter(mood) },
                        label = { Text("${mood.emoji} ${mood.displayName}") },
                        colors = filterChipColors()
                    )
                }
                item {
                    FilterChip(
                        selected = state.durationFilter == SessionHistoryFilter.SHORT,
                        onClick = { viewModel.setDurationFilter(if (state.durationFilter == SessionHistoryFilter.SHORT) null else SessionHistoryFilter.SHORT) },
                        label = { Text("< 45 min") },
                        colors = filterChipColors()
                    )
                }
                item {
                    FilterChip(
                        selected = state.durationFilter == SessionHistoryFilter.LONG,
                        onClick = { viewModel.setDurationFilter(if (state.durationFilter == SessionHistoryFilter.LONG) null else SessionHistoryFilter.LONG) },
                        label = { Text("> 60 min") },
                        colors = filterChipColors()
                    )
                }
                item {
                    FilterChip(
                        selected = state.volumeFilter == SessionHistoryFilter.HIGH_VOLUME,
                        onClick = { viewModel.setVolumeFilter(if (state.volumeFilter == SessionHistoryFilter.HIGH_VOLUME) null else SessionHistoryFilter.HIGH_VOLUME) },
                        label = { Text("> ${toDisplayWeight(HIGH_VOLUME_LB, useKg).toInt()} ${unitLabel(useKg)}") },
                        colors = filterChipColors()
                    )
                }
            }

            if (state.filtered.isEmpty()) {
                val filtersActive = state.moodFilter != null || state.durationFilter != null || state.volumeFilter != null
                val (emoji, title, subtitle) = if (filtersActive)
                    Triple("📋", "No sessions match.", "Try clearing a filter.")
                else
                    Triple("🏋️", "No sessions yet.", "Finish your first workout and it'll show up here.")
                EmptyState(emoji = emoji, title = title, subtitle = subtitle, modifier = Modifier.padding(16.dp))
            } else {
                LazyColumn(
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 4.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    items(state.filtered, key = { it.id }) { session ->
                        SessionRow(
                            session = session,
                            onClick = { onOpenSession(session.id) },
                            modifier = forgeItemMotion()
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun SessionRow(
    session: com.forge.app.data.db.entities.Session,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val dayName = Program.dayDisplayName(session.dayKey)
    val durationMin = session.durationMinutes()
    val useKg = LocalForgeSettings.current.useKg
    Surface(
        modifier = modifier.fillMaxWidth().clickable { onClick() },
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(12.dp)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Column {
                Text(dayName, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
                Text(
                    formatDate(session.startedAt),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            Column(horizontalAlignment = Alignment.End) {
                if (session.totalVolumeLb != null && session.totalVolumeLb > 0) {
                    Text(formatVolume(session.totalVolumeLb, useKg), style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.SemiBold)
                }
                if (durationMin != null) {
                    Text("${durationMin}m", style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun filterChipColors() = FilterChipDefaults.filterChipColors(
    selectedContainerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f),
    selectedLabelColor = MaterialTheme.colorScheme.primary
)

private val dateFormat = SimpleDateFormat("MMM d, yyyy", Locale.getDefault())
private fun formatDate(epochMs: Long) = dateFormat.format(Date(epochMs))
