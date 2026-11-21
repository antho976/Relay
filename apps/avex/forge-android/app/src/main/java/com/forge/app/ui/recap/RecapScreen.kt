package com.forge.app.ui.recap

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.OpenColumnChart
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenSmallButton
import com.forge.app.ui.common.OpenSwatchKey
import com.forge.app.ui.common.OpenTabs
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.goals.OpenBackBar
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.RecapType
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.time.Month
import java.time.format.TextStyle

/**
 * The Recap page (2026-10-21 redesign, "pg-recap"): from Weekly recap on Stats. Month: the month so
 * far against the month before, week by week, then what stood out. Year: every month as a bar, the
 * best one lit, then the year's highlights. The chevrons beside a period step back through earlier
 * months and years; the share glyph and the button at the end send the period as an image.
 */
@Composable
fun RecapScreen(
    onBack: () -> Unit,
    viewModel: RecapViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var tab by rememberSaveable { mutableStateOf(RecapTab.MONTH) }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val cs = MaterialTheme.colorScheme
    val palette = RecapCardRenderer.Palette(cs.background.toArgb(), cs.onBackground.toArgb(), cs.onSurfaceVariant.toArgb(), cs.primary.toArgb())
    val share: () -> Unit = {
        val kicker: String
        val period: String
        val figures: List<RecapFigure>
        val highlights: List<RecapHighlight>
        if (tab == RecapTab.MONTH) {
            val m = state.monthUi
            kicker = "Recap · ${m?.month?.year ?: ""}"; period = m?.name.orEmpty(); figures = m?.figures.orEmpty(); highlights = m?.highlights.orEmpty()
        } else {
            val y = state.yearUi
            kicker = "Recap · the year"; period = "${y?.year ?: ""}"; figures = y?.figures.orEmpty(); highlights = y?.highlights.orEmpty()
        }
        scope.launch {
            val uri = withContext(Dispatchers.Default) {
                RecapCardRenderer.render(context, kicker, period, figures, highlights, palette)
            }
            if (uri != null) runCatching { RecapCardRenderer.share(context, uri, "Share $period") }
        }
    }
    Scaffold(
        topBar = { OpenBackBar(onBack) { OpenTopBarIcon(OpenIcons.Share, "Share as an image", share) } },
        containerColor = Color.Transparent
    ) { inner ->
        // No spinner: the read is local and quick, the page appears when it lands.
        if (state.isLoading) {
            Box(Modifier.fillMaxSize().padding(inner))
            return@Scaffold
        }
        RecapPage(
            state = state,
            tab = tab,
            onTab = { tab = it },
            weightUnit = LocalForgeSettings.current.weightUnit,
            onStepMonth = viewModel::stepMonth,
            onStepYear = viewModel::stepYear,
            onShare = share,
            modifier = Modifier.padding(inner)
        )
    }
}

/** The page body, free of the ViewModel so the goldens render exactly what the app does. */
@Composable
internal fun RecapPage(
    state: RecapUiState,
    tab: RecapTab,
    onTab: (RecapTab) -> Unit,
    weightUnit: WeightUnit,
    onStepMonth: (Int) -> Unit,
    onStepYear: (Int) -> Unit,
    onShare: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(start = 24.dp, end = 24.dp, top = 4.dp)
            .navigationBarsPadding()
            .padding(bottom = 48.dp)
    ) {
        Spacer(Modifier.height(20.dp))
        Text("Recap", style = OpenType.title, color = ink, modifier = Modifier.semantics { heading() })
        Spacer(Modifier.height(24.dp))
        OpenTabs(RecapTab.entries, tab, { it.label }, onTab)
        Spacer(Modifier.height(32.dp))
        when (tab) {
            RecapTab.MONTH -> state.monthUi?.let { m ->
                val now = java.time.YearMonth.now()
                MonthBody(
                    m,
                    weightUnit = weightUnit,
                    canBack = state.firstMonth != null && m.month.isAfter(state.firstMonth),
                    canForward = m.month.isBefore(now),
                    onStep = onStepMonth,
                    onShare = onShare
                )
            }
            RecapTab.YEAR -> state.yearUi?.let { y ->
                YearBody(
                    y,
                    canBack = state.firstMonth != null && y.year > state.firstMonth.year,
                    canForward = y.year < java.time.YearMonth.now().year,
                    onStep = onStepYear,
                    onShare = onShare
                )
            }
        }
    }
}

