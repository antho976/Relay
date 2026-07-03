package com.forge.app.ui.gym.history

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
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
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.EditorialHairline
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.forgeItemMotion
import com.forge.app.ui.theme.LocalForgeSettings

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
    val cs = MaterialTheme.colorScheme

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
            // ── Search field — interactive control, keep bordered ──────────
            SearchField(
                query = state.query,
                onQueryChange = viewModel::setQuery,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp)
            )

            // ── Filter chips — interactive controls, keep bordered/pill ────
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

            // ── Top hairline before the list ──────────────────────────────
            EditorialHairline(outline = cs.outline)

            if (state.filtered.isEmpty()) {
                // Quiet italic hint — no boxed empty-state card
                val hint = if (state.anyFilterActive)
                    "No sessions match. Try a different search or clear a filter."
                else
                    "No sessions yet. Finish your first workout and it'll show up here."
                InlineEmptyHint(
                    text = hint,
                    color = cs.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 20.dp)
                )
            } else {
                LazyColumn(
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 0.dp)
                ) {
                    items(state.filtered, key = { it.key }) { item ->
                        when (item) {
                            is HistoryItem.Workout -> SessionRow(
                                session = item.session,
                                onClick = { onOpenSession(item.session.id) },
                                outline = cs.outline,
                                modifier = forgeItemMotion()
                            )
                            is HistoryItem.Cardio -> CardioHistoryRow(
                                entry = item.entry,
                                onClick = { onOpenCardio(item.entry.id) },
                                outline = cs.outline,
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
    OutlinedTextField(
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun filterChipColors() = FilterChipDefaults.filterChipColors(
    selectedContainerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f),
    selectedLabelColor = MaterialTheme.colorScheme.primary
)
