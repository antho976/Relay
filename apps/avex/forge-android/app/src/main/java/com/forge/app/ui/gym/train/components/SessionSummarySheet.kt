package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.material3.OutlinedTextField
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.mood.Mood
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.gym.train.state.SessionSummary
import com.forge.app.ui.gym.train.state.UnlockedTrophyHighlight
import com.forge.app.ui.trophies.components.TrophyIconBadge

/**
 * End-of-workout celebration sheet. Modal: must be dismissed to return to the day list.
 */
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
    ModalBottomSheet(
        onDismissRequest = { onDismiss(selectedMood, selectedTags.toList(), journal) },
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        Box {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Column {
                Text(
                    "WORKOUT COMPLETE",
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.Black,
                    style = MaterialTheme.typography.labelLarge
                )
                Text(
                    "${summary.displayName} · ${summary.dayWord}",
                    style = MaterialTheme.typography.headlineLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
            }

            // Big stats row
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                StatTile(
                    label = "Volume",
                    value = "${summary.totalVolumeLb.toInt()} lb",
                    modifier = Modifier.weight(1f)
                )
                StatTile(
                    label = "PRs",
                    value = "${summary.prCount}",
                    highlight = summary.prCount > 0,
                    modifier = Modifier.weight(1f)
                )
                StatTile(
                    label = "Time",
                    value = "${summary.durationMinutes}m",
                    modifier = Modifier.weight(1f)
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                MetaStat(
                    label = "Sets logged",
                    value = "${summary.setCount}",
                    modifier = Modifier.weight(1f)
                )
                MetaStat(
                    label = "Exercises",
                    value = "${summary.exercisesLogged}" + (
                        if (summary.exercisesSkipped > 0) " · ${summary.exercisesSkipped} skipped" else ""
                    ),
                    modifier = Modifier.weight(1f)
                )
            }

            // Session efficiency metrics (#83, #127, #82, #133)
            val hasEfficiency = summary.densityScore != null || summary.avgRestSeconds != null || summary.honestyPct != null
            if (hasEfficiency) {
                HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))
                Text(
                    "SESSION METRICS",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontWeight = FontWeight.SemiBold
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    summary.densityScore?.let { density ->
                        MetaStat(
                            label = "Density",
                            value = "${density.toInt()} lb/min",
                            modifier = Modifier.weight(1f)
                        )
                    }
                    summary.avgRestSeconds?.let { rest ->
                        val restStr = if (rest >= 60) "${rest / 60}m ${rest % 60}s" else "${rest}s"
                        MetaStat(
                            label = "Avg rest",
                            value = restStr,
                            modifier = Modifier.weight(1f)
                        )
                    }
                    summary.honestyPct?.let { pct ->
                        MetaStat(
                            label = "Completion",
                            value = "$pct%",
                            modifier = Modifier.weight(1f)
                        )
                    }
                }
            }

            // Session comparison vs last same-day session (#52)
            val hasComparison = summary.vsLastVolumeDelta != null || summary.vsLastSetsDelta != null
            if (hasComparison) {
                HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))
                SessionComparisonRow(
                    volumeDelta = summary.vsLastVolumeDelta,
                    setsDelta = summary.vsLastSetsDelta
                )
            }

            // Best session callout (#53)
            if (summary.isBestSession) {
                BestSessionCallout(dayWord = summary.dayWord)
            }

            if (summary.highlights.isNotEmpty()) {
                HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))
                Text(
                    "EXERCISES",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontWeight = FontWeight.SemiBold
                )
                summary.highlights.forEach { h ->
                    HighlightRow(h)
                }
            }

            if (summary.unlockedTrophies.isNotEmpty()) {
                HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))
                Text(
                    "TROPHIES UNLOCKED",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.Black
                )
                summary.unlockedTrophies.forEach { t -> TrophyUnlockRow(t) }
            }

            HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))
            MoodPrompt(
                selected = selectedMood,
                onSelect = { mood -> selectedMood = if (selectedMood == mood) null else mood }
            )

            HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))
            JournalField(value = journal, onValueChange = { journal = it })

            HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))
            TagPicker(
                selected = selectedTags,
                onToggle = { tag ->
                    selectedTags = if (tag in selectedTags) selectedTags - tag else selectedTags + tag
                }
            )

            Button(
                onClick = { onDismiss(selectedMood, selectedTags.toList(), journal) },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp, bottom = 16.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.primary
                )
            ) {
                Text("Done")
            }
        }
        // Trophy confetti (#30): fires once when sheet opens with unlocked trophies
        if (showTrophyConfetti) {
            ConfettiOverlay(
                modifier = Modifier.matchParentSize(),
                onComplete = { showTrophyConfetti = false }
            )
        }
        } // end Box
    }
}

