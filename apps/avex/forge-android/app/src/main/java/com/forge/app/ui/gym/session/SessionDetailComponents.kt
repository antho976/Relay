package com.forge.app.ui.gym.session

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.health.SessionHrView
import com.forge.app.domain.session.SessionType
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.domain.units.unitLabel
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenPrTag
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.common.window.Dialog
import com.forge.app.ui.gym.session.state.SessionDetailData
import com.forge.app.ui.gym.stats.components.BodyHeatmap
import com.forge.app.ui.gym.stats.components.musclesReadout
import com.forge.app.ui.gym.stats.state.MuscleSetCount
import com.forge.app.ui.theme.HistoryType
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter

// ─── Header ─────────────────────────────────────────────────────────────────────

/**
 * The session's name block: its markers and tags as a mono line, the name in serif with the PR tag,
 * the date and duration in serif italic, and the body map (tap to enlarge) on the right; the journal
 * under all of it.
 */
@Composable
internal fun SessionHeader(data: SessionDetailData) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val zone = ZoneId.systemDefault()
    val date = Instant.ofEpochMilli(data.dateMs).atZone(zone).toLocalDate()
    // The year when it isn't this one: "Monday, 14 October" was ambiguous for last year's sessions.
    val pattern = if (date.year == LocalDate.now(zone).year) "EEEE, d MMMM" else "EEEE, d MMMM yyyy"
    val dateLine = listOfNotNull(
        date.format(DateTimeFormatter.ofPattern(pattern, currentLocale())),
        data.durationMin?.takeIf { it > 0 }?.let { "$it min" }
    ).joinToString(" · ")
    val tags = sessionTags(data)

    Column {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.Bottom
        ) {
            Column(Modifier.weight(1f)) {
                if (tags.isNotEmpty()) {
                    OpenLabel(tags, style = OpenType.monoXS)
                    Spacer(Modifier.height(6.dp))
                }
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(data.title, style = OpenType.titleM, color = ink, modifier = Modifier.weight(1f, fill = false))
                    if (data.prCount > 0) OpenPrTag()
                }
                Spacer(Modifier.height(6.dp))
                Text(dateLine, style = HistoryType.dateLine, color = muted)
            }
            if (data.muscleSplit.isNotEmpty()) CompactBodyMap(data.muscleSplit, Modifier.width(96.dp))
        }
        if (data.journal.isNotBlank()) {
            Spacer(Modifier.height(14.dp))
            Text("“${data.journal}”", style = HistoryType.journal, color = muted)
        }
    }
}

/** "HEAVY · FOCUS": the session type's marker, a non-normal intensity, then the user's own tags. */
internal fun sessionTags(data: SessionDetailData): String = buildList {
    (if (data.deload) SessionType.DELOAD.pillLabel else SessionType.fromKey(data.sessionType)?.pillLabel)?.let { add(it) }
    if (data.intensity.isNotBlank() && data.intensity != "normal") add(data.intensity)
    data.tag.split(",").forEach { t -> t.trim().takeIf { it.isNotEmpty() }?.let { add(it) } }
}.joinToString(" · ")

/**
 * The front and back figures tinted by sets, shrunk for the header: no legend, no captions. Tap to
 * see it full size.
 */
@Composable
private fun CompactBodyMap(split: List<MuscleSetCount>, modifier: Modifier = Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val setsBy = split.associate { it.muscle to it.sets }
    var enlarged by rememberSaveable { mutableStateOf(false) }
    Box(
        modifier
            .clip(RoundedCornerShape(4.dp))
            .semantics { contentDescription = "Muscles worked: ${musclesReadout(setsBy)}. Tap to enlarge." }
            .clickableLabeled("Enlarge the muscle map", role = Role.Button) { enlarged = true }
    ) {
        BodyHeatmap(
            setsByMuscle = setsBy,
            accent = accent,
            faint = outline.copy(alpha = 0.35f),
            silhouette = outline.copy(alpha = 0.25f),
            labelColor = muted,
            figureHeight = 96.dp,
            showLegend = false,
            showTitles = false
        )
    }
    if (enlarged) MuscleMapDialog(setsBy) { enlarged = false }
}

