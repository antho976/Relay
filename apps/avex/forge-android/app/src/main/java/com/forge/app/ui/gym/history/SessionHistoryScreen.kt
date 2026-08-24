package com.forge.app.ui.gym.history

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.forgeItemMotion
import com.forge.app.ui.theme.LocalForgeSettings

/**
 * History — the "view all →" destination behind Home's RECENT trim. List archetype (DESIGN §3):
 * search-first, trim rows, a tiny hero of title plus two figures, no charts and no theatrics.
 *
 * ## The rebuild (2026-08-24)
 *
 * The screen was a flat run of rows, each printing its own full date and closing on a hairline. Three
 * things were wrong with that and all three are structural, not cosmetic:
 *
 *  - **The date was the loudest thing on every row and the only thing they shared.** Seven sessions
 *    on one Monday rendered "AUG 24, 2026" seven times. The date is a property of the DAY, so the
 *    day owns it now and says it once ([HistoryDay]).
 *  - **The hairlines were a §1 violation.** A line is a claim about data; a row separator is not
 *    data. Air and the date anchors carry the structure instead.
 *  - **The screen answered nothing.** A log you scroll should tell you how much of it there is. The
 *    two figures under the title read the CURRENT filter, so tapping "Heavy" is answered by the
 *    numbers moving rather than by a list you have to count.
 *
 * The "All" pill is now always present. It used to appear only for users who had tagged a session,
 * which meant everyone else could turn a filter on and had no drawn way to turn it back off.
 */
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
    val weightUnit = LocalForgeSettings.current.weightUnit

    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6: back only, never the screen's name — the serif hero below carries it.
                title = {},
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(modifier = Modifier.fillMaxSize().padding(inner)) {
            // ── Tiny hero: the screen names itself in content, then reads itself ──────────
            Text(
                "History",
                style = MaterialTheme.typography.headlineSmall,
                color = cs.onBackground,
                modifier = Modifier
                    .padding(horizontal = 24.dp)
                    .semantics { heading() }
            )
            Spacer(Modifier.height(12.dp))
            Row(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(20.dp)
            ) {
                // Honest zeros, never a dash and never hidden (§12) — at zero this is the shape the
                // screen will keep once there is data in it.
                EditorialFigure(
                    value = "${state.summary.sessions}",
                    label = if (state.summary.sessions == 1) "Session" else "Sessions",
                    onBg = cs.onBackground, muted = cs.onSurfaceVariant, accent = cs.primary
                )
                EditorialFigure(
                    value = formatVolumeCompact(state.summary.volumeLb, weightUnit),
                    label = "Volume",
                    onBg = cs.onBackground, muted = cs.onSurfaceVariant, accent = cs.primary
                )
            }

            Spacer(Modifier.height(18.dp))
            // ── Search field — interactive control, keep bordered (§13) ──────────
            SearchField(
                query = state.query,
                onQueryChange = viewModel::setQuery,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp)
            )

            // ── Filter pills — the shared SegmentPill, one short word each (§4) ────
            LazyRow(
                contentPadding = PaddingValues(horizontal = 24.dp, vertical = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                item {
                    // Always drawn, so a filter can always be taken back off — not only by the
                    // users who happen to have tagged a session.
                    HistoryFilterPill("All", !state.anyPillActive) { viewModel.clearPillFilters() }
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
                items(state.availableTags) { tag ->
                    HistoryFilterPill("#$tag", state.tagFilter == tag) {
                        viewModel.setTagFilter(if (state.tagFilter == tag) null else tag)
                    }
                }
            }

            if (state.isEmpty) {
                // Quiet italic hint — no boxed empty-state card (§12).
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
                LazyColumn(contentPadding = PaddingValues(start = 24.dp, end = 24.dp, bottom = 32.dp)) {
                    state.days.forEachIndexed { index, day ->
                        item(key = "day:${day.label}", contentType = "day") {
                            DayAnchor(day.label, first = index == 0, modifier = forgeItemMotion())
                        }
                        items(day.items, key = { it.key }, contentType = { "row" }) { item ->
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
}

/**
 * The date a run of rows shares. Mono and muted, one rung BELOW the sans row titles it introduces —
 * a date is where a session sat, not what it was, and it should never out-shout the sessions (§6).
 */
@Composable
private fun DayAnchor(label: String, first: Boolean, modifier: Modifier = Modifier) {
    Column(modifier) {
        Spacer(Modifier.height(if (first) 4.dp else 22.dp))
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.semantics { heading() }
        )
        Spacer(Modifier.height(8.dp))
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