/** The period's name with the steppers beside its mono line: ‹ › walk back and forward. */
@Composable
private fun PeriodHead(name: String, line: String, canBack: Boolean, canForward: Boolean, onStep: (Int) -> Unit, what: String) {
    FlowRow(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalArrangement = Arrangement.spacedBy(6.dp),
        itemVerticalAlignment = Alignment.Bottom
    ) {
        Text(name, style = RecapType.period, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.semantics { heading() })
        Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(bottom = 6.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                Stepper(OpenIcons.ChevronLeft, "Previous $what", canBack) { onStep(-1) }
                Stepper(OpenIcons.ChevronRight, "Next $what", canForward) { onStep(1) }
            }
            OpenLabel(line, style = OpenType.monoXS)
        }
    }
}

@Composable
private fun Stepper(icon: ImageVector, label: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        Modifier
            .touchSlop(vertical = 14.dp, horizontal = 14.dp)
            .then(if (enabled) Modifier.clickableLabeled(label, onClick = onClick) else Modifier)
            .padding(14.dp)
    ) {
        Icon(
            icon, contentDescription = if (enabled) null else label,
            tint = if (enabled) MaterialTheme.colorScheme.onSurfaceVariant else OpenColors.Unreached,
            modifier = Modifier.size(18.dp)
        )
    }
}

/** The four figures in a row: mono label, serif figure with its small suffix, mono change. */
@Composable
private fun Figures(figures: List<RecapFigure>) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        figures.forEach { f ->
            Column(
                Modifier
                    .weight(1f)
                    .semantics(mergeDescendants = true) { contentDescription = "${f.label} ${f.value}${f.suffix}, ${f.change}" },
                verticalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                OpenLabel(f.label, style = OpenType.monoXS, modifier = Modifier.clearAndSetSemantics { })
                Text(
                    buildAnnotatedString {
                        append(f.value)
                        if (f.suffix.isNotEmpty()) {
                            pushStyle(RecapType.figureUnit.toSpanStyle().copy(color = muted)); append(f.suffix); pop()
                        }
                    },
                    style = RecapType.figure, color = ink, modifier = Modifier.clearAndSetSemantics { }
                )
                OpenLabel(f.change, style = OpenType.monoXS, color = if (f.changeMoved) ink else muted, modifier = Modifier.clearAndSetSemantics { })
            }
        }
    }
}

@Composable
private fun SectionHead(left: String, right: String?) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        OpenLabel(left, Modifier.weight(1f).semantics { heading() })
        right?.let { OpenLabel(it) }
    }
}

