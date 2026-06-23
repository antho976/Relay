package com.forge.app.ui.recap

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
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
import androidx.compose.ui.draw.clip
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.ui.common.forgeShimmer
import com.forge.app.ui.theme.LocalForgeSettings

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RecapScreen(
    onBack: () -> Unit,
    viewModel: RecapViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val useKg = LocalForgeSettings.current.useKg

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("RECAP", style = MaterialTheme.typography.headlineLarge) },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, null) }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        if (state.isLoading) {
            // A skeleton that mirrors the two recap cards anchors the layout, so content swaps in
            // without a jump — beats a generic spinner that teaches the user nothing (#404).
            RecapSkeleton(Modifier.fillMaxSize().padding(inner).padding(horizontal = 16.dp, vertical = 8.dp))
            return@Scaffold
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Monthly recap card (#32)
            state.monthRecap?.let { recap ->
                RecapCard(
                    title = "THIS MONTH · ${recap.month.month.name.lowercase().replaceFirstChar { it.uppercase() }} ${recap.month.year}",
                    emoji = "📅"
                ) {
                    BigStat("${recap.sessionCount}", "workouts")
                    BigStat(formatRecapVolume(recap.totalVolumeLb, useKg), "total volume")
                    BigStat("${recap.totalPrs}", "PRs")
                    BigStat("${recap.totalSets}", "sets logged")
                    if (recap.topExercise != null) RecapRow("Most trained", recap.topExercise)
                    if (recap.avgDurationMin > 0) RecapRow("Avg session", "${recap.avgDurationMin} min")
                    if (recap.bestDayName != null) RecapRow("Best PR day", recap.bestDayName)
                }
            } ?: Text("No sessions this month yet.", style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)

            // Year-over-year recap card (#33)
            state.yearRecap?.let { recap ->
                RecapCard(
                    title = "${recap.year} IN REVIEW",
                    emoji = "🏆"
                ) {
                    BigStat("${recap.sessionCount}", "workouts")
                    BigStat(formatRecapVolume(recap.totalVolumeLb, useKg), "total volume")
                    BigStat("${recap.totalPrs}", "total PRs")
                    BigStat("${recap.longestStreak}d", "longest streak")
                    if (recap.avgWeeklyVolume > 0) RecapRow("Avg weekly volume", formatRecapVolume(recap.avgWeeklyVolume, useKg))
                    if (recap.topExercise != null) RecapRow("Most trained exercise", recap.topExercise)
                    if (recap.bestMonthName != null) RecapRow("Best month", recap.bestMonthName)
                }
            } ?: Text("No sessions this year yet.", style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)

            Spacer(Modifier.height(16.dp))
        }
    }
}

/** Loading placeholder mirroring the two recap cards (title · big stats · rows) — uses the shared
 *  [forgeShimmer] so the populated content swaps in without a layout jump (#404). */
@Composable
private fun RecapSkeleton(modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(16.dp)) {
        repeat(2) {
            Column(
                Modifier.fillMaxWidth()
                    .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(16.dp))
                    .padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp)
            ) {
                Box(Modifier.width(180.dp).height(18.dp).clip(RoundedCornerShape(50)).forgeShimmer())
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    repeat(4) {
                        Box(Modifier.width(56.dp).height(40.dp).clip(RoundedCornerShape(8.dp)).forgeShimmer())
                    }
                }
                repeat(3) {
                    Box(Modifier.fillMaxWidth().height(12.dp).clip(RoundedCornerShape(50)).forgeShimmer())
                }
            }
        }
    }
}

@Composable
private fun RecapCard(title: String, emoji: String, content: @Composable () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(16.dp))
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(emoji, style = MaterialTheme.typography.headlineMedium)
            Text(title, style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Black)
        }
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            content()
        }
    }
}

@Composable
private fun BigStat(value: String, label: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(value, style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Black, color = MaterialTheme.colorScheme.onSurface)
        Text(label, style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Volume for the recap cards: "12.5k lb" for big numbers, the exact value under 1000 lb (no "0k"). */
private fun formatRecapVolume(lb: Double, useKg: Boolean): String = formatVolumeCompact(lb, useKg)

@Composable
private fun RecapRow(label: String, value: String) {
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value, style = MaterialTheme.typography.bodySmall,
            fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onSurface)
    }
}
