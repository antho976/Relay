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
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
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
import com.forge.app.ui.common.ForgeChoiceChip
import com.forge.app.ui.common.ForgeSlidingSegments
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.forgeItemMotion
import com.forge.app.ui.gym.train.components.GymSearchRow
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
 * 2026-09-27: search is the filled rounded row and the duration filter a sliding segment. The rows
 * stay plain under their date; a filled group per day was tried and read as a stack of cards.
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
        topBar = { ForgeTopBar(onBack = onBack) },
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
            // ── Search: the filled rounded search row ──────────
            GymSearchRow(
                query = viewModel.queryText,
                onQueryChange = viewModel::setQuery,
                placeholder = "Search day, exercise or note",
                modifier = Modifier.padding(horizontal = 24.dp)
            )

            // ── Duration is one pick of three, so it is a sliding segment. "All" still takes every
            // pill filter back off, as it always has.
            Spacer(Modifier.height(12.dp))
            ForgeSlidingSegments(
                options = listOf("All", "Short", "Long"),
                selectedIndex = when (state.durationFilter) {
                    SessionHistoryFilter.SHORT -> 1
                    SessionHistoryFilter.LONG -> 2
                    else -> 0
                },
                onSelect = { i ->
                    when (i) {
                        1 -> viewModel.setDurationFilter(SessionHistoryFilter.SHORT)
                        2 -> viewModel.setDurationFilter(SessionHistoryFilter.LONG)
                        else -> viewModel.clearPillFilters()
                    }
                },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp)
            )

            // Heavy and the user's own tags combine with the duration pick, so they stay toggles.
            LazyRow(
                contentPadding = PaddingValues(horizontal = 24.dp, vertical = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                item {
                    ForgeChoiceChip("Heavy", state.volumeFilter == SessionHistoryFilter.HIGH_VOLUME, {
                        viewModel.setVolumeFilter(if (state.volumeFilter == SessionHistoryFilter.HIGH_VOLUME) null else SessionHistoryFilter.HIGH_VOLUME)
                    })
                }
                items(state.availableTags) { tag ->
                    ForgeChoiceChip("#$tag", state.tagFilter == tag, {
                        viewModel.setTagFilter(if (state.tagFilter == tag) null else tag)
                    })
                }
            }

            if (!state.loaded) {
                // Nothing until the first read lands: "No sessions yet" flashed for a frame or two
                // on every open, before the list it claimed was missing appeared.
            } else if (state.isEmpty) {
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
                // Plain rows under each day's date: a log reads as a list, not a stack of cards
                // (the grouped-rows pass was reverted here on 2026-09-27).
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
            modifier = Modifier.padding(horizontal = 4.dp).semantics { heading() }
        )
        Spacer(Modifier.height(8.dp))
    }
}
