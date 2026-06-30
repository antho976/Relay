package com.forge.app.ui.gym.stats

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/** The Stats sub-tabs. Each groups a few tiers so the screen is short lists, not one long scroll.
 *  OVERVIEW is the landing tab — a populated at-a-glance read so first open is never blank. */
enum class StatsTab(val label: String) {
    OVERVIEW("Overview"),
    STRENGTH("Strength"),
    VOLUME("Volume"),
    BODY("Body"),
    TRENDS("Trends")
}

/**
 * Tap-only segmented sub-nav (filled pill = selected). Tap-only on purpose: the hub-level
 * HorizontalPager owns left/right swipe, so a nested pager would fight it — tab content crossfades
 * instead. Horizontally scrollable so the row never crowds on a narrow phone.
 */
@Composable
internal fun StatsTabRow(selected: StatsTab, onSelect: (StatsTab) -> Unit, c: StatsColors) {
    val bg = MaterialTheme.colorScheme.background
    Row(
        Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = STATS_GUTTER, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        StatsTab.entries.forEach { tab ->
            val isSel = tab == selected
            val pill by animateColorAsState(if (isSel) c.onBg else Color.Transparent, label = "pill")
            val borderC by animateColorAsState(
                if (isSel) Color.Transparent else c.outline.copy(alpha = 0.5f), label = "border"
            )
            val txt by animateColorAsState(if (isSel) bg else c.muted, label = "txt")
            Box(
                Modifier
                    .clip(RoundedCornerShape(50))
                    .background(pill)
                    .border(1.dp, borderC, RoundedCornerShape(50))
                    .clickable(onClickLabel = "View ${tab.label}") { onSelect(tab) }
                    .semantics { this.selected = isSel; role = Role.Tab }
                    .padding(horizontal = 14.dp, vertical = 7.dp)
            ) {
                Text(
                    tab.label,
                    style = MaterialTheme.typography.labelMedium,
                    color = txt,
                    fontWeight = if (isSel) FontWeight.SemiBold else FontWeight.Normal
                )
            }
        }
    }
    HorizontalDivider(color = c.outline.copy(alpha = 0.2f))
}