@Composable
private fun StatTile(
    label: String,
    value: String,
    highlight: Boolean = false,
    modifier: Modifier = Modifier
) {
    val bg = if (highlight) MaterialTheme.colorScheme.primary.copy(alpha = 0.18f)
             else MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f)
    val fg = if (highlight) MaterialTheme.colorScheme.primary
             else MaterialTheme.colorScheme.onSurface
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .background(bg)
            .padding(vertical = 16.dp, horizontal = 8.dp)
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
            Text(value, fontSize = 28.sp, fontWeight = FontWeight.Black, color = fg)
            Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun MetaStat(
    label: String,
    value: String,
    modifier: Modifier = Modifier,
    valueColor: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.onSurface
) {
    Column(modifier = modifier) {
        Text(value, style = MaterialTheme.typography.titleLarge, color = valueColor)
        Text(label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun MoodPrompt(
    selected: Mood?,
    onSelect: (Mood) -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            "HOW DID IT FEEL?",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Mood.entries.forEach { mood ->
                MoodChip(
                    mood = mood,
                    isSelected = selected == mood,
                    onClick = { onSelect(mood) },
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }
}

@Composable
private fun MoodChip(
    mood: Mood,
    isSelected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val bg: Color = if (isSelected) MaterialTheme.colorScheme.primary.copy(alpha = 0.22f)
                    else MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f)
    val border = if (isSelected) MaterialTheme.colorScheme.primary else Color.Transparent
    Surface(
        modifier = modifier,
        color = bg,
        shape = RoundedCornerShape(12.dp),
        border = BorderStroke(1.dp, border),
        onClick = onClick
    ) {
        Column(
            modifier = Modifier.padding(vertical = 10.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(2.dp)
        ) {
            Text(mood.emoji, fontSize = 22.sp)
            Text(
                mood.displayName,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
private fun TrophyUnlockRow(t: UnlockedTrophyHighlight) {
    Surface(
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.primary.copy(alpha = 0.10f),
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            TrophyIconBadge(icon = t.icon, unlocked = true, size = 40.dp)
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(start = 12.dp)
            ) {
                Text(
                    t.name,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    fontWeight = FontWeight.SemiBold
                )
                Text(
                    t.description,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun SessionComparisonRow(volumeDelta: Double?, setsDelta: Int?) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        if (volumeDelta != null) {
            val positive = volumeDelta >= 0
            val sign = if (positive) "+" else ""
            val color = if (positive) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.error
            MetaStat(
                label = "vs last session",
                value = "$sign${volumeDelta.toInt()} lb",
                modifier = Modifier.weight(1f),
                valueColor = color
            )
        }
        if (setsDelta != null) {
            val positive = setsDelta >= 0
            val sign = if (positive) "+" else ""
            val color = if (positive) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.error
            MetaStat(
                label = "sets vs last",
                value = "$sign$setsDelta",
                modifier = Modifier.weight(1f),
                valueColor = color
            )
        }
    }
}

@Composable
private fun BestSessionCallout(dayWord: String) {
    Surface(
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.primary.copy(alpha = 0.13f),
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text("★", color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Black)
            Text(
                "Your best $dayWord ever",
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.primary,
                fontWeight = FontWeight.SemiBold
            )
        }
    }
}

@Composable
private fun JournalField(value: String, onValueChange: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            "SESSION JOURNAL",
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        OutlinedTextField(
            value = value,
            onValueChange = onValueChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("How was the session overall?", style = MaterialTheme.typography.bodyMedium) },
            minLines = 2,
            maxLines = 5
        )
    }
}

private val SESSION_TAGS = listOf("💪 Felt Strong", "😅 Low Energy", "🔥 On Fire", "😷 Sick", "⚡ Rushed", "✅ Great Session")

@OptIn(ExperimentalLayoutApi::class, ExperimentalMaterial3Api::class)
@Composable
private fun TagPicker(selected: Set<String>, onToggle: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            "TAG THIS SESSION",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold
        )
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            SESSION_TAGS.forEach { tag ->
                FilterChip(
                    selected = tag in selected,
                    onClick = { onToggle(tag) },
                    label = { Text(tag, style = MaterialTheme.typography.labelMedium) },
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f),
                        selectedLabelColor = MaterialTheme.colorScheme.primary
                    )
                )
            }
        }
    }
}

@Composable
private fun HighlightRow(h: com.forge.app.ui.gym.train.state.ExerciseHighlight) {
    Surface(
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.3f),
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    h.exerciseName,
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    "${h.setsLogged} sets · ${h.volumeLb.toInt()} lb",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            if (h.isPr) {
                Text(
                    "PR",
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.Black,
                    style = MaterialTheme.typography.labelLarge
                )
            }
        }
    }
}