/** The full-size map with its captions and key; closes on its X, a scrim tap or back. */
@Composable
private fun MuscleMapDialog(setsByMuscle: Map<MuscleGroup, Int>, onDismiss: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    Dialog(onDismissRequest = onDismiss) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(4.dp))
                .background(MaterialTheme.colorScheme.surface)
                .verticalScroll(rememberScrollState())
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                OpenLabel("Muscles worked", Modifier.weight(1f))
                OpenTopBarIcon(OpenIcons.X, "Close", onDismiss)
            }
            BodyHeatmap(
                setsByMuscle = setsByMuscle,
                accent = accent,
                faint = outline.copy(alpha = 0.35f),
                silhouette = outline.copy(alpha = 0.25f),
                labelColor = muted,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}

// ─── Figures ────────────────────────────────────────────────────────────────────

private enum class Trend { UP, DOWN, SAME }

/**
 * Direction of [cur] against the last session of the same training; null when there is none. A
 * prior of exactly zero is real (a bodyweight-only session), so the guard is on null.
 */
private fun trendOf(cur: Double, prev: Double?): Trend? {
    if (prev == null) return null
    val eps = prev * 0.001
    return when {
        cur > prev + eps -> Trend.UP
        cur < prev - eps -> Trend.DOWN
        else -> Trend.SAME
    }
}

/**
 * The session's figures in one row: volume and sets (with ↑/↓ against the last session of this
 * training when they moved), average RPE when sets were rated, and PRs. Volume and sets read zero
 * rather than vanish.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SessionFigures(data: SessionDetailData) {
    val unit = LocalForgeSettings.current.weightUnit
    val volume = data.volumeLb ?: 0.0
    val figures: List<@Composable (Modifier) -> Unit> = listOfNotNull(
        { m -> Figure(formatVolumeCompact(volume, unit, withUnit = false), "Volume · ${unitLabel(unit)}", trendOf(volume, data.prevVolumeLb), m) },
        { m -> Figure("${data.setCount}", if (data.setCount == 1) "Set" else "Sets", trendOf(data.setCount.toDouble(), data.prevSetCount?.toDouble()), m) },
        data.avgRpe?.let { rpe -> { m: Modifier -> Figure(rpeLabel(rpe), "Avg RPE", null, m) } },
        { m -> Figure("${data.prCount}", if (data.prCount == 1) "PR" else "PRs", null, m) },
    )
    // Even columns across the row at normal text, so all four fit a 384dp phone without one
    // dropping alone onto a second line; past 1.15x text they wrap rather than crowd.
    if (androidx.compose.ui.platform.LocalDensity.current.fontScale <= 1.15f) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            // Volume carries the longest label ("Volume · lb"), so its column is a little wider.
            figures.forEachIndexed { i, f -> f(Modifier.weight(if (i == 0) 1.35f else 1f)) }
        }
    } else {
        FlowRow(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(20.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            figures.forEach { it(FigureWidth) }
        }
    }
}

/** Each figure's least width, so the row reads as columns, as the design draws it. */
private val FigureWidth = Modifier.widthIn(min = 72.dp)

/** One figure: serif value, a ↑ (red) or ↓ (muted) when it moved, a mono label under it. */
@Composable
private fun Figure(value: String, label: String, trend: Trend?, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.Top) {
            Text(value, style = HistoryType.summaryFigure, color = MaterialTheme.colorScheme.onBackground)
            when (trend) {
                Trend.UP -> Text("↑", style = HistoryType.summaryArrow, color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.semantics { contentDescription = "up from last session" })
                Trend.DOWN -> Text("↓", style = HistoryType.summaryArrow, color = muted,
                    modifier = Modifier.semantics { contentDescription = "down from last session" })
                else -> Unit
            }
        }
        OpenLabel(label, style = OpenType.monoXS)
    }
}

// ─── Heart rate ─────────────────────────────────────────────────────────────────

/**
 * Heart rate from the watch: the average as a figure, its peak in the line under it, the trace over
 * the session's minutes, then each lift's average and the recovery read. Without a trace the reading
 * is a zero over a bare baseline and a line saying what fills it.
 */
