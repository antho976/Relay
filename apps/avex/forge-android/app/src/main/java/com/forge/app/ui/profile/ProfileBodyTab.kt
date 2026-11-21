package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.common.ChartPoint
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenSmallButton
import com.forge.app.ui.common.OpenSparkline
import com.forge.app.ui.common.OpenTrendChart
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.ProfileType
import java.time.Instant
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle

/*
 * Body (pf2): weight leads, as its 7-day average with its trend and a small Log; composition and
 * size sit three across under it, each opening its own log (body fat, the watch's muscle, sizes).
 */

private const val DAY_MS = 86_400_000L

@Composable
internal fun ProfileBodyTab(body: ProfileBody, unit: WeightUnit, nowMs: Long, actions: ProfileActions) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val locale = currentLocale()

    // ── Weight ──
    val weight = remember(body.weight, body.goalLb, unit) { weightReading(body.weight, body.goalLb, unit) }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        OpenLabel("Weight · 7-day average", modifier = Modifier.weight(1f, fill = false))
        goalLabel(body.goalLb, unit)?.let { OpenLabel(it, modifier = Modifier.padding(start = 12.dp)) }
    }
    Spacer(Modifier.height(10.dp))
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        FittedFigure(
            weight.figure, weight.unit,
            listOf(OpenType.figureXL to OpenType.figureXLUnit, OpenType.figureL to OpenType.figureLUnit),
            Modifier.weight(1f)
        )
        OpenSmallButton("Log", filled = false, onClick = actions.onLogWeight, icon = OpenIcons.Plus, modifier = Modifier.padding(start = 12.dp))
    }
    Spacer(Modifier.height(8.dp))
    weight.change?.let { OpenLabel(it, style = OpenType.monoXS, color = if (weight.moved) ink else muted) }
    Spacer(Modifier.height(16.dp))
    WeightChart(body, unit, nowMs, weight.figure + weight.unit, locale)

    // ── Composition and size ──
    Spacer(Modifier.height(48.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        OpenLabel("Composition and size", modifier = Modifier.weight(1f, fill = false))
        OpenLabel("30 days", modifier = Modifier.padding(start = 12.dp))
    }
    Spacer(Modifier.height(18.dp))
    val fat = remember(body.bodyFat) { bodyFatReading(body.bodyFat) }
    val muscle = remember(body.leanMass, unit) { muscleReading(body.leanMass, unit) }
    val size = remember(body.measurements) { sizeReading(body.measurements.series, body.measurements.useCm) }
    val dateFmt = remember(locale) { DateTimeFormatter.ofPattern("MMM d", locale) }
    fun dateLine(t: Long?): String? = t?.let {
        staleReadingAge(it, nowMs) ?: Instant.ofEpochMilli(it).atZone(ZoneId.systemDefault()).toLocalDate().format(dateFmt)
    }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
        CompositionColumn(
            label = "Body fat", reading = fat,
            source = dateLine(body.bodyFat.maxOfOrNull { it.recordedAt }) ?: "None yet",
            actionLabel = "Log body fat", onClick = actions.onLogBodyFat,
            modifier = Modifier.weight(1f)
        )
        val muscleSource = when {
            body.leanMass.isNotEmpty() -> staleReadingAge(body.leanMass.maxOf { it.recordedAt }, nowMs) ?: "Watch"
            body.leanMassConnected -> "Watch"
            else -> "No watch yet"
        }
        CompositionColumn(
            label = "Muscle", reading = muscle, source = muscleSource,
            actionLabel = "Sync muscle from your watch",
            // Muscle is read from a watch, never typed; with no watch linked there is nothing to open.
            onClick = if (body.leanMassConnected) actions.onSyncMuscle else null,
            modifier = Modifier.weight(1f)
        )
        CompositionColumn(
            label = size.label, reading = size.reading,
            source = "${size.tracked} of ${size.total} sizes",
            actionLabel = "Log ${size.label.lowercase()}", onClick = actions.onLogSize,
            onSource = actions.onOpenSizes, sourceLabel = "Open all sizes",
            modifier = Modifier.weight(1f)
        )
    }
}

