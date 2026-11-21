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
 * The coach's corner of the summary: the one-line [coachOpinion] (what the coach reads from THIS
 * session), then [EffortCatchUp] for any exercise it could not read the effort of.
 */
@Composable
internal fun CoachReadSection(
    coachOpinion: String?,
    unrated: List<com.forge.app.ui.gym.train.state.UnratedLift>,
    onRateSet: (setId: Long, rpe: Double) -> Unit,
    onBg: Color
) {
    // Nothing logged → nothing for the coach to read.
    if (coachOpinion == null && unrated.isEmpty()) return
    // Air, not a rule, above the section (grouped-surface pass, 2026-09-27).
    Spacer(Modifier.height(4.dp))
    Column(
        modifier = Modifier.fillMaxWidth().statsEntrance(1),
        verticalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        OpenLabel("Coach")
        coachOpinion?.let {
            Text(it, style = OpenType.body, color = onBg)
        }
        EffortCatchUp(unrated, onRateSet)
    }
}

/**
 * The effort question for everything the session left unrated, in one place: each exercise's name
 * over the same five reps-left keys the card uses. A pick is written straight to that exercise's
 * last working set and stays shown, so it can be changed until the sheet is closed.
 *
 * This replaced a pair of coverage figures ("EFFORT 2/5 lifts", "RPE 3/14 sets") and a line asking
 * for more next time: it reported the gap at the one moment it could have been closed instead.
 */
@Composable
private fun EffortCatchUp(
    unrated: List<com.forge.app.ui.gym.train.state.UnratedLift>,
    onRateSet: (setId: Long, rpe: Double) -> Unit
) {
    if (unrated.isEmpty()) return
    // What was picked here, by set: the summary is a snapshot, so the sheet keeps its own answers.
    val picks = androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateMapOf<Long, Double>() }
    Column(
        modifier = Modifier.fillMaxWidth().padding(top = 10.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        Text(
            if (unrated.size == 1) "One lift has no effort yet. How many more could you have done on its last set?"
            else "${unrated.size} lifts have no effort yet. How many more could you have done on each last set?",
            style = OpenType.bodyS,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        unrated.forEach { lift ->
            RepsLeftRow(
                rpe = picks[lift.setId],
                onPick = { rpe ->
                    picks[lift.setId] = rpe
                    onRateSet(lift.setId, rpe)
                },
                label = lift.exerciseName
            )
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
