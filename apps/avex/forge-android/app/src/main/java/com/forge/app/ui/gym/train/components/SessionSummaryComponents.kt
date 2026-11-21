@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.gym.train.components

import com.forge.app.ui.common.ROW_H

import com.forge.app.ui.common.GROUP_OUTER

import androidx.compose.foundation.layout.height

import androidx.compose.foundation.layout.Spacer

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.formatVolume
import com.forge.app.ui.common.CountUpText
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.theme.LocalForgeSettings
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.train.state.ExerciseHighlight

@Composable
internal fun FlatStat(value: String, label: String, modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(value, style = OpenType.figureMS, color = MaterialTheme.colorScheme.onBackground)
        OpenLabel(label, style = OpenType.monoXS)
    }
}

/**
 * Like [FlatStat] but the figure rolls up from 0 on first appearance (session-end celebration) —
 * reuses the Stats motion kit's [CountUpText] so reduced motion collapses it to an instant value.
 */
@Composable
internal fun CountUpStat(value: Double, label: String, modifier: Modifier = Modifier, format: (Double) -> String) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        CountUpText(
            value = value,
            style = OpenType.figureMS,
            color = MaterialTheme.colorScheme.onBackground,
            fromValue = 0.0,
            format = format
        )
        OpenLabel(label, style = OpenType.monoXS)
    }
}

/**
 * The coach's corner of the summary: first the celebratory [coachOpinion] (what he reads from THIS
 * session), then the [CoachCaptureNudge] showing how much effort signal the session actually carried
 * (per-set RPE + per-exercise "how hard it felt") with an ask to log more so he can calibrate load
 * and rest. Purely informational; effort capture itself stays inline during the session.
 */
@Composable
internal fun CoachReadSection(
    coachOpinion: String?,
    setsWithRpe: Int,
    totalSets: Int,
    exercisesRated: Int,
    exercisesLogged: Int,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    // Nothing logged → nothing for the coach to read.
    if (coachOpinion == null && exercisesLogged == 0) return
    // Air, not a rule, above the section (grouped-surface pass, 2026-09-27).
    Spacer(Modifier.height(4.dp))
    Column(
        modifier = Modifier.fillMaxWidth().statsEntrance(1),
        verticalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        OpenLabel("What the coach sees")
        coachOpinion?.let {
            Text(it, style = OpenType.body, color = onBg)
        }
        CoachCaptureNudge(setsWithRpe, totalSets, exercisesRated, exercisesLogged, onBg, muted, outline)
    }
}

/**
 * The "give me more to work with" block. Hidden for an empty session; a quiet confirmation when every
 * lift + set carried its effort signal; otherwise the coverage is shown as two stat-style figures
 * (effort per lift, RPE per set) above a short ask. Surfacing the numbers — rather than burying them
 * in an italic line — makes it obvious at a glance how much the coach actually had to read.
 */
@Composable
private fun CoachCaptureNudge(
    setsWithRpe: Int,
    totalSets: Int,
    exercisesRated: Int,
    exercisesLogged: Int,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    if (exercisesLogged == 0) return
    val effortComplete = exercisesRated >= exercisesLogged
    val rpeComplete = totalSets == 0 || setsWithRpe >= totalSets
    val complete = effortComplete && rpeComplete

    // No card around it: the figures and the ask sit on the sheet like everything else here.
    Column(
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        if (complete) {
            Text(
                "Full effort data this session. He can read exactly how hard it landed and tune the next one precisely.",
                style = OpenType.bodyS,
                color = onBg
            )
        } else {
            Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                CoverageStat("EFFORT", exercisesRated, exercisesLogged, "lifts", onBg, muted)
                if (totalSets > 0) CoverageStat("RPE", setsWithRpe, totalSets, "sets", onBg, muted)
            }
            Text(
                "Rate how hard each set feels next time and the coach can dial in your load and rest.",
                style = OpenType.bodyS,
                color = muted
            )
        }
    }
}

/** One coverage figure for [CoachCaptureNudge]: a mono label over a serif "done/total" and its unit. */
@Composable
private fun CoverageStat(label: String, done: Int, total: Int, unit: String, onBg: Color, muted: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        OpenLabel(label, style = OpenType.monoXS)
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text("$done/$total", style = OpenType.figureS, color = onBg)
            OpenLabel(unit, style = OpenType.monoXS, color = muted)
        }
    }
}

/** One exercise of the session: its name, "3 sets · 2,400 lb" in mono, and a PR tag when it set one. */
@Composable
internal fun HighlightRow(h: ExerciseHighlight) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(h.exerciseName, style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground)
            OpenLabel("${h.setsLogged} ${if (h.setsLogged == 1) "set" else "sets"} · ${formatVolume(h.volumeLb, weightUnit)}", style = OpenType.monoXS)
        }
        if (h.isPr) com.forge.app.ui.common.OpenPrTag()
    }
}