@Composable
private fun WeightChart(body: ProfileBody, unit: WeightUnit, nowMs: Long, figure: String, locale: java.util.Locale) {
    val start = nowMs - 90 * DAY_MS
    val inWindow = remember(body.weight, nowMs) { body.weight.filter { it.recordedAt >= start }.sortedBy { it.recordedAt } }
    if (inWindow.size < 2) {
        Canvas(Modifier.fillMaxWidth().height(40.dp).clearAndSetSemantics { contentDescription = "No weight trend yet" }) {
            drawRect(OpenColors.Rule, topLeft = Offset(0f, size.height - 1.dp.toPx()), size = Size(size.width, 1.dp.toPx()))
        }
        Spacer(Modifier.height(8.dp))
        OpenLabel("Two weigh-ins draw your trend", style = OpenType.monoXS)
        return
    }
    val span = (nowMs - start).toFloat()
    val points = inWindow.map {
        ChartPoint(((it.recordedAt - start) / span).coerceIn(0f, 1f), com.forge.app.domain.units.toDisplayWeight(it.weightLb, unit).toFloat())
    }
    val zone = ZoneId.systemDefault()
    val labels = remember(nowMs, locale) {
        val startMonth = YearMonth.from(Instant.ofEpochMilli(start).atZone(zone)).plusMonths(1)
        val end = YearMonth.from(Instant.ofEpochMilli(nowMs).atZone(zone))
        buildList {
            var m = startMonth
            while (!m.isAfter(end)) {
                val x = (m.atDay(1).atStartOfDay(zone).toInstant().toEpochMilli() - start) / span
                // A month name that would sit on top of "Now" gives way to it.
                if (x < 0.88f) add(x to m.month.getDisplayName(TextStyle.SHORT, locale))
                m = m.plusMonths(1)
            }
            add(1f to "Now")
        }
    }
    OpenTrendChart(
        current = points,
        previous = emptyList(),
        xLabels = labels,
        height = 130.dp,
        description = "Weight over 90 days, ${inWindow.size} weigh-ins, 7-day average $figure"
    )
}

/**
 * One of the three: its label, the figure with its unit at half size, its 30-day line, what moved,
 * and a quiet line saying where it comes from. The reading is the tap target that opens its log;
 * [onSource] makes the quiet line a second target (Waist's "1 of 5 sizes" opens every size).
 */
@Composable
private fun CompositionColumn(
    label: String,
    reading: BodyReading,
    source: String,
    actionLabel: String,
    onClick: (() -> Unit)?,
    modifier: Modifier = Modifier,
    onSource: (() -> Unit)? = null,
    sourceLabel: String? = null,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(modifier) {
        Column(
            Modifier
                .then(if (onClick != null) Modifier.clickableLabeled(actionLabel, onClick = onClick) else Modifier)
                .semantics(mergeDescendants = true) {},
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            OpenLabel(label, style = OpenType.monoXS)
            FittedFigure(
                reading.figure, reading.unit,
                listOf(
                    OpenType.figureMS to ProfileType.compositionUnit,
                    ProfileType.compositionStep to ProfileType.compositionUnitStep,
                    ProfileType.compositionSmall to ProfileType.compositionUnitStep,
                )
            )
            if (reading.series.size >= 2) {
                OpenSparkline(reading.series, Modifier.fillMaxWidth().height(18.dp), plain = true)
            } else {
                Canvas(Modifier.fillMaxWidth().height(18.dp)) {
                    drawRect(OpenColors.Rule, topLeft = Offset(0f, size.height / 2f), size = Size(size.width, 1.dp.toPx()))
                }
            }
            reading.change?.let { OpenLabel(it, style = OpenType.monoXS, color = if (reading.moved) ink else muted) }
        }
        Spacer(Modifier.height(8.dp))
        if (onSource != null) {
            Row(
                Modifier
                    .clickableLabeled(sourceLabel ?: source, onClick = onSource)
                    .padding(vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                OpenLabel(source, style = OpenType.monoXS, modifier = Modifier.weight(1f, fill = false))
                Icon(OpenIcons.ChevronRight, contentDescription = null, tint = muted, modifier = Modifier.size(12.dp))
            }
        } else {
            OpenLabel(source, style = OpenType.monoXS, modifier = Modifier.padding(vertical = 4.dp))
        }
    }
}
