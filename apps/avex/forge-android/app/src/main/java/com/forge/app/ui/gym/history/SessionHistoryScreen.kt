package com.forge.app.ui.gym.history

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.forgeItemMotion

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionHistoryScreen(
    onBack: () -> Unit,
    onOpenSession: (Long) -> Unit,
    onOpenCardio: (Long) -> Unit = {},
    viewModel: SessionHistoryViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val cs = MaterialTheme.colorScheme

    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6: bell + back, never the screen's name — the serif hero below carries it.
                title = {},
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(modifier = Modifier.fillMaxSize().padding(inner)) {
            // List archetype (§3): a tiny serif hero names the screen in content.
            Text(
                "History",
                style = MaterialTheme.typography.headlineSmall,
                color = cs.onBackground,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
            Spacer(Modifier.height(8.dp))
            // ── Search field — interactive control, keep bordered ──────────
            SearchField(
                query = state.query,
                onQueryChange = viewModel::setQuery,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 4.dp)
            )

            // ── Filter pills — the shared SegmentPill, one short word each (§4) ────
            LazyRow(
                contentPadding = PaddingValues(horizontal = 24.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                if (state.availableTags.isNotEmpty()) {
                    item {
                        HistoryFilterPill("All", state.tagFilter == null) { viewModel.setTagFilter(null) }
                    }
                    items(state.availableTags) { tag ->
                        HistoryFilterPill("#$tag", state.tagFilter == tag) {
                            viewModel.setTagFilter(if (state.tagFilter == tag) null else tag)
                        }
                    }
                }
                item {
                    HistoryFilterPill("Short", state.durationFilter == SessionHistoryFilter.SHORT) {
                        viewModel.setDurationFilter(if (state.durationFilter == SessionHistoryFilter.SHORT) null else SessionHistoryFilter.SHORT)
                    }
                }
                item {
                    HistoryFilterPill("Long", state.durationFilter == SessionHistoryFilter.LONG) {
                        viewModel.setDurationFilter(if (state.durationFilter == SessionHistoryFilter.LONG) null else SessionHistoryFilter.LONG)
                    }
                }
                item {
                    HistoryFilterPill("Heavy", state.volumeFilter == SessionHistoryFilter.HIGH_VOLUME) {
                        viewModel.setVolumeFilter(if (state.volumeFilter == SessionHistoryFilter.HIGH_VOLUME) null else SessionHistoryFilter.HIGH_VOLUME)
                    }
                }
            }

            if (state.filtered.isEmpty()) {
                // Quiet italic hint — no boxed empty-state card
                val hint = if (state.anyFilterActive)
                    "No sessions match. Try a different search or clear a filter."
                else
                    "No sessions yet. Finish your first workout and it'll show up here."
                InlineEmptyHint(
                    text = hint,
                    color = cs.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 20.dp)
                )
            } else {
                LazyColumn(
                    contentPadding = PaddingValues(horizontal = 24.dp, vertical = 0.dp)
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

/** One home for the screen's filter pills — all route through the shared [SegmentPill]. */
@Composable
private fun HistoryFilterPill(text: String, selected: Boolean, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    SegmentPill(
        text = text,
        selected = selected,
        onClick = onClick,
        accent = cs.primary,
        onBg = cs.onBackground,
        muted = cs.onSurfaceVariant,
        outline = cs.outline
    )
}
