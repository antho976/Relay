package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.stats.components.FrequencyHeatmap
import com.forge.app.ui.gym.stats.components.PrTimelineList
import com.forge.app.ui.gym.stats.components.StrengthCurveCard
import com.forge.app.ui.gym.stats.components.TotalsRow
import com.forge.app.ui.gym.stats.components.VolumeByMuscleChart

/**
 * The Stats subtab content. Lives inside DayListScreen's tab pager; no top bar of its
 * own — the parent screen owns navigation chrome.
 */
@Composable
fun StatsContent(
    modifier: Modifier = Modifier,
    viewModel: StatsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    LazyColumn(
        modifier = modifier,
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        item("totals") { TotalsRow(totals = state.totals) }
        item("heatmap") { FrequencyHeatmap(cells = state.heatmap) }
        item("volume") { VolumeByMuscleChart(rows = state.volumeByMuscle) }
        item("curve") { StrengthCurveCard(curve = state.strengthCurve) }
        item("pr-header") { PrTimelineList(entries = state.recentPrs) }
    }
}
