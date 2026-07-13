package com.forge.app.ui.gym.session

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.session.SessionType
import com.forge.app.domain.units.formatHoldLabel
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.formatWeight
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.EditorialHairline
import com.forge.app.ui.common.rpeLabel
import com.forge.app.ui.gym.session.state.ExerciseDetail
import com.forge.app.ui.gym.session.state.SessionChartStyle
import com.forge.app.ui.gym.session.state.SessionDetailData
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.session.state.SetDetail
import com.forge.app.ui.gym.stats.components.BodyHeatmap
import com.forge.app.ui.gym.stats.state.MuscleSetCount
import com.forge.app.ui.overview.SummaryStat
import com.forge.app.ui.theme.ForgeError
import com.forge.app.ui.theme.ForgeSuccess
import com.forge.app.ui.theme.ForgeWarning
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

// ─── Header + summary + muscle map ─────────────────────────────────────────────

/**
 * The top block: session title/date/journal + the summary stats on the left, and a compact
 * front+back muscle map on the right. The map is sized to sit just below the title and bottom out
 * near the summary stats so the whole header reads as one tight unit (replaces the old full-width
 * "MUSCLES WORKED" bar card, which ate too much vertical space).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SessionHeaderBlock(
    data: SessionDetailData,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        // Bottom-align so the map's lower edge lands on the summary-stats row rather than floating.
        verticalAlignment = Alignment.Bottom
    ) {
        Column(Modifier.weight(1f)) {
            val zone = ZoneId.systemDefault()
            val date = Instant.ofEpochMilli(data.dateMs).atZone(zone).toLocalDate()
            val dateStr = date.format(DateTimeFormatter.ofPattern("EEEE, d MMMM", Locale.getDefault()))
            val sub = buildList {
                // Deload marker wins; otherwise the stored session-type's pill (TEST/TECHNIQUE/… once a
                // picker writes them) — same resolution as the Overview status pill so the two can't drift.
                (if (data.deload) SessionType.DELOAD.pillLabel else SessionType.fromKey(data.sessionType)?.pillLabel)
                    ?.let { add(it) }
                if (data.intensity.isNotBlank() && data.intensity != "normal") add(data.intensity.uppercase())
                // Quick tags are stored comma-separated (#107); add each as its own token so the sub-line
                // never shows a raw "FOCUS,HEAVY" (the Overview pill splits them the same way).
                data.tag.split(",").forEach { t -> t.trim().takeIf { it.isNotEmpty() }?.let { add(it.uppercase()) } }
            }.joinToString(" · ")
            if (sub.isNotEmpty()) {
                Text(sub, style = MaterialTheme.typography.labelSmall, letterSpacing = 1.5.sp, color = muted, fontSize = 9.sp)
                Spacer(Modifier.height(2.dp))
            }
            Text(data.title, style = MaterialTheme.typography.headlineSmall, color = onBg, fontWeight = FontWeight.Normal)
            Text(dateStr, style = MaterialTheme.typography.bodySmall, color = muted, fontSize = 11.sp, fontStyle = FontStyle.Italic)
            if (data.journal.isNotBlank()) {
                Spacer(Modifier.height(10.dp))
                Text("“${data.journal}”", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
            }
            Spacer(Modifier.height(14.dp))
            SummaryStrip(data, onBg, muted)
        }
        if (data.muscleSplit.isNotEmpty()) {
            CompactBodyMap(data.muscleSplit, accent, muted, outline, Modifier.width(116.dp))
        }
    }
}

/**
 * The two anatomical figures (front + back) tinted by set count — same art as the Stats heatmap, but
 * shrunk for the header (no legend, no FRONT/BACK captions). Caller fixes the width; the figures
 * scale to a short fixed height so the pair tucks beside the title. Tap to blow it up full-size.
 */
@Composable
private fun CompactBodyMap(
    split: List<MuscleSetCount>,
    accent: Color,
    muted: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    val setsBy = split.associate { it.muscle to it.sets }
    var enlarged by rememberSaveable { mutableStateOf(false) }
    Box(
        modifier
            .clip(RoundedCornerShape(8.dp))
            .semantics { contentDescription = "Muscles worked, tap to enlarge"; role = Role.Button }
            .clickable { enlarged = true }
    ) {
        BodyHeatmap(
            setsByMuscle = setsBy,
            accent = accent,
            faint = outline.copy(alpha = 0.34f),
            silhouette = outline.copy(alpha = 0.26f),
            labelColor = muted,
            figureHeight = 96.dp,
            showLegend = false,
            showTitles = false
        )
    }
    if (enlarged) MuscleMapDialog(setsBy, accent, muted, outline) { enlarged = false }
}

/**
 * The full-size front+back heatmap (with captions + legend) shown when the header map is tapped.
 * Dismiss via the explicit close button, a scrim tap, or back — the content itself does NOT swallow
 * taps as dismiss, leaving the figures free for future per-muscle tap targets. Scrolls on short screens.
 */
