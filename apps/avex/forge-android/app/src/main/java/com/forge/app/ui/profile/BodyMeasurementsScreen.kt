@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.BodyMeasurementEntry
import com.forge.app.domain.units.lengthInputValue
import com.forge.app.domain.units.lengthUnitLabel
import com.forge.app.domain.units.toDisplayLength
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.statsEntrance
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.abs

/** The trend lane every tracked row draws into, so the marks share one right edge down the page. */
private val MARK_WIDTH = 140.dp
private val MARK_HEIGHT = 34.dp

/**
 * The Measurements sub-screen (GYMAP-52), reached from the Profile BODY section's SIZES row. It owns
 * the values, the trends and the logging that the Profile row only trims.
 *
 * Shape (reworked 2026-07-25): a serif hero whose MARK is a five-segment coverage rail — one segment
 * per circumference, filled once that site has a reading, hollow while it does not, captioned by the
 * names of the sites still missing. Under it, one row per TRACKED site only: mono name, the current
 * value as an open serif figure with its change since the first reading, and its trend lane (a
 * sparkline, or the empty track carrying a single dot when there is exactly one reading) stamped with
 * the date of that last reading.
 *
 * The untracked sites are the rail's hollow segments and its caption — never a stack of identical
 * empty rows (§12: N rows sharing one empty status collapse to ONE mark, and an all-ghost group drops
 * its marks). That makes zero, one and five the same page at different fills: at zero the rail is all
 * hollow and one filled capsule opens the log sheet; at one, a single live row sits above a rail that
 * is four-fifths hollow, so the contrast reads as "still forming" without drawing four ghost lines.
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

    // Lead with the live (§4.8): the sites that HAVE readings are the page's rows; the rest live in
    // the rail above them.
    val tracked = state.series.filter { it.entries.isNotEmpty() }
    val untracked = state.series.filter { it.entries.isEmpty() }

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
            Column(Modifier.fillMaxWidth().statsEntrance(0)) {
                Spacer(Modifier.height(4.dp))
                // An honest coverage count, never hidden or dashed at zero (§12). Eyebrow trim
                // matches the Gallery hero (labelMedium, 1sp tracking) so the two Profile sub-screens
                // open the same way.
                Text(
                    "${state.trackedCount} OF ${state.series.size} TRACKED",
                    style = MaterialTheme.typography.labelMedium,
                    color = muted,
                    letterSpacing = 1.sp
                )
                Spacer(Modifier.height(4.dp))
                // Serif hero names the screen (§2/§3) — bare name, no terminal period (§11).
                Text("Measurements", style = MaterialTheme.typography.headlineLarge, color = onBg)
            }

            Spacer(Modifier.height(16.dp))
            Column(Modifier.fillMaxWidth().statsEntrance(1)) {
                CoverageRail(state.series, accent)
                if (untracked.isNotEmpty()) {
                    Spacer(Modifier.height(8.dp))
                    // The rail's caption names what its hollow segments stand for (§11: a state never
                    // stands alone, it carries its referent). At zero every site is hollow, so the
                    // names ARE the caption; once something is tracked the missing ones are named as
                    // such. Words caption the mark, they are never the section's content (§12).
                    Text(
                        untracked.joinToString(" · ") { it.type.label.uppercase() }
                            .let { if (tracked.isEmpty()) it else "NOT TRACKED · $it" },
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        lineHeight = 15.sp
                    )
                }
            }

            Spacer(Modifier.height(28.dp))
            if (tracked.isEmpty()) {
                // The zero state's do-it-now action (§8 ①), and the only filled capsule on the page.
                // It exists only here: once rows are on the page each row taps into the same sheet,
                // and the top-bar + stays reachable at every state without scrolling.
                ForgePrimaryCapsule(
                    label = "Log measurements",
                    onClick = { showSheet = true },
                    modifier = Modifier.fillMaxWidth().statsEntrance(2)
                )
            } else {
                Box(Modifier.fillMaxWidth().statsEntrance(2)) { SectionHeader("TRACKED", muted) }
                Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
                    tracked.forEachIndexed { i, series ->
                        MeasurementRow(
                            series = series,
                            useCm = state.useCm,
                            onTap = { showSheet = true },
                            onBg = onBg,
                            muted = muted,
                            accent = accent,
                            modifier = Modifier.statsEntrance(3 + i)
                        )
                    }
                }
            }
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

/**
 * The hero's mark: one segment per circumference, filled at accent once that site has a reading and
 * an empty track (outline 0.25, the app's word for "a mark with nothing in it yet") while it does
 * not. It is the same coverage idea as the Profile BODY row's pips, drawn at this screen's scale —
 * five 9dp pips spread across a full 24dp-gutter page would be debris (§12), five full-width segments
 * read as the slots they are, and the mark works identically at zero, one and five.
 *
 * Passive: it reports, it is not an affordance (§1/§4.5 — nothing looks tappable while doing nothing,
 * and nothing tappable hides). Logging is the top-bar + and, at zero, the capsule beneath it.
 */
