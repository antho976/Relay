package com.tally.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AccountBalance
import androidx.compose.material.icons.rounded.Category
import androidx.compose.material.icons.rounded.Contrast
import androidx.compose.material.icons.rounded.CurrencyExchange
import androidx.compose.material.icons.rounded.DeleteForever
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.FileDownload
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.MoveToInbox
import androidx.compose.material.icons.rounded.Science
import androidx.compose.material.icons.rounded.SearchOff
import androidx.compose.material.icons.rounded.SettingsBackupRestore
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.repo.DataResult
import com.tally.app.ui.activity.SearchField
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.nav.AppNav
import com.tally.core.Copy
import java.time.Instant
import java.time.ZoneId

/** Space between two groups on the Settings list, Avex's kit spacing. */
private val GROUP_SPACING = 28.dp

@Composable
fun SettingsRoute(nav: AppNav) {
    val viewModel: SettingsViewModel = hiltViewModel()
    val dataModel: DataViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val data by dataModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(dataModel) { dataModel.erased.collect { nav.restart() } }
    val actions = remember(nav, dataModel) {
        SettingsActions(
            back = nav::back,
            open = { dest ->
                when (dest) {
                    SettingsDest.APPEARANCE -> nav.appearance()
                    SettingsDest.FORMAT -> nav.format()
                    SettingsDest.ACCOUNTS -> nav.accounts()
                    SettingsDest.CATEGORIES -> nav.categories()
                    SettingsDest.IMPORT -> nav.import()
                    SettingsDest.BACKUP -> nav.backup()
                    SettingsDest.EXPORT -> nav.export()
                    SettingsDest.ABOUT -> nav.about()
                    // The two acts that ask first are the screen's; it never routes them here.
                    SettingsDest.SAMPLE, SettingsDest.ERASE -> Unit
                }
            },
            loadSample = dataModel::loadSample,
            eraseAll = dataModel::eraseAll,
        )
    }
    SettingsScreen(state, actions, data = data)
}

/** Everything Settings can do, as plain lambdas. */
data class SettingsActions(
    val back: () -> Unit = {},
    val open: (SettingsDest) -> Unit = {},
    val loadSample: () -> Unit = {},
    val eraseAll: () -> Unit = {},
)

/**
 * Settings, Avex's root: a search pill over the grouped rows, every row carrying its live value so
 * the page answers "how is this set up" before a tap. General, Money, Data, Reset and About; while a
 * query is typed the groups give way to one ranked list of hits, each going straight to its page.
 * Rows lead with a neutral tile, as Avex's do; only what is wrong (a failed backup) or what cannot
 * be undone (erase) takes the error colour.
 */
@Composable
fun SettingsScreen(state: SettingsState, actions: SettingsActions, data: DataState? = null, query: String? = null) {
    var typed by rememberSaveable { mutableStateOf("") }
    val q = query ?: typed
    var confirmErase by rememberSaveable { mutableStateOf(false) }
    var confirmSample by rememberSaveable { mutableStateOf(false) }
    val hits = remember(q) { searchSettings(q) }
    val choose: (SettingsDest) -> Unit = { dest ->
        when (dest) {
            SettingsDest.SAMPLE -> if (state.canLoadSample) confirmSample = true
            SettingsDest.ERASE -> confirmErase = true
            else -> actions.open(dest)
        }
    }
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(if (q.isBlank()) GROUP_SPACING else 14.dp),
    ) {
        item(key = "head") {
            PageHead("Settings", onBack = actions.back, context = "Tally ${state.version} · on this phone only")
        }
        item(key = "search") {
            SearchField(q, { typed = it }, Modifier.padding(horizontal = GUTTER), placeholder = "Search settings")
        }
        if (q.isBlank()) {
            item(key = "general") { GeneralGroup(state, choose) }
            item(key = "money") { MoneyGroup(state, choose) }
            item(key = "data") { DataGroupRows(state, choose) }
            item(key = "reset") { ResetGroup(state, data, choose) }
            item(key = "about") { AboutGroup(state, choose) }
        } else if (hits.isEmpty()) {
            item(key = "none") {
                val none: @Composable (Shape) -> Unit = { shape ->
                    GroupRow(
                        "No settings match “${q.trim()}”",
                        shape,
                        subtitle = "Try a broader word, like backup, currency or bank",
                        leading = { GlyphBadge(Icons.Rounded.SearchOff) },
                    )
                }
                Group(rows = listOf(none), modifier = Modifier.padding(horizontal = GUTTER))
            }
        } else {
            item(key = "hits") {
                Group(
                    rows = hits.map { hitRow(it, choose) },
                    modifier = Modifier.padding(horizontal = GUTTER),
                    title = Copy.plural(hits.size, "result"),
                )
            }
        }
    }
    if (confirmErase) {
        ConfirmDialog(
            text = "Erase every entry, account, category, budget, bill and goal? This cannot be undone.",
            confirm = "Erase everything",
            destructive = true,
            onConfirm = {
                confirmErase = false
                actions.eraseAll()
            },
            onDismiss = { confirmErase = false },
        )
    }
    if (confirmSample) {
        ConfirmDialog(
            text = "Load three months of made-up entries, labelled as sample data? Erase everything removes them again.",
            confirm = "Load sample",
            destructive = false,
            onConfirm = {
                confirmSample = false
                actions.loadSample()
            },
            onDismiss = { confirmSample = false },
        )
    }
}

