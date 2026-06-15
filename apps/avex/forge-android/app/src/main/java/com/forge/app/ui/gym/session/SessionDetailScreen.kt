package com.forge.app.ui.gym.session

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.stats.components.StatCard
import com.forge.app.ui.gym.stats.components.statsEntrance

/**
 * Full-screen breakdown of a single finished training: header + summary strip, then a session
 * overview chart and a card per exercise (set table + a per-exercise graph). The Metric (weight /
 * volume / reps) and Style (bars / line) toggles restyle every chart at once; they're held here as
 * UI state so they reset when you leave.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionDetailScreen(
    onBack: () -> Unit,
    viewModel: SessionDetailViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    var metric by rememberSaveable { mutableStateOf(SessionMetric.WEIGHT) }
    var style by rememberSaveable { mutableStateOf(SessionChartStyle.BARS) }

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
                        "SESSION",
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
        val data = state.data
        when {
            state.isLoading -> Box(Modifier.fillMaxSize().padding(inner), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(color = accent)
            }
            data == null || data.exercises.isEmpty() -> Box(Modifier.fillMaxSize().padding(inner), contentAlignment = Alignment.Center) {
                Text(
                    "Nothing logged for this session.",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted,
                    fontStyle = FontStyle.Italic
                )
            }
            else -> LazyColumn(
                modifier = Modifier.fillMaxSize().padding(inner),
                contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 56.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                item("header") { Box(Modifier.statsEntrance(0)) { SessionHeader(data, onBg, muted, outline) } }
                item("summary") { Box(Modifier.statsEntrance(1)) { SummaryStrip(data, onBg, muted) } }
                item("controls") {
                    Box(Modifier.statsEntrance(2)) {
                        MetricStyleControls(metric, style, { metric = it }, { style = it }, onBg, muted, accent, outline)
                    }
                }
                item("overview") {
                    Box(Modifier.statsEntrance(3)) {
                        StatCard(title = "${metric.label.uppercase()} PER EXERCISE") {
                            MetricByExerciseChart(data.exercises, metric, onBg, muted, accent, outline)
                        }
                    }
                }
                itemsIndexed(data.exercises) { i, ex ->
                    Box(Modifier.statsEntrance(4 + i)) {
                        ExerciseCard(ex, metric, style, onBg, muted, accent, outline)
                    }
                }
            }
        }
    }
}