@Composable
private fun MuscleMapDialog(
    setsByMuscle: Map<MuscleGroup, Int>,
    accent: Color,
    muted: Color,
    outline: Color,
    onDismiss: () -> Unit
) {
    Dialog(onDismissRequest = onDismiss) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surface)
                .verticalScroll(rememberScrollState())
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                Text(
                    "MUSCLES WORKED",
                    style = MaterialTheme.typography.labelLarge,
                    color = muted,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f)
                )
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Filled.Close, contentDescription = "Close", tint = muted)
                }
            }
            BodyHeatmap(
                setsByMuscle = setsByMuscle,
                accent = accent,
                faint = outline.copy(alpha = 0.34f),
                silhouette = outline.copy(alpha = 0.26f),
                labelColor = muted,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}

/**
 * The summary stats; wraps so it fits beside the muscle map. Volume & Sets carry an up/down/same
 * caret vs the last session of this same training ([SessionDetailData.prevVolumeLb]/[prevSetCount]);
 * the rest reuse the plain overview [SummaryStat].
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SummaryStrip(data: SessionDetailData, onBg: Color, muted: Color) {
    val useKg = LocalForgeSettings.current.useKg
    FlowRow(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(18.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        data.volumeLb?.takeIf { it > 0 }?.let {
            TrendStat(formatVolume(it, useKg), "VOLUME", trendOf(it, data.prevVolumeLb), muted, onBg)
        }
        data.durationMin?.takeIf { it > 0 }?.let { SummaryStat("$it min", "DURATION", muted, onBg) }
        if (data.setCount > 0) {
            TrendStat("${data.setCount}", "SETS", trendOf(data.setCount.toDouble(), data.prevSetCount?.toDouble()), muted, onBg)
        }
        if (data.prCount > 0) SummaryStat("${data.prCount}", "PRs", muted, onBg)
        data.avgRpe?.let { SummaryStat(rpeLabel(it), "AVG RPE", muted, onBg) }
    }
}

private enum class Trend { UP, DOWN, SAME }

/**
 * Direction of [cur] vs the previous session's [prev]; null only when there's no prior to compare.
 * A prior of exactly 0.0 is valid (a bodyweight-only session logs zero volume), so we guard on null
 * rather than `<= 0.0` — comparing real volume against a prior zero correctly reads as UP.
 */
private fun trendOf(cur: Double, prev: Double?): Trend? {
    if (prev == null) return null
    val eps = prev * 0.001 // ignore sub-0.1% float noise so "same" really means same
    return when {
        cur > prev + eps -> Trend.UP
        cur < prev - eps -> Trend.DOWN
        else -> Trend.SAME
    }
}

/** A summary stat (value over label) with a small up/down/same caret vs the last session. */
@Composable
private fun TrendStat(value: String, label: String, trend: Trend?, muted: Color, onBg: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(value, style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.Normal)
            trend?.let {
                // §11 deltas: ↑ accent / ↓ muted — shape carries the direction, a contentDescription
                // gives TalkBack a word.
                val (glyph, color, desc) = when (it) {
                    Trend.UP -> Triple("↑", MaterialTheme.colorScheme.primary, "up from last session")
                    Trend.DOWN -> Triple("↓", muted, "down from last session")
                    Trend.SAME -> Triple("=", muted.copy(alpha = 0.7f), "same as last session")
                }
                Text(
                    glyph,
                    style = MaterialTheme.typography.labelSmall,
                    color = color,
                    fontSize = 8.sp,
                    modifier = Modifier.semantics { contentDescription = desc }
                )
            }
        }
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f), fontSize = 9.sp, letterSpacing = 0.5.sp)
    }
}

// ─── Per-exercise drill-in detail ──────────────────────────────────────────────

/**
 * The full breakdown for one exercise, shown when its row in a metric card is tapped: PR/e1RM/effort
 * chips, the exercise note, a one-line gist, the per-set chart (in the page's metric + the card's
 * bars/line style), and the complete set table. No outer card — the metric card already supplies it.
 */
@Composable
internal fun ExerciseDetailBody(
    ex: ExerciseDetail,
    metric: SessionMetric,
    style: SessionChartStyle,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    val useKg = LocalForgeSettings.current.useKg
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        val hasChip = ex.e1rmLb != null || ex.isPr || ex.effort != null
        if (hasChip) {
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                // A new best e1RM lights up like a PR; otherwise it's a quiet reference chip.
                ex.e1rmLb?.let { e ->
                    if (ex.e1rmIsBest) Chip("e1RM ${formatWeight(e, useKg)}", accent, accent.copy(alpha = 0.15f))
                    else Chip("e1RM ${formatWeight(e, useKg)}", muted, outline.copy(alpha = 0.12f))
                }
                if (ex.isPr) Chip("PR", accent, accent.copy(alpha = 0.15f))
                ex.effort?.let { Chip(it.displayName, effortColor(it), effortColor(it).copy(alpha = 0.15f)) }
            }
        }
        if (!ex.note.isNullOrBlank()) {
            Text("“${ex.note}”", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp)
        }
        Text(exerciseSummary(ex, useKg), style = MaterialTheme.typography.bodySmall, color = muted)
        PerExerciseSetChart(ex, metric, style, accent, muted, outline)
        SetTable(ex.sets, onBg, muted, outline)
    }
}

