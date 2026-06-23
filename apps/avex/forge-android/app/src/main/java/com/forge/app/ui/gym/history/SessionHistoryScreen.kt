package com.forge.app.ui.gym.history

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.Program
import com.forge.app.ui.common.EmptyState
import com.forge.app.ui.common.forgeItemMotion
import com.forge.app.ui.theme.LocalForgeSettings
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionHistoryScreen(
    onBack: () -> Unit,
    onOpenSession: (Long) -> Unit,
    onOpenCardio: (Long) -> Unit = {},
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
            SearchField(
                query = state.query,
                onQueryChange = viewModel::setQuery,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp)
            )

            // Filter row: tag chips (when any) + the duration / high-volume chips.
            LazyRow(
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                if (state.availableTags.isNotEmpty()) {
                    item {
                        FilterChip(
                            selected = state.tagFilter == null,
                            onClick = { viewModel.setTagFilter(null) },
                            label = { Text("All") },
                            colors = filterChipColors()
                        )
                    }
                    items(state.availableTags) { tag ->
                        FilterChip(
                            selected = state.tagFilter == tag,
                            onClick = { viewModel.setTagFilter(if (state.tagFilter == tag) null else tag) },
                            label = { Text("#$tag") },
                            colors = filterChipColors()
                        )
                    }
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
                val (title, subtitle) = if (state.anyFilterActive)
                    "No sessions match." to "Try a different search or clear a filter."
                else
                    "No sessions yet." to "Finish your first workout and it'll show up here."
                EmptyState(title = title, subtitle = subtitle, modifier = Modifier.padding(16.dp))
            } else {
                LazyColumn(
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 4.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    items(state.filtered, key = { it.key }) { item ->
                        when (item) {
                            is HistoryItem.Workout -> SessionRow(
                                session = item.session,
                                onClick = { onOpenSession(item.session.id) },
                                modifier = forgeItemMotion()
                            )
                            is HistoryItem.Cardio -> CardioHistoryRow(
                                entry = item.entry,
                                onClick = { onOpenCardio(item.entry.id) },
                                modifier = forgeItemMotion()
                            )
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SearchField(
    query: String,
    onQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    androidx.compose.material3.OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        modifier = modifier,
        placeholder = { Text("Search day, exercise or note…") },
        leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
        trailingIcon = {
            if (query.isNotEmpty()) {
                IconButton(onClick = { onQueryChange("") }) {
                    Icon(Icons.Filled.Close, contentDescription = "Clear search")
                }
            }
        },
        singleLine = true,
        shape = RoundedCornerShape(12.dp)
    )
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
    val tags = session.tags.split(",").map { it.trim() }.filter { it.isNotEmpty() }
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
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(dayName, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
                Text(
                    formatDate(session.startedAt),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                if (tags.isNotEmpty()) {
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        tags.take(4).forEach { TagChip(it) }
                    }
                }
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

@Composable
private fun CardioHistoryRow(
    entry: com.forge.app.data.db.entities.CardioEntry,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val type = com.forge.app.domain.cardio.CardioType.fromCode(entry.type)
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
            Row(modifier = Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Icon(type.icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(22.dp))
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(type.displayName, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
                    Text(formatDate(entry.date), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Column(horizontalAlignment = Alignment.End) {
                entry.distanceKm?.let {
                    Text(String.format(Locale.US, "%.1f km", it), style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.SemiBold)
                }
                if (entry.durationMin > 0) {
                    Text("${entry.durationMin}m", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

@Composable
private fun TagChip(text: String) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.12f))
            .padding(horizontal = 8.dp, vertical = 2.dp)
    ) {
        Text(
            "#$text",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.primary,
            fontSize = 9.sp,
            letterSpacing = 0.5.sp
        )
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
