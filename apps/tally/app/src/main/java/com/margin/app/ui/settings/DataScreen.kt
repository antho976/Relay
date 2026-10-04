package com.margin.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.material.icons.automirrored.rounded.ReceiptLong
import androidx.compose.material.icons.rounded.AccountBalance
import androidx.compose.material.icons.rounded.Backup
import androidx.compose.material.icons.rounded.Category
import androidx.compose.material.icons.rounded.DeleteForever
import androidx.compose.material.icons.rounded.DonutLarge
import androidx.compose.material.icons.rounded.EventRepeat
import androidx.compose.material.icons.rounded.FileDownload
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.Inventory2
import androidx.compose.material.icons.rounded.Restore
import androidx.compose.material.icons.rounded.Science
import androidx.compose.material.icons.rounded.UploadFile
import androidx.compose.material.icons.rounded.Verified
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
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.margin.app.data.repo.DataResult
import com.margin.app.ui.common.Dates
import com.margin.app.ui.common.GUTTER
import com.margin.app.ui.common.Group
import com.margin.app.ui.common.GroupRow
import com.margin.app.ui.common.HeroPanel
import com.margin.app.ui.common.LocalMoney
import com.margin.app.ui.common.PANEL_GAP
import com.margin.app.ui.common.StatChip
import com.margin.app.ui.common.StatTile
import com.margin.app.ui.nav.AppNav
import com.margin.core.Copy

@Composable
fun DataRoute(nav: AppNav) {
    val viewModel: DataViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(viewModel) { viewModel.erased.collect { nav.restart() } }

    // The Storage Access Framework: the owner picks where every file goes, so no storage permission.
    val backupSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri ->
        if (uri != null) viewModel.saveBackup(uri)
    }
    val backupOpener = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) viewModel.readBackup(uri)
    }
    val csvSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/csv")) { uri ->
        if (uri != null) viewModel.exportCsv(uri)
    }
    val csvOpener = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) viewModel.importCsv(uri)
    }

    /** A phone with no document picker throws on launch; that becomes the group's error line. */
    fun pick(group: DataGroup, launch: () -> Unit) {
        runCatching(launch).onFailure { viewModel.noPicker(group) }
    }

    DataScreen(
        state,
        DataActions(
            back = nav::back,
            saveBackup = { pick(DataGroup.BACKUP) { backupSaver.launch(backupFileName(state.today)) } },
            restoreBackup = { pick(DataGroup.BACKUP) { backupOpener.launch(arrayOf("application/json", "*/*")) } },
            confirmRestore = viewModel::confirmRestore,
            cancelRestore = viewModel::cancelRestore,
            exportCsv = { pick(DataGroup.SHEETS) { csvSaver.launch(csvFileName(state.today)) } },
            importCsv = {
                pick(DataGroup.SHEETS) {
                    csvOpener.launch(arrayOf("text/csv", "text/comma-separated-values", "text/plain", "*/*"))
                }
            },
            loadSample = viewModel::loadSample,
            eraseAll = viewModel::eraseAll,
        ),
    )
}

/** Everything the Data page can do, as plain lambdas; the file pickers stay in the route. */
data class DataActions(
    val back: () -> Unit = {},
    val saveBackup: () -> Unit = {},
    val restoreBackup: () -> Unit = {},
    val confirmRestore: () -> Unit = {},
    val cancelRestore: () -> Unit = {},
    val exportCsv: () -> Unit = {},
    val importCsv: () -> Unit = {},
    val loadSample: () -> Unit = {},
    val eraseAll: () -> Unit = {},
)

/**
 * Backup & data: what a backup would hold under the one lit panel, the newest entry and the
 * ledger's kind as tiles, then the groups that move the whole ledger: back up, spreadsheets, the
 * sample, and erase. Each group's last outcome is its footer; only erase and restore ask first.
 */
@Composable
fun DataScreen(state: DataState, actions: DataActions) {
    var confirmErase by rememberSaveable { mutableStateOf(false) }
    val money = LocalMoney.current
    val entries = remember(state.entries, money.locale) { countText(state.entries, money.locale) }
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
    ) {
        item(key = "head") {
            PageHead(
                "Backup & data",
                onBack = actions.back,
                context = (if (state.entries == 1) "1 entry" else "$entries entries") + " on this phone",
            )
        }
        item(key = "hero") { BackupHero(state, entries, Modifier.padding(horizontal = GUTTER)) }
        item(key = "tiles") { DataTiles(state, Modifier.padding(horizontal = GUTTER)) }
        item(key = "backup") { BackupGroup(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 10.dp)) }
        item(key = "sheets") { SheetsGroup(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 10.dp)) }
        item(key = "sample") { SampleGroup(state, actions, Modifier.padding(horizontal = GUTTER).padding(top = 10.dp)) }
        item(key = "erase") {
            EraseGroup(state, onErase = { confirmErase = true }, modifier = Modifier.padding(horizontal = GUTTER).padding(top = 10.dp))
        }
    }

    val pending = state.pendingRestore
    if (pending != null) {
        val savedOn = pending.savedOn?.let { Dates.short(it, state.today) }
        ConfirmDialog(
            text = restorePrompt(pending.entries, savedOn),
            confirm = "Replace",
            destructive = false,
            onConfirm = actions.confirmRestore,
            onDismiss = actions.cancelRestore,
        )
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
}

