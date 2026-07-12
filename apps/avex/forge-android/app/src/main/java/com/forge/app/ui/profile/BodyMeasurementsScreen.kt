@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
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
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.formatLengthDelta
import com.forge.app.domain.units.lengthInputValue
import com.forge.app.domain.units.lengthUnitLabel
import com.forge.app.domain.units.toDisplayLength
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceClick
import kotlin.math.abs

/**
 * The Measurements sub-screen (GYMAP-52), reached from the Profile hub's MEASUREMENTS card. An
 * overview-first read of the body's circumferences: a serif hero, then one row per measurement with
 * its current value, latest change and trend sparkline. Empty is drawn (a hollow tracked-rail + one
 * hint), never a wall of "not logged" rows (DESIGN §12). Logging opens the shared five-field sheet.
 */
@Composable
fun BodyMeasurementsScreen(
    onBack: () -> Unit,
    viewModel: BodyMeasurementsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var showSheet by remember { mutableStateOf(false) }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    Scaffold(
        topBar = {
            TopAppBar(
                // §2: wordmark in the chrome — the serif "Measurements" hero below names the screen.
                title = { ForgeWordmark() },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = { showSheet = true }) {
                        Icon(Icons.Filled.Add, contentDescription = "Log measurements", tint = accent)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
                .padding(bottom = 40.dp)
        ) {
            Spacer(Modifier.height(4.dp))
            Text(
                "${state.trackedCount} OF ${state.series.size} TRACKED",
                style = MaterialTheme.typography.labelMedium,
                color = muted
            )
            Spacer(Modifier.height(4.dp))
            // Serif hero names the screen (§2/§3) — bare name, no terminal period (§11).
            Text("Measurements", style = MaterialTheme.typography.headlineLarge, color = onBg)
            Spacer(Modifier.height(28.dp))

            MeasurementsBody(state, onRowTap = { showSheet = true }, onBg = onBg, muted = muted, accent = accent)
        }
    }

    if (showSheet) {
        BodyMeasurementLogSheet(
            series = state.series,
            useCm = state.useCm,
            onSave = { values ->
                viewModel.log(values)
                showSheet = false
            },
            onDismiss = { showSheet = false }
        )
    }
}

/** The measurement list, or the drawn zero-state when nothing has been logged yet (§12). */
@Composable
private fun MeasurementsBody(
    state: BodyMeasurementsUiState,
    onRowTap: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    if (!state.anyData) {
        TrackedRail(state, muted, accent)
        Spacer(Modifier.height(16.dp))
        InlineEmptyHint(
            "Log your waist, chest, arms, thighs and hips to track how your body changes.",
            muted
        )
        return
    }
    Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
        state.series.forEach { series ->
            MeasurementRow(series, state.useCm, onRowTap, onBg, muted, accent)
        }
    }
}

/**
 * One measurement: mono label + latest change on the header line, then the current value as an open
 * serif figure with its trend sparkline. A tracked-but-single reading shows just the figure; an
 * untracked measurement (only shown beside tracked siblings) shows a flat ghost line — "still
 * forming", drawn not written (§12). The whole row taps into the log sheet.
 */
@Composable
private fun MeasurementRow(
    series: MeasurementSeries,
    useCm: Boolean,
    onTap: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    val entries = series.entries
    val unit = lengthUnitLabel(useCm)
    // Display-unit values for the sparkline, hoisted out of the conditional so the remember slot is
    // stable when a measurement grows from one reading to two.
    val display = remember(entries, useCm) { entries.map { toDisplayLength(it.valueCm, useCm) } }
    // Change since the previous reading (neutral — arrow shows direction, both tones muted like the
    // bodyweight delta; up isn't "good" for a circumference).
    val deltaCm = if (entries.size >= 2) entries.last().valueCm - entries[entries.size - 2].valueCm else null

    Column(Modifier.fillMaxWidth().bounceClick(onClick = onTap)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(series.type.label.uppercase(), style = MaterialTheme.typography.labelMedium, color = muted)
            Spacer(Modifier.weight(1f))
            deltaCm?.let {
                if (abs(it) >= 0.05) Text(
                    "${if (it > 0) "↑" else "↓"} ${formatLengthDelta(abs(it), useCm)}",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 9.sp
                )
            }
        }
        Spacer(Modifier.height(6.dp))
        if (entries.isEmpty()) {
            GhostFlatLine(muted, Modifier.fillMaxWidth().height(28.dp))
        } else {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
                Text(
                    lengthInputValue(entries.last().valueCm, useCm),
                    style = MaterialTheme.typography.headlineMedium,
                    color = onBg
                )
                Spacer(Modifier.width(5.dp))
                Text(
                    unit.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 9.sp,
                    modifier = Modifier.padding(bottom = 5.dp)
                )
                Spacer(Modifier.weight(1f))
                if (entries.size >= 2) {
                    ProfileSparkline(display, accent, Modifier.width(120.dp).height(36.dp))
                }
            }
        }
    }
}