/** "Top 135 × 8 · 4 sets · 4,320 lb" — the collapsed-card gist. A timed hold reads "Best 0:45 · N sets". */
private fun exerciseSummary(ex: ExerciseDetail, useKg: Boolean): String {
    // A timed-hold exercise summarises by its longest hold, not weight×reps volume (GYMAP-51).
    if (ex.sets.isNotEmpty() && ex.sets.all { it.durationSeconds != null }) {
        val best = ex.sets.maxOf { it.durationSeconds ?: 0 }
        return "Best ${formatHoldLabel(best)} · ${ex.sets.size} ${if (ex.sets.size == 1) "set" else "sets"}"
    }
    val top = ex.sets.firstOrNull { it.isTopSet } ?: ex.sets.maxByOrNull { it.weightLb ?: 0.0 }
    return buildList {
        top?.let { add("Top ${weightLabel(it, useKg)} × ${it.reps}") }
        add("${ex.sets.size} ${if (ex.sets.size == 1) "set" else "sets"}")
        if (ex.volumeLb > 0) add(formatVolume(ex.volumeLb, useKg))
    }.joinToString(" · ")
}

@Composable
private fun SetTable(sets: List<SetDetail>, onBg: Color, muted: Color, outline: Color) {
    val useKg = LocalForgeSettings.current.useKg
    Column {
        sets.forEachIndexed { i, s ->
            // Table rule — one of the few sanctioned lines (§1: a line exists only as data).
            if (i > 0) EditorialHairline(outline)
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text("${s.number}", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f), fontSize = 10.sp, modifier = Modifier.width(14.dp))
                    Text(
                        // A timed hold reads its held time (0:45); every other set reads "weight × reps".
                        if (s.durationSeconds != null) formatHoldLabel(s.durationSeconds) else "${weightLabel(s, useKg)} × ${s.reps}",
                        style = MaterialTheme.typography.bodyMedium,
                        // Always white — the top set keeps a SemiBold emphasis but is no longer accent-tinted.
                        color = onBg,
                        fontWeight = if (s.isTopSet) FontWeight.SemiBold else FontWeight.Normal
                    )
                }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (s.setType == "warmup") SetTag("WARM", muted)
                    if (s.isAmrap) SetTag("AMRAP", muted)
                    if (s.toFailure) SetTag("FAIL", muted)
                    if (s.isAssisted) SetTag("ASSIST", muted)
                    // A drop set is marked by set_type; the annotation (drop weight/reps) is optional
                    // extra detail, so the badge keys off either (freestyle drops carry no annotation).
                    if (s.setType == "drop" || !s.dropAnnotation.isNullOrBlank()) SetTag("DROP", muted)
                    s.rpe?.let {
                        Text("RPE ${rpeLabel(it)}", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                    }
                }
            }
        }
    }
}

// ─── Small bits ────────────────────────────────────────────────────────────--

@Composable
private fun Chip(text: String, fg: Color, bg: Color) {
    Box(
        modifier = Modifier.clip(RoundedCornerShape(50)).background(bg).padding(horizontal = 8.dp, vertical = 2.dp)
    ) {
        Text(text, style = MaterialTheme.typography.labelSmall, color = fg, fontSize = 9.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.5.sp)
    }
}

/** Passive set metadata — bare mono text, no box (§1: a border is earned by interactivity). */
@Composable
private fun SetTag(text: String, muted: Color) {
    Text(text, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f), fontSize = 8.sp, letterSpacing = 0.5.sp)
}

private fun weightLabel(s: SetDetail, useKg: Boolean): String =
    if (s.weightLb != null && s.weightLb > 0) formatWeight(s.weightLb, useKg) else s.weightText.ifBlank { "BW" }

// §5 reserved state colors only — EASY and JUST_RIGHT are both healthy readings (success),
// HARD cautions, BRUTAL alarms. Never raw literals here.
private fun effortColor(effort: EffortRating): Color = when (effort) {
    EffortRating.EASY -> ForgeSuccess
    EffortRating.JUST_RIGHT -> ForgeSuccess
    EffortRating.HARD -> ForgeWarning
    EffortRating.BRUTAL -> ForgeError
}
