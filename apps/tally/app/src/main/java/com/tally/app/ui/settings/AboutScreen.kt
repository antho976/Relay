package com.tally.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CloudOff
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material.icons.rounded.NoAccounts
import androidx.compose.material.icons.rounded.Science
import androidx.compose.material.icons.rounded.Shield
import androidx.compose.material.icons.rounded.Storage
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.StatTile
import com.tally.app.ui.nav.AppNav
import com.tally.core.Copy

@Composable
fun AboutRoute(nav: AppNav) {
    val viewModel: SettingsViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    AboutScreen(state, onBack = nav::back)
}

/**
 * About Tally: what this phone holds under the one lit panel, the net and the period as tiles, and
 * the promises the app keeps, each as a plain row.
 */
@Composable
fun AboutScreen(state: SettingsState, onBack: () -> Unit = {}) {
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") { PageHead("About Tally", onBack = onBack, context = "Version ${state.version}") }
        item(key = "hero") { LedgerHero(state, Modifier.padding(horizontal = GUTTER)) }
        item(key = "tiles") { AboutTiles(state, Modifier.padding(horizontal = GUTTER)) }
        item(key = "promises") {
            val rows: List<@Composable (Shape) -> Unit> = listOf(
                promiseRow(Icons.Rounded.CloudOff, "No internet permission", "Tally cannot send anything anywhere, by design"),
                promiseRow(Icons.Rounded.NoAccounts, "No account", "Nothing to sign in to, nothing kept about you elsewhere"),
                promiseRow(Icons.Rounded.Storage, "Kept on this phone", "Your entries live in the app's own storage"),
                promiseRow(Icons.Rounded.Shield, "Bank files stay here", "A statement you import is read on the phone and never kept"),
            )
            Group(rows = rows, modifier = Modifier.padding(horizontal = GUTTER).padding(top = 14.dp), title = "Privacy")
        }
    }
}

private fun promiseRow(icon: androidx.compose.ui.graphics.vector.ImageVector, title: String, subtitle: String): @Composable (Shape) -> Unit =
    { shape -> GroupRow(title, shape, subtitle = subtitle, leading = { GlyphBadge(icon) }) }

/** What lives on this phone: the entry count as the figure, and the privacy facts as chips. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LedgerHero(state: SettingsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val count = remember(state.entries, money.locale) { countText(state.entries, money.locale) }
    HeroPanel(modifier) {
        HeroHead("On this phone", end = "OFFLINE")
        Spacer(Modifier.height(12.dp))
        HeroNumber(count, description = Copy.plural(state.entries, "entry", "entries"))
        Text(
            if (state.entries == 0) {
                "No entries yet. What you log is kept here and nowhere else"
            } else {
                (if (state.entries == 1) "entry" else "entries") + " across " + Copy.plural(state.accounts, "account") +
                    ", kept here and nowhere else"
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.CloudOff, "Works offline")
            StatChip(Icons.Rounded.NoAccounts, "No sign-in")
            StatChip(Icons.Rounded.Lock, "Leaves only when you export")
            if (state.settings.sampleLoaded) StatChip(Icons.Rounded.Science, "Sample data")
        }
    }
}

/** Two different readings: where the money sits overall, and which budget period runs now. */
@Composable
private fun AboutTiles(state: SettingsState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                "Net",
                money.formatWhole(state.net),
                m,
                detail = if (state.accounts == 0) "No accounts yet" else "Across " + Copy.plural(state.accounts, "account"),
            )
        },
        second = { m ->
            StatTile(
                "Period",
                Dates.period(state.period, state.today),
                m,
                detail = "Resets " + Dates.short(state.period.endExclusive, state.today),
            )
        },
    )
}
