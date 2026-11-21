package com.forge.app.ui.gym.session

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenTabs
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionDetailUiState
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

/**
 * One finished session (2026-10-21 "Open panel" redesign, designs `history_hd/hdv/hdr/hde`). The page
 * Antho asked to have improved rather than replaced: the name with its tags, date and body map, the
 * journal, the session's figures with heart rate beside them, then the metric switch (Weight /
 * Volume / Reps / Effort) over every lift. The lift bars, the opened lift's chips and its per-set
 * chart all follow the metric. Read-only, as before; the top bar's share saves the session as JSON.
 */
@Composable
fun SessionDetailScreen(
    onBack: () -> Unit,
    /** Opens a lift's own page (route gym/lift/{liftId}); null hides the link. */
    onOpenLiftTrend: ((String) -> Unit)? = null,
    viewModel: SessionDetailViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val exportPath by viewModel.exportPath.collectAsStateWithLifecycle()
    val context = LocalContext.current

    // When the per-session JSON has been written, open the system share sheet on it (Save to Files /
    // Drive / send), the same path Settings' exports use. Held back while the app lock is up.
    if (!com.forge.app.security.LocalAppLockActive.current) exportPath?.let { path ->
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
            // The path is cleared either way so the one-shot doesn't loop; a failure says so on the
            // app's one snackbar rather than vanishing.
            if (!shared) viewModel.showMessage("Couldn't open the share sheet.")
            viewModel.clearExportPath()
        }
    }

    SessionDetailPage(
        state = state,
        onBack = onBack,
        onExport = viewModel::exportSession,
        onOpenLiftTrend = onOpenLiftTrend
    )
}

/** The page as a pure function of [state], so the goldens can draw it from fake data. */
@Composable
fun SessionDetailPage(
    state: SessionDetailUiState,
    onBack: () -> Unit,
    onExport: () -> Unit,
    onOpenLiftTrend: ((String) -> Unit)? = null,
    initialMetric: SessionMetric = SessionMetric.WEIGHT
) {
    val ink = MaterialTheme.colorScheme.onBackground
    var metric by rememberSaveable { mutableStateOf(initialMetric) }
    // Bars or line, per metric: each keeps its own choice while the page is open.
    var weightStyle by rememberSaveable { mutableStateOf(SessionChartStyle.BARS) }
    var volumeStyle by rememberSaveable { mutableStateOf(SessionChartStyle.BARS) }
    var repsStyle by rememberSaveable { mutableStateOf(SessionChartStyle.BARS) }
    var effortStyle by rememberSaveable { mutableStateOf(SessionChartStyle.BARS) }
    val style = when (metric) {
        SessionMetric.WEIGHT -> weightStyle
        SessionMetric.VOLUME -> volumeStyle
        SessionMetric.REPS -> repsStyle
        SessionMetric.RPE -> effortStyle
    }
    val setStyle: (SessionChartStyle) -> Unit = {
        when (metric) {
            SessionMetric.WEIGHT -> weightStyle = it
            SessionMetric.VOLUME -> volumeStyle = it
            SessionMetric.REPS -> repsStyle = it
            SessionMetric.RPE -> effortStyle = it
        }
    }
    val data = state.data

    LazyColumn(
        Modifier.fillMaxSize().statusBarsPadding(),
        contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 56.dp)
    ) {
        item("top") {
            Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
                OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", onBack)
                Spacer(Modifier.weight(1f))
                if (data != null) OpenTopBarIcon(OpenIcons.Share, "Save this session as JSON", onExport)
            }
        }
        when {
            // No spinners: the local read is instant and the page simply appears.
            state.isLoading -> Unit
            data == null -> item("missing") {
                Column {
                    Spacer(Modifier.height(20.dp))
                    OpenLabel("Session not found. It may have been deleted.", style = OpenType.monoXS)
                }
            }
            else -> {
                item("header") {
                    Column {
                        Spacer(Modifier.height(12.dp))
                        SessionHeader(data)
                        Spacer(Modifier.height(24.dp))
                        SessionFigures(data)
                    }
                }
                if (state.hrChecked) item("heart") {
                    Column {
                        Spacer(Modifier.height(36.dp))
                        HeartRateSection(state.hrView)
                    }
                }
                if (data.exercises.isEmpty()) {
                    item("no-lifts") {
                        Column {
                            Spacer(Modifier.height(44.dp))
                            OpenLabel("No lifts logged in this session", style = OpenType.monoXS)
                        }
                    }
                } else {
                    item("tabs") {
                        Column {
                            Spacer(Modifier.height(32.dp))
                            OpenTabs(
                                SessionMetric.entries, metric, { metricTabLabel(it) }, { metric = it },
                                // Sideways rather than wrapped: at a large font scale "Effort" broke mid-word.
                                Modifier.horizontalScroll(rememberScrollState()).semantics { heading() }
                            )
                            Spacer(Modifier.height(14.dp))
                            MetricHead(metric, style, setStyle)
                            Spacer(Modifier.height(8.dp))
                        }
                    }
                    val max = data.exercises.maxOf { it.metricValue(metric) }
                    itemsIndexed(data.exercises, key = { i, ex -> "ex:$i:${ex.name}" }) { i, ex ->
                        LiftRow(
                            ex = ex,
                            metric = metric,
                            style = style,
                            maxValue = max,
                            defaultOpen = i == 0,
                            onOpenLiftTrend = onOpenLiftTrend
                        )
                    }
                }
            }
        }
    }
}

/** The tab's word: RPE reads as Effort on the page, as the design names it. */
internal fun metricTabLabel(m: SessionMetric): String = if (m == SessionMetric.RPE) "Effort" else m.label
