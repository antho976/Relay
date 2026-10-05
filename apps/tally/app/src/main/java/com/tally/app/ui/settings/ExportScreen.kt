package com.tally.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Backup
import androidx.compose.material.icons.rounded.CalendarMonth
import androidx.compose.material.icons.rounded.TableChart
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.repo.DataResult
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.RowPill
import com.tally.app.ui.nav.AppNav
import com.tally.core.Copy

@Composable
fun ExportRoute(nav: AppNav) {
    val viewModel: DataViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    // The Storage Access Framework: the owner picks where every file goes, so no storage permission.
    val allSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/csv")) { uri ->
        if (uri != null) viewModel.exportCsv(uri)
    }
    val monthSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/csv")) { uri ->
        if (uri != null) viewModel.exportCsv(uri, state.period)
    }
    val fileSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri ->
        if (uri != null) viewModel.exportFile(uri)
    }
    fun pick(launch: () -> Unit) {
        runCatching(launch).onFailure { viewModel.noPicker(DataGroup.EXPORT) }
    }
    ExportScreen(
        state,
        ExportActions(
            back = nav::back,
            allCsv = { pick { allSaver.launch(csvFileName(state.today)) } },
            monthCsv = { pick { monthSaver.launch(monthCsvFileName(state.period.start)) } },
            backupFile = { pick { fileSaver.launch(backupFileName(state.today)) } },
        ),
    )
}

/** Everything the Export page can do, as plain lambdas. */
data class ExportActions(
    val back: () -> Unit = {},
    val allCsv: () -> Unit = {},
    val monthCsv: () -> Unit = {},
    val backupFile: () -> Unit = {},
)

/**
 * Export, Avex's quick exports: each row writes one file at once, tagged with the format it
 * writes, into a place the owner picks. The backup file is the one that restores.
 */
@Composable
fun ExportScreen(state: DataState, actions: ExportActions) {
    val idle = state.busy == null
    val empty = state.isEmpty
    val month = Dates.period(state.period, state.today)
    LazyColumn(
        Modifier.fillMaxSize().navigationBarsPadding(),
        contentPadding = PaddingValues(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        item(key = "head") {
            PageHead(
                "Export",
                onBack = actions.back,
                context = (if (state.entries == 1) "1 entry" else "${state.entries} entries") + " · files you choose where to keep",
            )
        }
        item(key = "sheets") {
            val all: @Composable (Shape) -> Unit = { shape ->
                GroupRow(
                    "Every entry",
                    shape,
                    subtitle = if (empty) "Nothing to export yet" else "One row per entry, for any spreadsheet",
                    leading = { GlyphBadge(Icons.Rounded.TableChart) },
                    trailing = { RowPill("CSV") },
                    chevron = false,
                    onClick = if (idle && !empty) actions.allCsv else null,
                )
            }
            val thisMonth: @Composable (Shape) -> Unit = { shape ->
                GroupRow(
                    month,
                    shape,
                    subtitle = if (empty) "Nothing to export yet" else "This month's entries alone",
                    leading = { GlyphBadge(Icons.Rounded.CalendarMonth) },
                    trailing = { RowPill("CSV") },
                    chevron = false,
                    onClick = if (idle && !empty) actions.monthCsv else null,
                )
            }
            Group(
                rows = listOf(all, thisMonth),
                modifier = Modifier.padding(horizontal = GUTTER),
                title = "Spreadsheet",
                footer = "Columns: date, type, amount, currency, category, account, to account, note",
            )
        }
        item(key = "file") {
            val file: @Composable (Shape) -> Unit = { shape ->
                GroupRow(
                    "Backup file",
                    shape,
                    subtitle = "Everything, restorable in Tally on any phone",
                    leading = { GlyphBadge(Icons.Rounded.Backup) },
                    trailing = { RowPill("JSON") },
                    chevron = false,
                    onClick = if (idle) actions.backupFile else null,
                )
            }
            val busy = state.busy?.takeIf { it.group == DataGroup.EXPORT }
            val result = state.results[DataGroup.EXPORT]
            Group(
                rows = listOf(file),
                modifier = Modifier.padding(horizontal = GUTTER),
                title = "Backup",
                footer = busy?.working ?: when (result) {
                    is DataResult.Done -> result.message
                    is DataResult.Failed -> result.message
                    null -> if (state.sampleLoaded) "The sample is labelled in every file it goes into" else null
                },
                footerIsError = busy == null && result is DataResult.Failed,
            )
        }
    }
}

/** "1,284 entries", for a page's context. */
internal fun entriesLine(n: Int): String = Copy.plural(n, "entry", "entries")
