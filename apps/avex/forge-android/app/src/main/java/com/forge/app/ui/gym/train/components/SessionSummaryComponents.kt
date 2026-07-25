@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.gym.train.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.CountUpText
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import androidx.compose.ui.unit.sp
import com.forge.app.domain.mood.Mood
import com.forge.app.ui.gym.train.state.ExerciseHighlight

@Composable
internal fun FlatStat(value: String, label: String, onBg: Color, muted: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
        Text(value, style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.SemiBold)
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f), fontSize = 9.sp, letterSpacing = 0.5.sp)
    }
}

/**
 * Like [FlatStat] but the big number rolls up from 0 on first appearance (session-end celebration) —
 * reuses the Stats motion kit's [CountUpText] so reduced motion collapses it to an instant value.
 */
@Composable
internal fun CountUpStat(value: Double, label: String, onBg: Color, muted: Color, format: (Double) -> String) {
    Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
        CountUpText(
            value = value,
            style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold),
            color = onBg,
            fromValue = 0.0,
            format = format
        )
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f), fontSize = 9.sp, letterSpacing = 0.5.sp)
    }
}

@Composable
internal fun MoodPrompt(selected: Mood?, onSelect: (Mood) -> Unit, onBg: Color, muted: Color, outline: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("HOW DID IT FEEL?", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Mood.entries.forEach { mood ->
                MoodChip(
                    mood = mood,
                    isSelected = selected == mood,
                    onClick = { onSelect(mood) },
                    onBg = onBg,
                    muted = muted,
                    outline = outline,
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }
}

@Composable
internal fun MoodChip(mood: Mood, isSelected: Boolean, onClick: () -> Unit, onBg: Color, muted: Color, outline: Color, modifier: Modifier = Modifier) {
    // Crossfade the selection colours instead of hard-swapping them — the picker is prominent on the
    // summary, so the instant flip read cheap. Tweens collapse to instant under reduced motion.
    val bgColor by animateColorAsState(
        if (isSelected) onBg.copy(alpha = 0.08f) else Color.Transparent,
        animationSpec = ForgeMotion.standardTween(), label = "mood-bg"
    )
    val borderColor by animateColorAsState(
        if (isSelected) onBg else outline.copy(alpha = 0.35f),
        animationSpec = ForgeMotion.standardTween(), label = "mood-border"
    )
    val textColor by animateColorAsState(
        if (isSelected) onBg else muted.copy(alpha = 0.7f),
        animationSpec = ForgeMotion.standardTween(), label = "mood-text"
    )
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(4.dp))
            .border(0.5.dp, borderColor, RoundedCornerShape(4.dp))
            .background(bgColor)
            .bounceClick { onClick() }
            .padding(vertical = 8.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(mood.displayName, style = MaterialTheme.typography.labelSmall, color = textColor, fontSize = 10.sp)
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
    HorizontalDivider(color = outline.copy(alpha = 0.2f))
    Column(
        modifier = Modifier.fillMaxWidth().statsEntrance(1),
        verticalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Text(
            "WHAT THE COACH SEES",
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp,
            letterSpacing = 1.sp
        )
        coachOpinion?.let {
            Text(it, style = MaterialTheme.typography.bodySmall, color = onBg, lineHeight = 18.sp)
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

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(onBg.copy(alpha = 0.05f))
            .border(0.5.dp, outline.copy(alpha = 0.25f), RoundedCornerShape(8.dp))
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        if (complete) {
            Text(
                "Full effort data this session. He can read exactly how hard it landed and tune the next one precisely.",
                style = MaterialTheme.typography.bodySmall,
                color = onBg,
                lineHeight = 18.sp
            )
        } else {
            Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                CoverageStat("EFFORT", exercisesRated, exercisesLogged, "lifts", onBg, muted)
                if (totalSets > 0) CoverageStat("RPE", setsWithRpe, totalSets, "sets", onBg, muted)
            }
            Text(
                "Rate how hard each set feels next time and the coach can dial in your load and rest.",
                style = MaterialTheme.typography.bodySmall,
                color = muted,
                lineHeight = 18.sp
            )
        }
    }
}

/** One coverage figure for [CoachCaptureNudge]: a small caps label over a "done/total unit" count. */
@Composable
private fun CoverageStat(label: String, done: Int, total: Int, unit: String, onBg: Color, muted: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.6f),
            fontSize = 9.sp,
            letterSpacing = 0.5.sp
        )
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
            Text("$done/$total", style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.SemiBold)
            Text(unit, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        }
    }
}

@Composable
internal fun JournalField(value: String, onValueChange: (String) -> Unit, muted: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("SESSION JOURNAL", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        OutlinedTextField(
            value = value,
            onValueChange = onValueChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("How was the session overall?", style = MaterialTheme.typography.bodySmall) },
            minLines = 2,
            maxLines = 5
        )
    }
}

internal val SESSION_TAGS = listOf("Felt Strong", "Low Energy", "On Fire", "Sick", "Rushed", "Great Session")

@Composable
internal fun TagPicker(selected: Set<String>, onToggle: (String) -> Unit, onBg: Color, muted: Color, outline: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("TAG THIS SESSION", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            SESSION_TAGS.forEach { tag ->
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(4.dp))
                        .border(0.5.dp, if (tag in selected) onBg else outline.copy(alpha = 0.35f), RoundedCornerShape(4.dp))
                        .background(if (tag in selected) onBg.copy(alpha = 0.08f) else Color.Transparent)
                        .bounceClick { onToggle(tag) }
                        .padding(horizontal = 10.dp, vertical = 6.dp)
                ) {
                    Text(tag, style = MaterialTheme.typography.labelSmall, color = if (tag in selected) onBg else muted.copy(alpha = 0.7f), fontSize = 10.sp)
                }
            }
        }
    }
}

@Composable
internal fun HighlightRow(h: ExerciseHighlight, onBg: Color, muted: Color) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 5.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f)) {
            Text(h.exerciseName, style = MaterialTheme.typography.bodySmall, color = onBg)
            Text("${h.setsLogged} sets · ${formatVolume(h.volumeLb, weightUnit)}", style = MaterialTheme.typography.bodySmall, color = muted, fontSize = 10.sp)
        }
        if (h.isPr) {
            Text("PR", color = onBg, fontWeight = FontWeight.Black, style = MaterialTheme.typography.labelSmall, fontSize = 9.sp, letterSpacing = 0.5.sp)
        }
    }
}
