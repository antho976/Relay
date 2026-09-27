package com.forge.app.ui.settings

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.InsertDriveFile
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.FolderOff
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Restore
import androidx.compose.material.icons.rounded.Share
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import com.forge.app.data.importer.FoundImport
import com.forge.app.data.importer.foundImportSummary
import com.forge.app.ui.common.clickableLabeled

/**
 * Import from another app (#GYMAP-17), as a sheet. It leads with the files it FOUND: once Avex can
 * see one folder (Download/Avex), recognised exports are listed for a one-tap import, and Avex
 * backups found there are offered as restores. Picking a file by hand and sharing in from the other
 * app are the fallbacks below.
 */
@Composable
internal fun ImportDialog(
    viewModel: SettingsViewModel,
    onGrantFolder: () -> Unit,
    onManualPick: () -> Unit,
    onImportFound: (android.net.Uri) -> Unit,
    onRestoreFound: (android.net.Uri) -> Unit,
    onDismiss: () -> Unit
) {
    val granted by viewModel.importFolderGranted.collectAsState()
    val scanning by viewModel.scanningImports.collectAsState()
    val found by viewModel.foundImports.collectAsState()
    val backups by viewModel.foundBackups.collectAsState()

    // Re-scan whenever the sheet opens with access already granted (new exports may have landed).
    LaunchedEffect(granted) { if (granted) viewModel.scanImportFolder() }

    SettingsSheet(
        "Import",
        "Bring in history from Strong, Hevy, FitNotes or any CSV. It joins your log; nothing is replaced.",
        onDismiss
    ) {
        if (!granted) {
            SettingsGroup(footer = "Opens Download/Avex. Tap Use this folder, then save your exports there.") {
                SettingsActionRow("Find my exports", "Let Avex look in one folder for files to import", Icons.Rounded.FolderOpen) {
                    onGrantFolder()
                }
            }
        } else {
            SettingsGroup(
                "In your folder",
                headerTrailing = if (!scanning && (found.size + backups.size) > 0) "${found.size + backups.size} found" else null
            ) {
                when {
                    scanning -> SettingsInfoRow("Looking for exports", "Reading the folder", Icons.Rounded.Folder)
                    found.isEmpty() && backups.isEmpty() -> SettingsEmptyBlock(
                        Icons.Rounded.FolderOff,
                        "Nothing found yet",
                        "Save exports into that folder, or a folder inside it."
                    )
                    else -> {
                        found.forEach { file -> FoundFileRow(file) { onImportFound(file.uri); onDismiss() } }
                        // Backups replace everything rather than adding to it, so they say Restore and
                        // go through the usual confirm, never a one-tap swap.
                        backups.forEach { backup -> FoundBackupRow(backup) { onRestoreFound(backup.uri); onDismiss() } }
                    }
                }
                SettingsNavigationRow("Choose a different folder", icon = Icons.Rounded.Folder, onClick = onGrantFolder)
            }
        }

        SettingsGroup("Other ways") {
            SettingsActionRow("Choose a file", "Pick a CSV or JSON export yourself", Icons.AutoMirrored.Rounded.InsertDriveFile) {
                onManualPick(); onDismiss()
            }
            SettingsInfoRow("Share from the other app", "In Strong, Hevy or FitNotes: Export, then Share to Avex", Icons.Rounded.Share)
        }
    }
}

/** One recognised export: its name, what it holds and when, and an Import button the row answers to. */
@Composable
private fun FoundFileRow(file: FoundImport, onClick: () -> Unit) {
    // What the file HOLDS, not just a workout count (L-01): a bodyweight CSV carries no workouts,
    // and "0 workouts" is why it used to read as empty. The name is user content, so it wraps.
    SettingsRowContainer(interaction = Modifier.clickableLabeled("Import ${file.name}", onClick = onClick)) {
        SettingsIconTile(Icons.AutoMirrored.Rounded.InsertDriveFile)
        SettingsRowText(file.name, "${foundImportSummary(file)} · ${formatShortDate(file.lastModified)}")
        SettingsCompactButton("Import")
    }
}

/** One Avex backup found in the folder: its name and date, and a Restore button. */
@Composable
private fun FoundBackupRow(backup: com.forge.app.data.importer.FoundBackup, onClick: () -> Unit) {
    // A file name has no spaces to wrap at, so the button stays beside it and the name breaks where
    // it must; the adaptive stacking would split this list into two layouts.
    SettingsRowContainer(interaction = Modifier.clickableLabeled("Restore ${backup.name}", onClick = onClick)) {
        SettingsIconTile(Icons.Rounded.Restore)
        SettingsRowText(
            backup.name,
            buildString {
                append("Avex backup")
                if (backup.passwordProtected) append(" · password")
                append(" · ${formatShortDate(backup.lastModified)}")
            }
        )
        SettingsCompactButton("Restore")
    }
}
