package com.tally.app.ui.nav

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.BarChart
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.Savings
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.FloatingActionButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.home.HomeTab
import com.tally.app.ui.insights.InsightsTab
import com.tally.app.ui.plan.PlanTab
import com.tally.app.ui.theme.TallyMotion

/**
 * The tabs. Activity was one until Home took its place (2026-10-05, the owner: "no need to have
 * it twice"): Home's Recent is the trim and its "view all" opens the full History, as in Avex.
 */
enum class HubTab(val label: String, val icon: ImageVector) {
    HOME("Home", Icons.Rounded.Home),
    PLAN("Plan", Icons.Rounded.Savings),
    INSIGHTS("Insights", Icons.Rounded.BarChart),
}

/** How far the root snackbar must sit above the bottom, so it never covers the nav bar or FAB. */
val LocalSnackbarLift = staticCompositionLocalOf { mutableStateOf(0.dp) }

/**
 * The hub: three tabs with one add FAB that stays put across them, because logging is the most
 * frequent act and must cost one tap from anywhere. Compact width gets a bottom bar; 600dp and up
 * a rail, so a tablet never wears a stretched phone bar. Each tab keeps its scroll position.
 * Home is the start: Back from any other tab returns there, and only Back on Home leaves the app.
 */
@Composable
fun Hub(nav: AppNav) {
    var tab by rememberSaveable { mutableStateOf(HubTab.HOME) }
    val holder = rememberSaveableStateHolder()
    val lift = LocalSnackbarLift.current
    BackHandler(enabled = tab != HubTab.HOME) { tab = HubTab.HOME }

    BoxWithConstraints(Modifier.fillMaxSize()) {
        val wide = maxWidth >= 600.dp
        DisposableEffect(wide) {
            lift.value = if (wide) 88.dp else 160.dp
            onDispose { lift.value = 0.dp }
        }
        val content: @Composable (Modifier) -> Unit = { modifier ->
            AnimatedContent(
                targetState = tab,
                modifier = modifier,
                transitionSpec = { fadeIn(TallyMotion.enter(TallyMotion.Fast)) togetherWith fadeOut(TallyMotion.exit(TallyMotion.Fast)) },
                label = "hub_tab",
            ) { current ->
                holder.SaveableStateProvider(current.name) {
                    when (current) {
                        HubTab.HOME -> HomeTab(nav, openTab = { tab = it })
                        HubTab.PLAN -> PlanTab(nav)
                        HubTab.INSIGHTS -> InsightsTab(nav)
                    }
                }
            }
        }
        if (wide) {
            Row(Modifier.fillMaxSize()) {
                HubRail(tab, onSelect = { tab = it }, onAdd = { nav.entry() })
                content(Modifier.weight(1f).fillMaxHeight())
            }
        } else {
            Column(Modifier.fillMaxSize()) {
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    content(Modifier.fillMaxSize())
                    AddFab(
                        onClick = { nav.entry() },
                        modifier = Modifier.align(Alignment.BottomEnd).padding(end = 20.dp, bottom = 16.dp),
                    )
                }
                HubBottomBar(tab, onSelect = { tab = it })
            }
        }
    }
}

@Composable
private fun AddFab(onClick: () -> Unit, modifier: Modifier = Modifier) {
    FloatingActionButton(
        onClick = onClick,
        modifier = modifier,
        shape = RoundedCornerShape(16.dp),
        containerColor = MaterialTheme.colorScheme.primary,
        contentColor = MaterialTheme.colorScheme.onPrimary,
        elevation = FloatingActionButtonDefaults.elevation(defaultElevation = 2.dp, pressedElevation = 4.dp),
    ) {
        Icon(Icons.Rounded.Add, contentDescription = "Add entry", modifier = Modifier.size(28.dp))
    }
}

/**
 * The Material 3 bar on a raised warm surface: a glyph over a label for each tab, the picked one
 * in the accent on its indicator pill. Each item is a full-height Tab-role target.
 */
@Composable
private fun HubBottomBar(selected: HubTab, onSelect: (HubTab) -> Unit) {
    Column(
        Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceContainerLow)
    ) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(MaterialTheme.colorScheme.outlineVariant))
        Row(
            Modifier.fillMaxWidth().navigationBarsPadding().height(72.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            HubTab.entries.forEach { t ->
                NavItem(t, t == selected, Modifier.weight(1f).fillMaxHeight()) { onSelect(t) }
            }
        }
    }
}

@Composable
private fun HubRail(selected: HubTab, onSelect: (HubTab) -> Unit, onAdd: () -> Unit) {
    Column(
        Modifier.fillMaxHeight().width(88.dp).statusBarsPadding().navigationBarsPadding().padding(vertical = 16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        AddFab(onAdd)
        Spacer(Modifier.height(16.dp))
        HubTab.entries.forEach { t ->
            NavItem(t, t == selected, Modifier.fillMaxWidth().height(72.dp)) { onSelect(t) }
        }
    }
}

@Composable
private fun NavItem(tab: HubTab, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant
    val pill by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.16f) else Color.Transparent,
        TallyMotion.standard(TallyMotion.Fast),
        label = "nav_pill",
    )
    Column(
        modifier.selectable(selected = selected, role = Role.Tab, onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            Modifier.width(60.dp).height(32.dp).clip(RoundedCornerShape(16.dp)).background(pill),
            contentAlignment = Alignment.Center,
        ) {
            Icon(tab.icon, contentDescription = null, tint = color, modifier = Modifier.size(24.dp))
        }
        Spacer(Modifier.height(4.dp))
        Text(
            tab.label,
            style = MaterialTheme.typography.bodySmall,
            color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
        )
    }
}

/** Bottom space a tab's scroll content leaves so the FAB never covers its last row. */
val FAB_CLEARANCE: Dp = 96.dp

