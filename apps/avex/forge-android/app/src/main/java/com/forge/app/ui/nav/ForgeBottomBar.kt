package com.forge.app.ui.nav

import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density

/**
 * The five primary hubs, in display order. They're pages of the [HubScreen] HorizontalPager, so the
 * bar is driven by page index (not nav routes). A tab's page is its index in the VISIBLE tab list,
 * which [HubScreen] resolves with `indexOf`: it matches the [ordinal] only while every tab is shown,
 * and Coach can be hidden. Home sits in the middle.
 */
enum class BottomTab(val label: String, val icon: ImageVector) {
    CARDIO("Cardio", OpenIcons.Activity),
    STATS("Stats", OpenIcons.BarChart),
    HOME("Home", OpenIcons.Home),
    COACH("Coach", OpenIcons.Compass),
    // Took Profile's slot on 2026-07-27: the Academy had been a link inside Coach, which buried the
    // half of the coach you can read. Profile moved to the Home top bar, where Settings used to sit.
    ACADEMY("Academy", OpenIcons.GraduationCap),
}

/**
 * The bottom bar, as the redesign draws it (2026-10-21): the page colour at 90% under a faint top
 * hairline, outline glyphs from the redesign's family, mono labels. The selected tab's glyph takes the
 * accent and its label the ink; the rest sit at the quiet muted rung. Each item is a full-height,
 * ≥48dp Tab-role target (a11y).
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
    val faint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.65f)
    val ink = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    val ground = MaterialTheme.colorScheme.background
    Column(
        Modifier
            .fillMaxWidth()
            .background(ground.copy(alpha = 0.9f))
            .drawBehind { drawRect(Color.White.copy(alpha = 0.08f), size = Size(size.width, 1.dp.toPx())) }
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 22.dp)
                .navigationBarsPadding(),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically
        ) {
            tabs.forEachIndexed { index, tab ->
                val selected = index == selectedIndex
                val color = if (selected) accent else faint
                Column(
                    // A minimum, not a fixed height, so a label at large text grows the bar instead
                    // of being cut. On the tab, not the row: fillMaxHeight under a row with only a
                    // minimum took the whole screen.
                    modifier = Modifier
                        .weight(1f)
                        .heightIn(min = 58.dp)
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
                    Spacer(Modifier.height(5.dp))
                    // Chrome clamps its own scaling (§14): at 2x, five tabs left "Academy" about
                    // 90dp of text in an 82dp tab, and it was cut to "Academ". At 1.3x it fits a
                    // 360dp phone's 72dp tab, and the bar grows to the label rather than clipping.
                    val d = LocalDensity.current
                    CompositionLocalProvider(
                        LocalDensity provides Density(d.density, d.fontScale.coerceAtMost(1.3f))
                    ) {
                        Text(
                            tab.label,
                            style = OpenType.monoXS.copy(letterSpacing = 0.sp),
                            color = if (selected) ink else faint
                        )
                    }
                }
            }
        }
    }
}
