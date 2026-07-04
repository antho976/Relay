package com.forge.app.ui.gym.session

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.IosShare
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.stats.components.statsEntrance

/**
 * Full-screen breakdown of a single finished training: a header that pairs the title/summary with a
 * compact muscle map, then a global Metric picker (weight / volume / reps / RPE) and a single card
 * for that metric. Weight/Volume/Reps render a tappable per-exercise comparison — tap a row to expand
 * that exercise's full detail; RPE merges into one chart with an exercise selector. The bars/line
 * style is now per-card (held here so each metric keeps its own choice but resets on leave).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionDetailScreen(
    onBack: () -> Unit,
    viewModel: SessionDetailViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val exportPath by viewModel.exportPath.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    // When the per-session JSON has been written, open the system share sheet on it (Save to Files /
    // Drive / send) — the same path Settings' exports use. Mirror of SettingsScreen's export effect.
    exportPath?.let { path ->
        LaunchedEffect(path) {
            val shared = runCatching {
                val file = java.io.File(path)
                val uri = androidx.core.content.FileProvider.getUriForFile(
                    context, "${context.packageName}.fileprovider", file
                )
                val send = android.content.Intent(android.content.Intent.ACTION_SEND).apply {
                    type = "application/json"
                    putExtra(android.content.Intent.EXTRA_STREAM, uri)
                    addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
                }
                context.startActivity(android.content.Intent.createChooser(send, "Save or share session"))
            }.isSuccess
            // Tell the user if the share sheet couldn't open instead of silently swallowing it. The path
            // is cleared either way so the one-shot doesn't loop; tapping export again re-fires cleanly.
            if (!shared) {
                android.widget.Toast.makeText(
                    context, "Couldn't open the share sheet.", android.widget.Toast.LENGTH_SHORT
                ).show()
            }
            viewModel.clearExportPath()
        }
    }

    var metric by rememberSaveable { mutableStateOf(SessionMetric.WEIGHT) }
    // Bars/line is per-stat now — each metric carries its own style instead of one page-wide switch.
    var weightStyle by rememberSaveable { mutableStateOf(SessionChartStyle.BARS) }
    var volumeStyle by rememberSaveable { mutableStateOf(SessionChartStyle.BARS) }
    var repsStyle by rememberSaveable { mutableStateOf(SessionChartStyle.BARS) }
    var rpeStyle by rememberSaveable { mutableStateOf(SessionChartStyle.BARS) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { com.forge.app.ui.common.ForgeWordmark() },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                actions = {
                    // Save just this session's data as a JSON file (opens the share sheet).
                    if (state.data != null) {
                        IconButton(onClick = { viewModel.exportSession() }) {
                            Icon(Icons.Default.IosShare, contentDescription = "Save this session as JSON", tint = muted)
                        }
                    }
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
            // A missing row (stale/broken link) is a different state from a real session that simply
            // logged nothing — say so distinctly instead of one catch-all message.
            data == null -> Box(Modifier.fillMaxSize().padding(inner), contentAlignment = Alignment.Center) {
                Text(
                    "Session not found.",
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
                item("header") { Box(Modifier.statsEntrance(0)) { SessionHeaderBlock(data, onBg, muted, accent, outline) } }
                if (data.exercises.isEmpty()) {
                    // Session exists but every exercise was skipped or logged no sets — keep its header
                    // and just note there's nothing to chart.
                    item("empty") {
                        Box(Modifier.statsEntrance(1)) {
                            Text(
                                "No exercises logged for this session.",
                                style = MaterialTheme.typography.bodySmall,
                                color = muted,
                                fontStyle = FontStyle.Italic
                            )
                        }
                    }
                } else {
                    item("metric") {
                        Box(Modifier.statsEntrance(1)) {
                            SegmentRow(
                                items = SessionMetric.entries,
                                isSelected = { it == metric },
                                label = { it.label },
                                onSelect = { metric = it },
                                onBg = onBg, muted = muted, accent = accent, outline = outline
                            )
                        }
                    }
                    item("metric-card") {
                        Box(Modifier.statsEntrance(2)) {
                            when (metric) {
                                SessionMetric.RPE -> RpeExerciseCard(
                                    data.exercises, rpeStyle, { rpeStyle = it }, onBg, muted, accent, outline
                                )
                                else -> {
                                    val style = when (metric) {
                                        SessionMetric.WEIGHT -> weightStyle
                                        SessionMetric.VOLUME -> volumeStyle
                                        else -> repsStyle
                                    }
                                    MetricExerciseCard(
                                        exercises = data.exercises,
                                        metric = metric,
                                        style = style,
                                        onStyle = { newStyle ->
                                            when (metric) {
                                                SessionMetric.WEIGHT -> weightStyle = newStyle
                                                SessionMetric.VOLUME -> volumeStyle = newStyle
                                                else -> repsStyle = newStyle
                                            }
                                        },
                                        onBg = onBg, muted = muted, accent = accent, outline = outline
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
