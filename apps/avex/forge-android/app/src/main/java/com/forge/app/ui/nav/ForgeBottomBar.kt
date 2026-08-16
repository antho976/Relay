package com.forge.app.ui.nav

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.CountBadge

/**
 * The five primary hubs, in display order. They're pages of the [HubScreen] HorizontalPager, so the
 * bar is driven by page index (not nav routes) — each tab's [ordinal] IS its pager page, so the rest
 * of the nav code refers to pages via e.g. [HOME]`.ordinal` rather than magic numbers. Home sits in
 * the middle.
 */
enum class BottomTab(val label: String, val icon: ImageVector) {
    CARDIO("Cardio", NavIcons.Cardio),
    STATS("Stats", NavIcons.Stats),
    HOME("Home", NavIcons.Home),
    COACH("Coach", NavIcons.Coach),
    // Took Profile's slot on 2026-07-27: the Academy had been a link inside Coach, which buried the
    // half of the coach you can read. Profile moved to the Home top bar, where Settings used to sit.
    ACADEMY("Academy", NavIcons.Academy),
}

/**
 * Minimal bottom navigation bar — a top hairline over the app's gradient (no solid footer), so it
 * reads as part of the pearl surface rather than a heavy chrome bar. The selected tab takes the
 * accent; the rest stay muted. Each item is a full-height, ≥48dp Tab-role target (a11y).
 */
@Composable
fun ForgeBottomBar(
    tabs: List<BottomTab> = BottomTab.entries.toList(),
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    /**
     * Unread counts per tab, drawn as a [CountBadge] on the icon. Only the Academy uses it today.
     *
     * §4.6 pushed back on a global unread badge as "a nag", and this is a deliberate narrowing of
     * that rule rather than an exception to it: the BELL counts everything waiting on you and stays
     * Home-only, while this counts one thing you can ignore forever and never appears on a tab that
     * has nothing. Recorded in `design/SETTLED.md`, 2026-08-15.
     */
    badges: Map<BottomTab, Int> = emptyMap()
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    Column(Modifier.fillMaxWidth()) {
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Row(
            Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .height(58.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically
        ) {
            tabs.forEachIndexed { index, tab ->
                val selected = index == selectedIndex
                val color = if (selected) accent else muted
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxHeight()
                        .selectable(
                            selected = selected,
                            role = Role.Tab,
                            onClick = { onSelect(index) }
                        ),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center
                ) {
                    val badge = badges[tab] ?: 0
                    Box(contentAlignment = Alignment.Center) {
                        Icon(tab.icon, contentDescription = null, tint = color, modifier = Modifier.size(22.dp))
                        // `offset` (not `absoluteOffset`) so the disc mirrors under RTL.
                        CountBadge(
                            badge,
                            Modifier.align(Alignment.TopEnd).offset(x = 10.dp, y = (-6).dp),
                            contentDescription = "$badge new"
                        )
                    }
                    Spacer(Modifier.height(3.dp))
                    Text(
                        tab.label,
                        style = MaterialTheme.typography.labelSmall,
                        color = color,
                        fontSize = 10.sp,
                        fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal
                    )
                }
            }
        }
    }
}