/** What stood out: two across, each a mono label, the finding and its fact. */
@Composable
private fun Highlights(items: List<RecapHighlight>) {
    val ink = MaterialTheme.colorScheme.onBackground
    Column(verticalArrangement = Arrangement.spacedBy(22.dp)) {
        items.chunked(2).forEach { pair ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                pair.forEach { h ->
                    Column(Modifier.weight(1f).semantics(mergeDescendants = true) { }, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        OpenLabel(h.label, style = OpenType.monoXS)
                        Text(h.value, style = OpenType.rowTitleL, color = ink)
                        OpenLabel(h.sub, style = OpenType.monoXS)
                    }
                }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun MonthBody(m: MonthRecapUi, weightUnit: WeightUnit, canBack: Boolean, canForward: Boolean, onStep: (Int) -> Unit, onShare: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    PeriodHead(m.name, m.daysLine, canBack, canForward, onStep, "month")
    Spacer(Modifier.height(20.dp))
    Figures(m.figures)
    Spacer(Modifier.height(40.dp))
    SectionHead("Week by week · ${unitLabel(weightUnit)}", "vs ${m.previousName}")
    Spacer(Modifier.height(14.dp))
    OpenSwatchKey(listOf(m.previousName to OpenColors.Previous, m.name to accent))
    Spacer(Modifier.height(12.dp))
    PairedWeeks(m.weeks, m.previousWeeks, m.currentWeek, "Volume per week, ${m.name} against ${m.previousName}")
    m.weekCaption?.let {
        Spacer(Modifier.height(8.dp))
        OpenLabel(it, style = OpenType.monoXS)
    }
    Spacer(Modifier.height(40.dp))
    SectionHead("What stood out", null)
    Spacer(Modifier.height(16.dp))
    if (m.highlights.isEmpty()) OpenLabel("Log a session and this month fills in", style = OpenType.monoXS)
    else Highlights(m.highlights)
    Spacer(Modifier.height(36.dp))
    OpenSmallButton("Share ${m.name} as an image", filled = false, onClick = onShare, icon = OpenIcons.Share)
}

@Composable
private fun YearBody(y: YearRecapUi, canBack: Boolean, canForward: Boolean, onStep: (Int) -> Unit, onShare: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    val locale = currentLocale()
    PeriodHead("${y.year}", y.sinceLine, canBack, canForward, onStep, "year")
    Spacer(Modifier.height(20.dp))
    Figures(y.figures)
    Spacer(Modifier.height(40.dp))
    SectionHead("Workouts per month", y.bestLine)
    Spacer(Modifier.height(14.dp))
    OpenColumnChart(
        values = y.months.map { it.toFloat() },
        colors = y.months.indices.map { i ->
            when (i) {
                y.bestMonth -> accent
                y.currentMonth -> accent.copy(alpha = 0.35f)
                else -> OpenColors.Previous
            }
        },
        height = 140.dp,
        columnLabels = Month.entries.map { it.getDisplayName(TextStyle.NARROW_STANDALONE, locale) },
        valueLabels = true,
        gapFraction = 0.35f,
        description = "Workouts per month in ${y.year}: " + y.months.withIndex().filter { it.value > 0 }
            .joinToString(", ") { "${Month.of(it.index + 1).getDisplayName(TextStyle.SHORT_STANDALONE, locale)} ${it.value}" }
    )
    y.caption?.let {
        Spacer(Modifier.height(8.dp))
        OpenLabel(it, style = OpenType.monoXS)
    }
    Spacer(Modifier.height(40.dp))
    SectionHead("The year so far", null)
    Spacer(Modifier.height(16.dp))
    if (y.highlights.isEmpty()) OpenLabel("Log a session and this year fills in", style = OpenType.monoXS)
    else Highlights(y.highlights)
    Spacer(Modifier.height(36.dp))
    OpenSmallButton("Share ${y.year} as an image", filled = false, onClick = onShare, icon = OpenIcons.Share)
}

/**
 * Four weeks, two columns each: the month before in the quiet grey, this month in red, and a week
 * still going in a dimmer red so an unfinished week does not read as a drop.
 */
@Composable
private fun PairedWeeks(now: List<Double>, before: List<Double>, current: Int?, description: String) {
    val accent = MaterialTheme.colorScheme.primary
    val labelColor = MaterialTheme.colorScheme.onSurfaceVariant
    val measurer = rememberTextMeasurer()
    Canvas(Modifier.fillMaxWidth().height(120.dp).clearAndSetSemantics { contentDescription = description }) {
        val labelRoom = 18.dp.toPx()
        val bottom = size.height - labelRoom
        val max = (now + before).maxOrNull()?.takeIf { it > 0 } ?: 1.0
        val slot = size.width / 4f
        val bw = minOf(slot * 0.33f, 30.dp.toPx())
        val r = CornerRadius(2.dp.toPx())
        drawRect(OpenColors.Rule, topLeft = Offset(0f, bottom), size = Size(size.width, 1.dp.toPx()))
        for (i in 0 until 4) {
            val x0 = i * slot + (slot - 2 * bw) / 2f
            fun bar(v: Double, x: Float, c: Color) {
                if (v <= 0) return
                val h = ((v / max) * (bottom - 4.dp.toPx())).toFloat().coerceAtLeast(3.dp.toPx())
                drawRoundRect(c, topLeft = Offset(x, bottom - h), size = Size(bw, h), cornerRadius = r)
            }
            bar(before.getOrElse(i) { 0.0 }, x0, OpenColors.Previous)
            bar(now.getOrElse(i) { 0.0 }, x0 + bw, if (i == current) accent.copy(alpha = 0.35f) else accent)
            val t = measurer.measure("Wk ${i + 1}", OpenType.chartLabel.copy(color = labelColor))
            drawText(t, topLeft = Offset(i * slot + slot / 2f - t.size.width / 2f, bottom + 5.dp.toPx()))
        }
    }
}