@Composable
internal fun HeartRateSection(hr: SessionHrView?) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            OpenLabel("Heart rate")
            OpenLabel("From your watch")
        }
        Spacer(Modifier.height(10.dp))
        Text(
            buildAnnotatedString {
                append("${hr?.avgBpm ?: 0}")
                withStyle(HistoryType.heartUnit.toSpanStyle().copy(color = muted)) { append(" bpm") }
            },
            style = HistoryType.heartFigure,
            color = ink
        )
        Spacer(Modifier.height(6.dp))
        OpenLabel(
            if (hr == null) "Wear your watch while you train and it fills this"
            else "Average · peak ${hr.maxBpm}",
            style = OpenType.monoXS
        )
        Spacer(Modifier.height(12.dp))
        if (hr == null) {
            Box(Modifier.fillMaxWidth().height(1.dp).background(OpenColors.Rule))
            return@Column
        }
        HeartTrace(hr)
        if (hr.perExercise.isNotEmpty()) {
            Spacer(Modifier.height(14.dp))
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                hr.perExercise.forEach { ex ->
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text(ex.name, style = OpenType.bodyS, color = muted, modifier = Modifier.weight(1f))
                        OpenLabel("Avg ${ex.avgBpm}", style = OpenType.monoXS, color = ink)
                    }
                }
            }
        }
        hr.avgHrr60?.let { drop ->
            Spacer(Modifier.height(10.dp))
            OpenLabel("Recovery · $drop bpm down in the first minute", style = OpenType.monoXS)
        }
    }
}

/** The trace as a red line over a faint red area, with the session's minutes under it. */
@Composable
private fun HeartTrace(hr: SessionHrView) {
    val accent = MaterialTheme.colorScheme.primary
    val measurer = rememberTextMeasurer()
    val pts = hr.points
    val minutes = if (pts.size < 2) 0 else ((pts.last().timeMs - pts.first().timeMs) / 60_000L).toInt()
    val ticks = minuteTicks(minutes)
    Canvas(
        Modifier.fillMaxWidth().height(84.dp + 18.dp).clearAndSetSemantics {
            contentDescription = "Heart rate over $minutes minutes, average ${hr.avgBpm}, peak ${hr.maxBpm}"
        }
    ) {
        if (pts.size < 2) return@Canvas
        val plotH = 84.dp.toPx()
        val t0 = pts.first().timeMs
        val span = (pts.last().timeMs - t0).coerceAtLeast(1L).toFloat()
        val lo = pts.minOf { it.bpm }.toFloat()
        val hi = pts.maxOf { it.bpm }.toFloat()
        val range = (hi - lo).coerceAtLeast(1f)
        val inset = 6.dp.toPx()
        fun x(ms: Long) = inset + ((ms - t0) / span) * (size.width - 2 * inset)
        fun y(bpm: Int) = plotH - ((bpm - lo) / range) * (plotH * 0.85f) - 2.dp.toPx()
        val line = Path()
        pts.forEachIndexed { i, p -> if (i == 0) line.moveTo(x(p.timeMs), y(p.bpm)) else line.lineTo(x(p.timeMs), y(p.bpm)) }
        val area = Path().apply {
            addPath(line)
            lineTo(x(pts.last().timeMs), plotH)
            lineTo(x(t0), plotH)
            close()
        }
        drawPath(area, accent.copy(alpha = 0.15f))
        drawPath(line, accent, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        ticks.forEach { m ->
            val t = measurer.measure("$m", OpenType.chartLabel.copy(color = OpenColors.Glyph))
            val cx = (inset + (m * 60_000f / span) * (size.width - 2 * inset) - t.size.width / 2f)
                .coerceIn(0f, size.width - t.size.width)
            drawText(t, topLeft = Offset(cx, plotH + 6.dp.toPx()))
        }
    }
}

/** Minute ticks under the trace: every 10 (15 or 30 on a long session), always ending on the last. */
internal fun minuteTicks(total: Int): List<Int> {
    if (total <= 0) return listOf(0)
    val step = when {
        total <= 70 -> 10
        total <= 120 -> 15
        else -> 30
    }
    val ticks = (0 until total step step).toMutableList()
    // The last tick is the session's length; drop a regular tick too close to it to read apart.
    if (ticks.isNotEmpty() && total - ticks.last() < step / 2) ticks.removeAt(ticks.lastIndex)
    return ticks + total
}