/** A full backup's contents: the entry count as the figure, every other kind of record as a chip. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BackupHero(state: DataState, entries: String, modifier: Modifier = Modifier) {
    HeroPanel(modifier) {
        HeroHead(Icons.Rounded.Inventory2, "In a backup", end = "JSON")
        Spacer(Modifier.height(12.dp))
        HeroNumber(entries, description = Copy.plural(state.entries, "entry", "entries"))
        Text(
            if (state.isEmpty) {
                "No entries yet. A backup would hold your accounts, categories and plan"
            } else {
                (if (state.entries == 1) "entry" else "entries") + ", with everything they belong to, in one file you keep"
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.AccountBalance, Copy.plural(state.accounts, "account"))
            StatChip(Icons.Rounded.Category, Copy.plural(state.categories, "category", "categories"))
            StatChip(Icons.Rounded.DonutLarge, Copy.plural(state.budgets, "budget"))
            StatChip(Icons.Rounded.EventRepeat, Copy.plural(state.bills, "bill"))
            StatChip(Icons.Rounded.Flag, Copy.plural(state.goals, "goal"))
        }
    }
}

/** The newest entry, and whether the ledger is the owner's own or the labelled sample. */
@Composable
private fun DataTiles(state: DataState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val newest = state.newest
    TilePair(
        modifier,
        first = { m ->
            StatTile(
                Icons.AutoMirrored.Rounded.ReceiptLong,
                "Newest entry",
                if (newest != null) Dates.short(newest.date, state.today) else "None",
                m,
                detail = if (newest != null) {
                    newest.note.ifBlank { newest.categoryName ?: "Transfer" } + " · " + money.format(newest.amount)
                } else {
                    "Nothing logged yet"
                },
            )
        },
        second = { m ->
            StatTile(
                if (state.sampleLoaded) Icons.Rounded.Science else Icons.Rounded.Verified,
                "Ledger",
                when {
                    state.sampleLoaded -> "Sample"
                    state.isEmpty -> "Empty"
                    else -> "Your own"
                },
                m,
                detail = when {
                    state.sampleLoaded -> "Made-up entries, labelled as such"
                    state.isEmpty -> "Log an entry, import a CSV or load the sample"
                    else -> "Every entry typed or imported by you"
                },
            )
        },
    )
}

/** The footer line for [group]: its running act, or its last outcome, or the standing note. */
private fun footerFor(state: DataState, group: DataGroup, standing: String?): Pair<String?, Boolean> {
    val busy = state.busy
    if (busy != null && busy.group == group) return busy.working to false
    return when (val r = state.results[group]) {
        is DataResult.Done -> r.message to false
        is DataResult.Failed -> r.message to true
        null -> standing to false
    }
}

@Composable
private fun BackupGroup(state: DataState, actions: DataActions, modifier: Modifier = Modifier) {
    val idle = state.busy == null
    val save: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Save a backup",
            shape,
            subtitle = "Everything, as one JSON file you keep",
            leading = { RowBadge(Icons.Rounded.Backup) },
            onClick = if (idle) actions.saveBackup else null,
        )
    }
    val restore: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Restore a backup",
            shape,
            subtitle = "Replaces everything on this phone",
            leading = { RowBadge(Icons.Rounded.Restore) },
            onClick = if (idle) actions.restoreBackup else null,
        )
    }
    val (footer, isError) = footerFor(state, DataGroup.BACKUP, null)
    Group(rows = listOf(save, restore), modifier = modifier, title = "Back up", footer = footer, footerIsError = isError)
}

@Composable
private fun SheetsGroup(state: DataState, actions: DataActions, modifier: Modifier = Modifier) {
    val idle = state.busy == null
    val exportRow: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Export entries as CSV",
            shape,
            subtitle = if (state.isEmpty) "Nothing to export yet" else "One row per entry, for any spreadsheet",
            leading = { RowBadge(Icons.Rounded.FileDownload) },
            onClick = if (idle && !state.isEmpty) actions.exportCsv else null,
        )
    }
    val importRow: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Import a CSV",
            shape,
            subtitle = "Adds entries; matches accounts and categories by name",
            leading = { RowBadge(Icons.Rounded.UploadFile) },
            onClick = if (idle) actions.importCsv else null,
        )
    }
    val (footer, isError) = footerFor(state, DataGroup.SHEETS, "Columns: date, type, amount, currency, category, account, to account, note")
    Group(rows = listOf(exportRow, importRow), modifier = modifier, title = "Spreadsheets", footer = footer, footerIsError = isError)
}

@Composable
private fun SampleGroup(state: DataState, actions: DataActions, modifier: Modifier = Modifier) {
    val can = state.busy == null && state.isEmpty
    val sample: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Load sample data",
            shape,
            subtitle = if (state.isEmpty) {
                "Three months of made-up, labelled entries. Only into an empty ledger."
            } else {
                "Loads only into an empty ledger. Erase first to try it."
            },
            leading = {
                RowBadge(
                    Icons.Rounded.Science,
                    tint = if (state.isEmpty) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
            titleColor = if (state.isEmpty) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            onClick = if (can) actions.loadSample else null,
        )
    }
    val (footer, isError) = footerFor(state, DataGroup.SAMPLE, null)
    Group(rows = listOf(sample), modifier = modifier, title = "Try it", footer = footer, footerIsError = isError)
}

@Composable
private fun EraseGroup(state: DataState, onErase: () -> Unit, modifier: Modifier = Modifier) {
    val erase: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "Erase everything",
            shape,
            subtitle = "Keeps the look and the currency; removes every record",
            leading = { RowBadge(Icons.Rounded.DeleteForever, tint = MaterialTheme.colorScheme.error) },
            titleColor = MaterialTheme.colorScheme.error,
            chevron = false,
            onClick = if (state.busy == null) onErase else null,
        )
    }
    val (footer, isError) = footerFor(state, DataGroup.ERASE, null)
    Group(rows = listOf(erase), modifier = modifier, title = "Erase", footer = footer, footerIsError = isError)
}

/** The Material confirm for the two acts that replace or remove everything. Its button names the act. */
@Composable
private fun ConfirmDialog(
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
