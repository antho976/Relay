package com.tally.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.TrendingUp
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.prefs.Accent
import com.tally.app.ui.common.Caption
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupBlock
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PaceMeter
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.common.SwitchRow
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.accentForeground
import com.tally.core.Copy
import com.tally.core.MoneyFormatter
import com.tally.core.PaceReading

@Composable
fun AppearanceRoute(nav: AppNav) {
    val viewModel: AppearanceViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val actions = remember(viewModel, nav) {
        AppearanceActions(
            back = nav::back,
            setAmoled = viewModel::setAmoled,
            setAccentEnabled = viewModel::setAccentEnabled,
            setAccent = viewModel::setAccent,
        )
    }
    AppearanceScreen(state, actions)
}

/** Everything Appearance can change, as plain lambdas. Each write lands at once; there is no save. */
data class AppearanceActions(
    val back: () -> Unit = {},
    val setAmoled: (Boolean) -> Unit = {},
    val setAccentEnabled: (Boolean) -> Unit = {},
    val setAccent: (Accent) -> Unit = {},
)

/**
 * Appearance: a working sample of Home's hero under the current look, the two choices as tiles,
 * then the ground and the accent. The whole app re-themes as each one changes, the sample with it.
 */
@Composable
fun AppearanceScreen(state: AppearanceState, actions: AppearanceActions) {
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            PageHead(
                "Appearance",
                onBack = actions.back,
                context = appearanceSummary(state.amoled, state.accentEnabled, state.accent),
            )
        }
        item(key = "sample") {
            Column(Modifier.padding(horizontal = GUTTER)) {
                SampleHero()
                Caption("A sample of Home in your current look", Modifier.padding(start = 4.dp, top = 10.dp))
            }
        }
        item(key = "tiles") { LookTiles(state, Modifier.padding(horizontal = GUTTER)) }
        item(key = "theme") { ThemeGroup(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 10.dp)) }
        item(key = "accent") { AccentGroup(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 10.dp)) }
    }
}

/** A fixed, made-up month: 62% spent with the pace tick at 45%, so fill past the tick shows. */
private const val SAMPLE_DAYS = 31
private const val SAMPLE_ELAPSED = 14
private const val SAMPLE_BUDGET_UNITS = 2_900L
private const val SAMPLE_SPENT_UNITS = 1_798L

/** Home's left-to-spend hero in miniature, drawn with whatever accent and ground are set right now. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SampleHero(modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val reading = remember(money.fractionDigits) {
        val unit = MoneyFormatter.pow10(money.fractionDigits)
        PaceReading(SAMPLE_BUDGET_UNITS * unit, SAMPLE_SPENT_UNITS * unit, SAMPLE_DAYS, SAMPLE_ELAPSED)
    }
    HeroPanel(modifier) {
        HeroHead("Left to spend", end = "SAMPLE")
        Spacer(Modifier.height(14.dp))
        HeroNumber(money.formatWhole(reading.remaining), description = "Sample, " + money.formatWhole(reading.remaining) + " left to spend")
        Text(
            "left of your " + money.formatWhole(reading.budget) + " budget · " + money.formatWhole(reading.spent) + " spent",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        PaceMeter(
            reading.spentFraction,
            reading.paceFraction,
            "Sample meter: spent ${money.formatWhole(reading.spent)} of ${money.formatWhole(reading.budget)}, " +
                "an even pace would be ${money.formatWhole(reading.expected)}.",
            height = 14.dp,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.Speed, Copy.marginLine(reading, money))
            StatChip(Icons.AutoMirrored.Rounded.TrendingUp, Copy.paceLine(reading, money))
        }
    }
}

/** The two choices as readings: the accent in use and the ground under everything. */
@Composable
private fun LookTiles(state: AppearanceState, modifier: Modifier = Modifier) {
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                "Accent",
                if (state.accentEnabled) state.accent.label else "Off",
                m,
                detail = if (state.accentEnabled) "Picks, actions and charts" else "Black and white throughout",
            )
        },
        second = { m ->
            StatTile(
                "Ground",
                groundLabel(state.amoled),
                m,
                detail = if (state.amoled) "True black for OLED" else "Warm near-black",
            )
        },
    )
}

@Composable
private fun ThemeGroup(state: AppearanceState, actions: AppearanceActions, modifier: Modifier = Modifier) {
    val pureBlack: @Composable (Shape) -> Unit = { shape ->
        SwitchRow(
            "Pure black",
            checked = state.amoled,
            onToggle = actions.setAmoled,
            shape = shape,
            subtitle = "True-black backgrounds. Saves battery on OLED screens.",
        )
    }
    Group(rows = listOf(pureBlack), modifier = modifier, title = "Theme")
}

@Composable
private fun AccentGroup(state: AppearanceState, actions: AppearanceActions, modifier: Modifier = Modifier) {
    val toggle: @Composable (Shape) -> Unit = { shape ->
        SwitchRow(
            "Use an accent color",
            checked = state.accentEnabled,
            onToggle = actions.setAccentEnabled,
            shape = shape,
            subtitle = "Off keeps the app black and white",
        )
    }
    val swatches: @Composable (Shape) -> Unit = { shape ->
        GroupBlock(shape) { AccentSwatches(state.accent, state.accentEnabled, actions.setAccent) }
    }
    Group(
        rows = listOf(toggle, swatches),
        modifier = modifier,
        title = "Accent color",
        trailing = if (state.accentEnabled) state.accent.label else "Off",
        footer = "Category colors keep their own hues either way",
    )
}

/** Every accent as a 56dp circle, four a row; one radio group. Dimmed and inert while the accent is off. */
@Composable
private fun AccentSwatches(selected: Accent, enabled: Boolean, onPick: (Accent) -> Unit) {
    val rows = remember { Accent.entries.chunked(4) }
    Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        rows.forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                row.forEach { accent ->
                    AccentSwatch(accent, accent == selected, enabled) { onPick(accent) }
                }
            }
        }
    }
}

@Composable
private fun AccentSwatch(accent: Accent, selected: Boolean, enabled: Boolean, onClick: () -> Unit) {
    val color = Color(accent.argb)
    val ring = MaterialTheme.colorScheme.onBackground
    val mark = accentForeground(color, dark = MaterialTheme.colorScheme.background, light = MaterialTheme.colorScheme.onBackground)
    Box(
        Modifier
            .size(56.dp)
            .alpha(if (enabled) 1f else 0.35f)
            .clip(CircleShape)
            .semantics(mergeDescendants = true) { contentDescription = accent.label }
            .selectable(selected = selected, enabled = enabled, role = Role.RadioButton, onClick = onClick)
            .then(if (selected) Modifier.border(2.dp, ring, CircleShape) else Modifier)
            .padding(6.dp)
            .clip(CircleShape)
            .background(color),
        contentAlignment = Alignment.Center,
    ) {
        if (selected) {
            Icon(Icons.Rounded.Check, contentDescription = null, tint = mark, modifier = Modifier.size(24.dp))
        }
    }
}
