package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.mood.Mood
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.gym.train.state.SessionSummary
import com.forge.app.ui.gym.train.state.UnlockedTrophyHighlight

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionSummarySheet(
    summary: SessionSummary,
    onDismiss: (mood: Mood?, tags: List<String>, journal: String) -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var selectedMood by remember { mutableStateOf<Mood?>(null) }
    var selectedTags by remember { mutableStateOf<Set<String>>(emptySet()) }
    var journal by remember { mutableStateOf("") }
    var showTrophyConfetti by remember { mutableStateOf(summary.unlockedTrophies.isNotEmpty()) }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background

    ModalBottomSheet(
        onDismissRequest = {},
        sheetState = sheetState,
        containerColor = bg
    ) {
        Box {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 24.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                // Header
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        summary.dayWord.uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 1.5.sp,
                        color = muted,
                        fontSize = 9.sp
                    )
                    Text(
                        summary.displayName,
                        style = MaterialTheme.typography.headlineSmall,
                        color = onBg
                    )
                    Text(
                        "workout complete",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted,
                        fontSize = 11.sp,
                        fontStyle = FontStyle.Italic
                    )
                }

                // Primary stats strip
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                    FlatStat(value = "${summary.totalVolumeLb.toInt()} lb", label = "VOLUME", onBg = onBg, muted = muted)
                    FlatStat(value = "${summary.prCount}", label = "PRs", onBg = onBg, muted = muted)
                    FlatStat(value = "${summary.durationMinutes} min", label = "TIME", onBg = onBg, muted = muted)
                }

                // Secondary meta stats
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                    FlatStat(value = "${summary.setCount}", label = "SETS", onBg = onBg, muted = muted)
                    FlatStat(
                        value = "${summary.exercisesLogged}" + (if (summary.exercisesSkipped > 0) " · ${summary.exercisesSkipped} skipped" else ""),
                        label = "EXERCISES",
                        onBg = onBg,
                        muted = muted
                    )
                }

                // Session efficiency metrics (#83, #127, #82, #133)
                val hasEfficiency = summary.densityScore != null || summary.avgRestSeconds != null || summary.honestyPct != null
                if (hasEfficiency) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Text("SESSION METRICS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                        summary.densityScore?.let { density ->
                            FlatStat(value = "${density.toInt()} lb/min", label = "DENSITY", onBg = onBg, muted = muted)
                        }
                        summary.avgRestSeconds?.let { rest ->
                            val restStr = if (rest >= 60) "${rest / 60}m ${rest % 60}s" else "${rest}s"
                            FlatStat(value = restStr, label = "AVG REST", onBg = onBg, muted = muted)
                        }
                        summary.honestyPct?.let { pct ->
                            FlatStat(value = "$pct%", label = "COMPLETION", onBg = onBg, muted = muted)
                        }
                    }
                }

                // Session comparison vs last same-day session (#52)
                val hasComparison = summary.vsLastVolumeDelta != null || summary.vsLastSetsDelta != null
                if (hasComparison) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                        summary.vsLastVolumeDelta?.let { delta ->
                            val sign = if (delta >= 0) "+" else ""
                            FlatStat(value = "$sign${delta.toInt()} lb", label = "vs LAST", onBg = onBg, muted = muted)
                        }
                        summary.vsLastSetsDelta?.let { delta ->
                            val sign = if (delta >= 0) "+" else ""
                            FlatStat(value = "$sign$delta sets", label = "vs LAST", onBg = onBg, muted = muted)
                        }
                    }
                }

                // Best session callout (#53)
                if (summary.isBestSession) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .border(0.5.dp, outline.copy(alpha = 0.3f), RoundedCornerShape(4.dp))
                            .padding(horizontal = 14.dp, vertical = 10.dp)
                    ) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text("★", color = onBg, fontWeight = FontWeight.Black, style = MaterialTheme.typography.titleSmall)
                            Text("Your best ${summary.dayWord} ever", style = MaterialTheme.typography.bodySmall, color = onBg)
                        }
                    }
                }

                if (summary.highlights.isNotEmpty()) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Text("EXERCISES", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                    summary.highlights.forEach { h -> HighlightRow(h, onBg = onBg, muted = muted) }
                }

                if (summary.unlockedTrophies.isNotEmpty()) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Text("TROPHIES UNLOCKED", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                    summary.unlockedTrophies.forEach { t -> TrophyUnlockRow(t, onBg = onBg, muted = muted, outline = outline) }
                }

                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                MoodPrompt(
                    selected = selectedMood,
                    onSelect = { mood -> selectedMood = if (selectedMood == mood) null else mood },
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )

                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                JournalField(value = journal, onValueChange = { journal = it }, muted = muted)

                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                TagPicker(
                    selected = selectedTags,
                    onToggle = { tag ->
                        selectedTags = if (tag in selectedTags) selectedTags - tag else selectedTags + tag
                    },
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )

                // Complete button — only way to dismiss
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 8.dp, bottom = 16.dp)
                        .clip(RoundedCornerShape(4.dp))
                        .border(0.5.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(4.dp))
                        .clickable { onDismiss(selectedMood, selectedTags.toList(), journal) }
                        .padding(vertical = 16.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        "COMPLETE",
                        style = MaterialTheme.typography.labelSmall,
                        color = onBg,
                        letterSpacing = 1.5.sp
                    )
                }
            }

            // Trophy confetti (#30): fires once when sheet opens with unlocked trophies
            if (showTrophyConfetti) {
                ConfettiOverlay(
                    modifier = Modifier.matchParentSize(),
                    onComplete = { showTrophyConfetti = false }
                )
            }
        }
    }
}