/** A Settings row: a neutral glyph tile, the name, the live value, a chevron. */
@Composable
private fun SettingsRow(
    dest: SettingsDest,
    shape: Shape,
    subtitle: String,
    onOpen: (SettingsDest) -> Unit,
    icon: ImageVector = destIcon(dest),
    tint: Color? = null,
    enabled: Boolean = true,
) {
    GroupRow(
        pageName(dest),
        shape,
        subtitle = subtitle,
        leading = {
            if (tint != null) GlyphBadge(icon, tint = tint, fill = tint.copy(alpha = 0.14f)) else GlyphBadge(icon)
        },
        titleColor = when {
            dest == SettingsDest.ERASE -> MaterialTheme.colorScheme.error
            !enabled -> MaterialTheme.colorScheme.onSurfaceVariant
            else -> MaterialTheme.colorScheme.onBackground
        },
        chevron = dest != SettingsDest.ERASE && dest != SettingsDest.SAMPLE,
        onClick = if (enabled) ({ onOpen(dest) }) else null,
    )
}

internal fun destIcon(dest: SettingsDest): ImageVector = when (dest) {
    SettingsDest.APPEARANCE -> Icons.Rounded.Contrast
    SettingsDest.FORMAT -> Icons.Rounded.CurrencyExchange
    SettingsDest.ACCOUNTS -> Icons.Rounded.AccountBalance
    SettingsDest.CATEGORIES -> Icons.Rounded.Category
    SettingsDest.IMPORT -> Icons.Rounded.MoveToInbox
    SettingsDest.BACKUP -> Icons.Rounded.SettingsBackupRestore
    SettingsDest.EXPORT -> Icons.Rounded.FileDownload
    SettingsDest.SAMPLE -> Icons.Rounded.Science
    SettingsDest.ERASE -> Icons.Rounded.DeleteForever
    SettingsDest.ABOUT -> Icons.Rounded.Info
}

private fun hitRow(entry: SettingsEntry, onOpen: (SettingsDest) -> Unit): @Composable (Shape) -> Unit = { shape ->
    GroupRow(
        entry.name,
        shape,
        subtitle = "In " + entry.where,
        leading = { GlyphBadge(destIcon(entry.dest)) },
        onClick = { onOpen(entry.dest) },
    )
}

private val GROUP_MODIFIER = Modifier.padding(horizontal = GUTTER)

@Composable
private fun GeneralGroup(state: SettingsState, onOpen: (SettingsDest) -> Unit) {
    val s = state.settings
    val appearance: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(SettingsDest.APPEARANCE, shape, appearanceSummary(s.amoled, s.accentEnabled, s.accent), onOpen)
    }
    val format: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(SettingsDest.FORMAT, shape, formatSummary(s.currency, s.monthStartDay, s.weekStartsMonday), onOpen)
    }
    Group(rows = listOf(appearance, format), modifier = GROUP_MODIFIER, title = "General")
}

