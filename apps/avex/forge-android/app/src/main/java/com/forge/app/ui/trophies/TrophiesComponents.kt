package com.forge.app.ui.trophies

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.trophies.components.TrophyIconBadge
import com.forge.app.ui.trophies.state.TrophiesUiState
import com.forge.app.ui.trophies.state.TrophyDisplay
import com.forge.app.ui.trophies.state.TrophyFilter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
internal fun HeroSection(state: TrophiesUiState, nextLocked: TrophyDisplay?, onBg: Color, muted: Color, outline: Color) {
    val frac = if (state.totalCount == 0) 0f else state.unlockedCount.toFloat() / state.totalCount
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .padding(top = 8.dp, bottom = 16.dp)
    ) {
        Text("${state.totalCount} IN ALL · ${state.unlockedCount} EARNED", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(8.dp))
        Text("${numberWord(state.unlockedCount)} of ${numberWord(state.totalCount)}.", style = MaterialTheme.typography.displayLarge, color = onBg)
        Spacer(Modifier.height(14.dp))
        Box(modifier = Modifier.fillMaxWidth().height(2.dp).background(outline.copy(alpha = 0.2f))) {
            Box(modifier = Modifier.fillMaxWidth(frac.coerceIn(0f, 1f)).fillMaxHeight().background(onBg.copy(alpha = 0.7f)))
        }
        if (nextLocked != null) {
            Spacer(Modifier.height(12.dp))
            val nudgeText = buildAnnotatedString {
                append("Up next: ")
                withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(nextLocked.trophy.name) }
                append(" — ")
                append(nextLocked.trophy.description.replaceFirstChar { it.lowercase() })
                append(".")
            }
            Text(nudgeText, style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        }
        Spacer(Modifier.height(16.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
    }
}

@Composable
internal fun TrophyRow(display: TrophyDisplay, onBg: Color, muted: Color, bg: Color, outline: Color, modifier: Modifier = Modifier) {
    val unlocked = display.isUnlocked
    val nameColor = if (unlocked) onBg else muted.copy(alpha = 0.65f)
    val descColor = muted.copy(alpha = if (unlocked) 0.6f else 0.45f)
    Row(
        modifier = modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        Box(contentAlignment = Alignment.BottomEnd) {
            TrophyIconBadge(icon = display.trophy.icon, unlocked = unlocked, size = 40.dp)
            if (unlocked) {
                Box(modifier = Modifier.size(17.dp).clip(CircleShape).background(bg), contentAlignment = Alignment.Center) {
                    Box(modifier = Modifier.size(14.dp).clip(CircleShape).background(onBg), contentAlignment = Alignment.Center) {
                        Icon(Icons.Filled.Check, contentDescription = null, tint = bg, modifier = Modifier.size(9.dp))
                    }
                }
            }
        }
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(display.trophy.name, style = MaterialTheme.typography.bodyMedium, color = nameColor)
            Text(display.trophy.description, style = MaterialTheme.typography.bodySmall, color = descColor, fontStyle = FontStyle.Italic, fontSize = 11.sp)
            val frac = display.progressFraction
            if (!display.isUnlocked && frac != null && frac > 0f) {
                Spacer(Modifier.height(5.dp))
                Box(modifier = Modifier.fillMaxWidth().height(2.dp).background(outline.copy(alpha = 0.18f))) {
                    Box(modifier = Modifier.fillMaxWidth(frac.coerceIn(0f, 1f)).fillMaxHeight().background(onBg.copy(alpha = 0.5f)))
                }
            }
        }
        if (unlocked) {
            val dateText = remember(display.unlockedAt) {
                "· " + SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(display.unlockedAt!!)).uppercase()
            }
            Text(dateText, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
        } else {
            Text(display.progressHint ?: "LOCKED", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.4f), fontSize = 9.sp)
        }
    }
}

@Composable
internal fun FilterChips(selected: TrophyFilter, onSelect: (TrophyFilter) -> Unit, onBg: Color, muted: Color, outline: Color) {
    Row(
        modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 24.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        TrophyFilter.entries.forEach { filter ->
            val isSelected = filter == selected
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(50))
                    .border(0.5.dp, if (isSelected) onBg else outline.copy(alpha = 0.35f), RoundedCornerShape(50))
                    .background(if (isSelected) onBg.copy(alpha = 0.08f) else Color.Transparent)
                    .clickable { onSelect(filter) }
                    .padding(horizontal = 12.dp, vertical = 5.dp)
            ) {
                Text(filter.label, style = MaterialTheme.typography.labelSmall, color = if (isSelected) onBg else muted.copy(alpha = 0.65f), fontSize = 10.sp)
            }
        }
    }
}

internal fun numberWord(n: Int): String = when (n) {
    0 -> "Zero"; 1 -> "One"; 2 -> "Two"; 3 -> "Three"; 4 -> "Four"; 5 -> "Five"
    6 -> "Six"; 7 -> "Seven"; 8 -> "Eight"; 9 -> "Nine"; 10 -> "Ten"
    11 -> "Eleven"; 12 -> "Twelve"; 13 -> "Thirteen"; 14 -> "Fourteen"; 15 -> "Fifteen"
    16 -> "Sixteen"; 17 -> "Seventeen"; 18 -> "Eighteen"; 19 -> "Nineteen"; 20 -> "Twenty"
    21 -> "Twenty-one"; 22 -> "Twenty-two"; 23 -> "Twenty-three"; 24 -> "Twenty-four"
    25 -> "Twenty-five"; 26 -> "Twenty-six"; 27 -> "Twenty-seven"; 28 -> "Twenty-eight"
    29 -> "Twenty-nine"; 30 -> "Thirty"; 31 -> "Thirty-one"; 32 -> "Thirty-two"
    else -> n.toString()
}