@Composable
private fun CoverageRail(series: List<MeasurementSeries>, accent: Color) {
    // Filled and hollow differ in SHAPE, not just tone — a solid accent bar against a hollow ringed
    // slot, the same disc/ring language as the Profile's pips (§8/§12). Tone alone could not carry it:
    // the navy accent measures 2.39:1 on Pearl, so any empty rung bright enough to read on near-black
    // would come out brighter than the filled state it is supposed to sit beneath. The ring takes
    // §12's sanctioned `muted@0.55`; at the outline 0.25 this first shipped with, the whole rail
    // measured 1.09:1 on device — at zero, when every segment is hollow, that is an invisible page.
    val ring = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f)
    val shape = RoundedCornerShape(50)
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        series.forEach { s ->
            Box(
                Modifier
                    .weight(1f)
                    .height(8.dp)
                    .clip(shape)
                    .then(
                        if (s.entries.isNotEmpty()) Modifier.background(accent)
                        else Modifier.border(1.5.dp, ring, shape)
                    )
            )
        }
    }
}

/**
 * One tracked site, on TWO rails and nothing between them — the Profile BODY row's anatomy at this
 * screen's depth. LEFT: the mono site name with its current value as an open serif figure, unit and
 * total change riding the baseline. RIGHT: the trend lane, stamped underneath with the date of the
 * last reading, so a value logged in March never reads as current beside one logged last week.
 *
 * The change is measured from the FIRST reading and names it ("↓ 1.5 SINCE MAR 3") — a circumference
 * is logged sparsely, so a bare "since last time" is a delta over an unknown gap. Both directions are
 * muted: the arrow states direction, never a good/bad verdict (§11), matching the body-fat row.
 *
 * The whole row is the tap target and opens the log sheet with every site's latest value seeded, so
 * one site can be corrected or the rest filled in from wherever you tapped. No nested tap (§8).
 */
@Composable
private fun MeasurementRow(
    series: MeasurementSeries,
    useCm: Boolean,
    onTap: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    modifier: Modifier = Modifier
) {
    val entries = series.entries
    val last = entries.last()
    // Display-unit values drive the sparkline; the figures format from cm at the edge.
    val display = remember(entries, useCm) { entries.map { toDisplayLength(it.valueCm, useCm) } }
    val today = remember { LocalDate.now() }
    // Gate on the DISPLAY-unit magnitude, not cm — a sub-0.05-inch wobble that rounds to "0.0 in"
    // should not render as a change at all.
    val changeCm = if (entries.size >= 2) last.valueCm - entries.first().valueCm else null
    val change = changeCm?.takeIf { toDisplayLength(abs(it), useCm) >= 0.05 }

    Row(
        modifier
            .fillMaxWidth()
            .bounceCombinedClick(onClickLabel = "Log ${series.type.label}", onClick = onTap),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                series.type.label.uppercase(),
                style = MaterialTheme.typography.labelMedium,
                color = muted
            )
            Spacer(Modifier.height(3.dp))
            Row(verticalAlignment = Alignment.Bottom) {
                Text(
                    lengthInputValue(last.valueCm, useCm),
                    style = MaterialTheme.typography.headlineMedium,
                    color = onBg
                )
                Spacer(Modifier.width(4.dp))
                Text(
                    lengthUnitLabel(useCm).uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 9.sp,
                    modifier = Modifier.padding(bottom = 4.dp)
                )
            }
            change?.let {
                Spacer(Modifier.height(3.dp))
                Text(
                    // The magnitude carries no unit suffix: the figure directly above it already
                    // states the unit, and repeating it twice per row is noise (§4.3).
                    "${if (it > 0) "↑" else "↓"} ${lengthInputValue(abs(it), useCm)} SINCE ${monoDate(entries.first(), today)}",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted
                )
            }
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.width(MARK_WIDTH), horizontalAlignment = Alignment.End) {
            if (display.size >= 2) {
                ProfileSparkline(display, accent, Modifier.fillMaxWidth().height(MARK_HEIGHT))
            } else {
                SingleReadingMark(accent, Modifier.fillMaxWidth().height(MARK_HEIGHT))
            }
            Spacer(Modifier.height(4.dp))
            Text(
                monoDate(last, today),
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 9.sp
            )
        }
    }
}

/**
 * The one-reading mark: the empty track a trend will fill, carrying the single reading as the same
 * accent dot [ProfileSparkline] leaves at the end of a line. A sparkline needs two points, so this
 * lane used to be blank — the row's first log landed as a figure with nothing beside it, and the
 * screen looked broken exactly when the user had just used it.
 */
@Composable
private fun SingleReadingMark(accent: Color, modifier: Modifier = Modifier) {
    // Solid bar, so the lower rung (see GhostSpark): it must stay under the accent dot riding on it.
    val track = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.30f)
    Canvas(modifier) {
        val y = size.height / 2f
        val h = 4.dp.toPx()
        val r = 3.dp.toPx()
        drawRoundRect(
            color = track,
            topLeft = Offset(0f, y - h / 2f),
            size = Size(size.width, h),
            cornerRadius = CornerRadius(h / 2f)
        )
        drawCircle(accent, radius = r, center = Offset(size.width - r, y))
    }
}

/** A reading's day as a mono stamp — the year only when it is not this one ("MAR 3" / "MAR 3, 2025"). */
private fun monoDate(entry: BodyMeasurementEntry, today: LocalDate): String {
    val date = LocalDate.parse(entry.dateKey)
    val pattern = if (date.year == today.year) "MMM d" else "MMM d, yyyy"
    return date.format(DateTimeFormatter.ofPattern(pattern, Locale.getDefault())).uppercase()
}

// The Profile-hub trim of this screen lives in the merged BODY section ([BodyMetricsSection] in
// ProfileBody.kt) as the "SIZES" row — a coverage rail plus its "n of 5" and an Open pill. The values,
// the trends and the logging are this screen's alone (§4.2).