@Composable
private fun MoneyGroup(state: SettingsState, onOpen: (SettingsDest) -> Unit) {
    val money = LocalMoney.current
    val accounts: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(SettingsDest.ACCOUNTS, shape, accountsSummary(state.accounts, money.formatWhole(state.net)), onOpen)
    }
    val categories: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(SettingsDest.CATEGORIES, shape, categoriesSummary(state.spendingCategories, state.incomeCategories), onOpen)
    }
    val bank: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(SettingsDest.IMPORT, shape, "Desjardins · Wealthsimple · any bank's CSV", onOpen)
    }
    val plan = listOfNotNull(
        Copy.plural(state.budgets, "budget").takeIf { state.budgets > 0 },
        Copy.plural(state.bills, "bill").takeIf { state.bills > 0 },
        Copy.plural(state.goals, "goal").takeIf { state.goals > 0 },
    ).joinToString(" · ")
    Group(
        rows = listOf(accounts, categories, bank),
        modifier = GROUP_MODIFIER,
        title = "Money",
        footer = if (plan.isEmpty()) "No budgets, bills or goals yet" else "$plan in your plan",
    )
}

@Composable
private fun DataGroupRows(state: SettingsState, onOpen: (SettingsDest) -> Unit) {
    val b = state.backup
    val lastOn = b.lastAt.takeIf { it > 0L }?.let {
        Dates.short(Instant.ofEpochMilli(it).atZone(ZoneId.systemDefault()).toLocalDate(), state.today)
    }
    val backup: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(
            SettingsDest.BACKUP,
            shape,
            backupSummary(b.auto, lastOn, b.lastFailed),
            onOpen,
            icon = if (b.lastFailed) Icons.Rounded.ErrorOutline else destIcon(SettingsDest.BACKUP),
            tint = if (b.lastFailed) MaterialTheme.colorScheme.error else null,
        )
    }
    val export: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(
            SettingsDest.EXPORT,
            shape,
            if (state.entries == 0) "Nothing to export yet" else "Entries as CSV · a backup file",
            onOpen,
        )
    }
    Group(
        rows = listOf(backup, export),
        modifier = GROUP_MODIFIER,
        title = "Data",
        footer = if (b.folderUri == null && state.entries > 0) "Backups stay inside the app until you choose a folder" else null,
    )
}

@Composable
private fun ResetGroup(state: SettingsState, data: DataState?, onOpen: (SettingsDest) -> Unit) {
    val idle = data?.busy == null
    val sample: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(
            SettingsDest.SAMPLE,
            shape,
            if (state.canLoadSample) "Three months of made-up, labelled entries" else "Loads only into an empty app",
            onOpen,
            enabled = idle && state.canLoadSample,
        )
    }
    val erase: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(
            SettingsDest.ERASE,
            shape,
            "Keeps the look and the currency; removes every record",
            onOpen,
            tint = MaterialTheme.colorScheme.error,
            enabled = idle,
        )
    }
    val busy = data?.busy?.takeIf { it.group == DataGroup.RESET }
    val result = data?.results?.get(DataGroup.RESET)
    Group(
        rows = listOf(sample, erase),
        modifier = GROUP_MODIFIER,
        title = "Reset",
        footer = busy?.working ?: when (result) {
            is DataResult.Done -> result.message
            is DataResult.Failed -> result.message
            null -> if (state.settings.sampleLoaded) "Sample data is loaded. Erase everything removes it" else null
        },
        footerIsError = busy == null && result is DataResult.Failed,
    )
}

@Composable
private fun AboutGroup(state: SettingsState, onOpen: (SettingsDest) -> Unit) {
    val about: @Composable (Shape) -> Unit = { shape ->
        SettingsRow(SettingsDest.ABOUT, shape, "Version ${state.version} · offline, no account, no internet permission", onOpen)
    }
    Group(rows = listOf(about), modifier = GROUP_MODIFIER, title = "About")
}

/** The Material confirm for the acts that replace or remove everything. Its button names the act. */
@Composable
internal fun ConfirmDialog(
    text: String,
    confirm: String,
    destructive: Boolean,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(
                    confirm,
                    color = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel", color = MaterialTheme.colorScheme.onBackground)
            }
        },
        text = { Text(text, style = MaterialTheme.typography.bodyLarge) },
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        textContentColor = MaterialTheme.colorScheme.onBackground,
    )
}
