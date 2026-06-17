package com.forge.app.ui.trophies

import androidx.compose.foundation.background
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
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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
import com.forge.app.program.Trophies
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.profile.AnimatedTrophyBadge
import com.forge.app.ui.trophies.state.TrophiesUiState
import com.forge.app.ui.trophies.state.TrophyDisplay
import com.forge.app.ui.trophies.state.TrophyFilter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
internal fun HeroSection(state: TrophiesUiState, nextLocked: TrophyDisplay?, onBg: Color, muted: Color, outline: Color) {
    val accent = MaterialTheme.colorScheme.primary
    val frac = if (state.totalCount == 0) 0f else state.unlockedCount.toFloat() / state.totalCount
    val animFrac by animateFloatAsState(frac.coerceIn(0f, 1f), label = "trophyProgress")
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .padding(top = 8.dp, bottom = 16.dp)
    ) {
        Text("${state.totalCount} IN ALL · ${state.unlockedCount} EARNED", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(14.dp))
        Box(modifier = Modifier.fillMaxWidth().height(3.dp).clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.2f))) {
            Box(modifier = Modifier.fillMaxWidth(animFrac).fillMaxHeight().clip(RoundedCornerShape(50)).background(accent))
        }
        if (state.unlockedCount == 0) {
            // An empty bar next to "0 EARNED" reads as broken — reassure it's a start line, not a bug.
            Spacer(Modifier.height(12.dp))
            InlineEmptyHint(
                "Nothing earned yet — every trophy below is up for grabs. Finish a workout to start unlocking them.",
                muted
            )
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
internal fun TrophyRow(
    display: TrophyDisplay,
    onBg: Color,
    muted: Color,
    bg: Color,
    outline: Color,
    active: Boolean = false,
    onToggle: () -> Unit = {},
    modifier: Modifier = Modifier
) {
    val accent = MaterialTheme.colorScheme.primary
    val unlocked = display.isUnlocked
    val nameColor = if (unlocked) onBg else muted.copy(alpha = 0.65f)
    val descColor = muted.copy(alpha = if (unlocked) 0.6f else 0.45f)
    Row(
        // Tapping a row brings its icon to life (the same per-icon motion as the profile trophy case)
        // and keeps it playing — [active] stays true until the screen clears it.
        modifier = modifier.fillMaxWidth().bounceClick { onToggle() }.padding(horizontal = 24.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        Box(contentAlignment = Alignment.BottomEnd) {
            AnimatedTrophyBadge(
                icon = display.trophy.icon,
                unlocked = unlocked,
                active = active,
                size = 40.dp,
                accent = accent,
                variant = Trophies.variantFor(display.trophy.id)
            )
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
                val animFrac by animateFloatAsState(frac.coerceIn(0f, 1f), label = "rowProgress")
                Spacer(Modifier.height(5.dp))
                Box(modifier = Modifier.fillMaxWidth().height(2.dp).clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.18f))) {
                    Box(modifier = Modifier.fillMaxWidth(animFrac).fillMaxHeight().clip(RoundedCornerShape(50)).background(accent.copy(alpha = 0.7f)))
                }
            }
        }
        if (unlocked) {
            val dateText = remember(display.unlockedAt) {
                "· " + SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(display.unlockedAt!!)).uppercase()
            }
            Text(dateText, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
        } else {
            Text(display.progressHint ?: "LOCKED", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f), fontSize = 9.sp)
        }
    }
}

@Composable
internal fun FilterChips(selected: TrophyFilter, onSelect: (TrophyFilter) -> Unit, onBg: Color, muted: Color, outline: Color) {
    val accent = MaterialTheme.colorScheme.primary
    Row(
        modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 24.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        TrophyFilter.entries.forEach { filter ->
            SegmentPill(
                text = filter.label,
                selected = filter == selected,
                onClick = { onSelect(filter) },
                accent = accent, onBg = onBg, muted = muted, outline = outline
            )
        }
    }
}
