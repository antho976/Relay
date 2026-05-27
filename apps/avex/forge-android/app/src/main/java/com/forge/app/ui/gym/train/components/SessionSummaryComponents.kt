@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.mood.Mood
import com.forge.app.ui.gym.train.state.ExerciseHighlight
import com.forge.app.ui.gym.train.state.UnlockedTrophyHighlight
import com.forge.app.ui.trophies.components.TrophyIconBadge

@Composable
internal fun FlatStat(value: String, label: String, onBg: Color, muted: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
        Text(value, style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.SemiBold)
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
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(4.dp))
            .border(0.5.dp, if (isSelected) onBg else outline.copy(alpha = 0.35f), RoundedCornerShape(4.dp))
            .background(if (isSelected) onBg.copy(alpha = 0.08f) else Color.Transparent)
            .clickable { onClick() }
            .padding(vertical = 8.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(mood.displayName, style = MaterialTheme.typography.labelSmall, color = if (isSelected) onBg else muted.copy(alpha = 0.7f), fontSize = 10.sp)
    }
}

@Composable
internal fun TrophyUnlockRow(t: UnlockedTrophyHighlight, onBg: Color, muted: Color, outline: Color) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .border(0.5.dp, outline.copy(alpha = 0.25f), RoundedCornerShape(4.dp))
            .padding(12.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            TrophyIconBadge(icon = t.icon, unlocked = true, size = 40.dp)
            Column(modifier = Modifier.weight(1f).padding(start = 12.dp)) {
                Text(t.name, style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.SemiBold)
                Text(t.description, style = MaterialTheme.typography.bodySmall, color = muted)
            }
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
                        .clickable { onToggle(tag) }
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
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 5.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f)) {
            Text(h.exerciseName, style = MaterialTheme.typography.bodySmall, color = onBg)
            Text("${h.setsLogged} sets · ${h.volumeLb.toInt()} lb", style = MaterialTheme.typography.bodySmall, color = muted, fontSize = 10.sp)
        }
        if (h.isPr) {
            Text("PR", color = onBg, fontWeight = FontWeight.Black, style = MaterialTheme.typography.labelSmall, fontSize = 9.sp, letterSpacing = 0.5.sp)
        }
    }
}