/**
 * The drawn zero-state: five pips (one per measurement) captioned with their names — hollow rings
 * for untracked, filled disc once a measurement has a reading. Works at zero (all hollow) and shows
 * which measurements exist, so the empty screen is data-at-zero, not a text wall (§8/§12).
 */
@Composable
private fun TrackedRail(state: BodyMeasurementsUiState, muted: Color, accent: Color) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        state.series.forEach { series ->
            val filled = series.entries.isNotEmpty()
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                Canvas(Modifier.height(12.dp).width(12.dp)) {
                    val r = size.minDimension / 2f
                    if (filled) drawCircle(accent, radius = r)
                    else drawCircle(muted.copy(alpha = 0.55f), radius = r - 0.75.dp.toPx(), style = Stroke(width = 1.5.dp.toPx()))
                }
                Spacer(Modifier.height(6.dp))
                Text(
                    series.type.label.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 9.sp,
                    textAlign = TextAlign.Center
                )
            }
        }
    }
}

/** A flat baseline line — the "no readings yet" ghost mark for a measurement shown beside tracked siblings. */
@Composable
private fun GhostFlatLine(muted: Color, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        val y = size.height / 2f
        drawLine(
            color = muted.copy(alpha = 0.3f),
            start = androidx.compose.ui.geometry.Offset(0f, y),
            end = androidx.compose.ui.geometry.Offset(size.width, y),
            strokeWidth = 1.5.dp.toPx(),
            cap = StrokeCap.Round
        )
    }
}

/**
 * The Profile-hub MEASUREMENTS card — a trim of the destination (§4.2): each tracked measurement's
 * latest value as a small serif figure, or the drawn tracked-rail + hint at zero. Uses its own
 * [BodyMeasurementsViewModel] so the hub needs no changes to ProfileViewModel. Header "open →" and
 * the trim both drill into [BodyMeasurementsScreen].
 */
@Composable
internal fun MeasurementsHubCard(
    onOpen: () -> Unit,
    viewModel: BodyMeasurementsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    SectionHeader("MEASUREMENTS", muted, action = "open →", onAction = onOpen)
    if (!state.anyData) {
        TrackedRail(state, muted, accent)
        Spacer(Modifier.height(12.dp))
        InlineEmptyHint("Track waist, chest, arms, thighs and hips.", muted)
        return
    }
    val unit = lengthUnitLabel(state.useCm)
    FlowRow(
        Modifier.fillMaxWidth().bounceClick(onClick = onOpen),
        horizontalArrangement = Arrangement.spacedBy(24.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        state.series.filter { it.entries.isNotEmpty() }.forEach { series ->
            Column {
                Row(verticalAlignment = Alignment.Bottom) {
                    Text(
                        lengthInputValue(series.entries.last().valueCm, state.useCm),
                        style = MaterialTheme.typography.headlineSmall,
                        color = onBg
                    )
                    Spacer(Modifier.width(3.dp))
                    Text(
                        unit,
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp,
                        modifier = Modifier.padding(bottom = 3.dp)
                    )
                }
                Spacer(Modifier.height(2.dp))
                Text(
                    series.type.label.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 9.sp
                )
            }
        }
    }
}
