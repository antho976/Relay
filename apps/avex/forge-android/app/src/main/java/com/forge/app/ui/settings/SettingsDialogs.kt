package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.DirectionsRun
import androidx.compose.material.icons.rounded.DataObject
import androidx.compose.material.icons.rounded.DeleteForever
import androidx.compose.material.icons.rounded.EmojiEvents
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.FitnessCenter
import androidx.compose.material.icons.rounded.FolderZip
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.PhotoLibrary
import androidx.compose.material.icons.rounded.PictureAsPdf
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.Restore
import androidx.compose.material.icons.rounded.TableChart
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.clickableLabeled

/**
 * Export & back up, as a sheet: the restorable backup first (it is the real safety net), then the
 * one-tap exports, each tagged with the format it writes. Every file goes out through Android's
 * share sheet, so the person picks where it lands.
 */
@Composable
internal fun DataExportDialog(
    viewModel: SettingsViewModel,
    onBackup: () -> Unit,
    onRestore: () -> Unit,
    onExportCrashLogs: () -> Unit,
    onDismiss: () -> Unit
) {
    // Refresh the auto-backup slot and progress-photo stats on open (both may have changed since).
    LaunchedEffect(Unit) {
        viewModel.refreshAutoBackupInfo()
        viewModel.refreshPhotoInfo()
    }
    val autoBackupFailed by viewModel.autoBackupFailed.collectAsState()
    val noBackupWarning by viewModel.noBackupWarning.collectAsState()
    val photoCount by viewModel.photoCount.collectAsState()
    val photoLastTakenMs by viewModel.photoLastTakenMs.collectAsState()
    val dbSize by viewModel.dbSizeLabel.collectAsState()
    val autoBackupCopies by viewModel.autoBackupCopies.collectAsState()
    // Which kept copy the confirm is about (null: none), and whether the copy picker is open. With
    // one copy the picker is skipped: there is nothing to choose.
    var confirmAutoRestore by remember { mutableStateOf<SettingsViewModel.AutoBackupCopyUi?>(null) }
    var chooseAutoCopy by remember { mutableStateOf(false) }

    // What a backup would carry, so "Back up now" says what is at stake.
    val stake = buildList {
        add("Everything, photos included")
        if (dbSize.isNotBlank()) add(dbSize)
        if (photoCount > 0) add(photoCountLabel(photoCount) + (photoLastTakenMs?.let { ", last ${formatShortDate(it)}" } ?: ""))
    }.joinToString(" · ")

    SettingsSheet("Export & back up", "Every file opens in your share sheet, so you choose where it goes.", onDismiss) {
        if (noBackupWarning) {
            SettingsNotice(
                icon = Icons.Rounded.ErrorOutline,
                title = "No backup yet",
                body = "Your training lives only on this phone. Back it up so losing the phone doesn't lose it."
            )
        }

        SettingsGroup(
            "Backup & restore",
            footer = if (autoBackupFailed) "The last auto-backup failed. Free up storage, then back up now."
                     else "Restoring replaces all current data and restarts Avex.",
            footerIsError = autoBackupFailed
        ) {
            SettingsActionRow("Back up now", stake, SettingsIcons.Backup) { onBackup(); onDismiss() }
            SettingsActionRow("Restore from a file", "Pick an Avex backup .zip", Icons.Rounded.Restore) { onRestore(); onDismiss() }
            // Recover from the silent weekly auto-backup without the file picker (#86).
            autoBackupCopies.firstOrNull()?.let { newest ->
                val many = autoBackupCopies.size > 1
                SettingsNavigationRow(
                    "Restore an auto-backup",
                    if (many) "${autoBackupCopies.size} kept · newest ${newest.savedAt}" else "Saved ${newest.savedAt}",
                    Icons.Rounded.History
                ) { if (many) chooseAutoCopy = true else confirmAutoRestore = newest }
            }
        }

        // Each row carries its own format and writes at once; the format is a tag, not an action.
        SettingsGroup("Quick export", footer = "Quick exports leave photos out. Back up now for a copy you can restore.") {
            ExportRow("Training history", "JSON", "Finished workouts, cardio, goals and preferences") { viewModel.exportFullBackup(); onDismiss() }
            ExportRow("This week", "JSON", "A summary made for AI analysis") { viewModel.exportWeeklyJson(); onDismiss() }
            ExportRow("All sessions", "CSV", "A spreadsheet of every session") { viewModel.exportSessionsCsv(); onDismiss() }
            ExportRow("All PRs", "CSV", "Your best lift per exercise") { viewModel.exportPrsCsv(); onDismiss() }
            ExportRow("Bodyweight", "CSV", "Every weigh-in") { viewModel.exportBodyweightCsv(); onDismiss() }
            ExportRow("Cardio", "CSV", "Every cardio session") { viewModel.exportCardioCsv(); onDismiss() }
            ExportRow("Last session", "PDF", "A printable session sheet") { viewModel.exportLastSessionPdf(); onDismiss() }
            ExportRow("Crash logs", "ZIP", "Diagnostics, if something broke") { onExportCrashLogs(); onDismiss() }
        }
    }

    if (chooseAutoCopy) {
        // Each kept copy is one row; the newest is named as such, so picking an older one (when the
        // newest is damaged or already wrong) is deliberate.
        com.forge.app.ui.common.window.AlertDialog(
            onDismissRequest = { chooseAutoCopy = false },
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
            icon = { Icon(Icons.Rounded.History, contentDescription = null) },
            title = { Text("Pick an auto-backup") },
            text = {
                Column {
                    Text(
                        "Older copies are kept in case the newest is damaged.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(bottom = 8.dp)
                    )
                    autoBackupCopies.forEachIndexed { i, copy ->
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .clickableLabeled("Restore the copy saved ${copy.savedAt}") {
                                    chooseAutoCopy = false
                                    confirmAutoRestore = copy
                                }
                                .padding(vertical = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp)
                        ) {
                            Text(copy.savedAt, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface, modifier = Modifier.weight(1f))
                            if (i == 0) SettingsStatus("Newest", live = true)
                        }
                    }
                }
            },
            confirmButton = {},
            dismissButton = {
                androidx.compose.material3.TextButton(onClick = { chooseAutoCopy = false }) {
                    Text("Cancel", color = MaterialTheme.colorScheme.onSurface)
                }
            }
        )
    }
    confirmAutoRestore?.let { copy ->
        SettingsConfirmDialog(
            title = "Restore this auto-backup?",
            body = "Replaces all current data with the copy saved ${copy.savedAt}, then restarts Avex.",
            confirmLabel = "Restore and restart",
            icon = Icons.Rounded.Restore,
            onConfirm = {
                confirmAutoRestore = null
                viewModel.restoreAutoBackup(copy.generation)
                onDismiss()
            },
            onDismiss = { confirmAutoRestore = null }
        )
    }
}

/** One quick export: its format's glyph, what it holds, and the format as a tag. The row writes it. */
@Composable
private fun ExportRow(label: String, format: String, hint: String, onClick: () -> Unit) {
    val glyph = when (format) {
        "JSON" -> Icons.Rounded.DataObject
        "CSV" -> Icons.Rounded.TableChart
        "PDF" -> Icons.Rounded.PictureAsPdf
        else -> Icons.Rounded.FolderZip
    }
    SettingsAdaptiveRow(
        title = label,
        supporting = hint,
        interaction = Modifier.clickableLabeled("Export $label as $format", onClick = onClick),
        leading = { SettingsIconTile(glyph) }
    ) { SettingsPill(format, PillTone.Quiet) }
}

/**
 * The reset confirmation. A factory reset is irreversible and wipes everything, so it is gated
 * behind typing a word; the targeted resets confirm with one tap.
 */
@Composable
internal fun ResetConfirmDialog(
    target: ResetTarget,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    /** Extra line under the message, used to warn about progress photos on factory reset. */
    photoWarning: String? = null
) {
    val factory = target == ResetTarget.FACTORY
    val confirmWord = "ERASE"
    var typed by remember { mutableStateOf("") }
    val canConfirm = !factory || typed.trim().equals(confirmWord, ignoreCase = true)
    SettingsConfirmDialog(
        title = "${target.label}?",
        body = target.message,
        confirmLabel = when (target) {
            ResetTarget.FACTORY -> "Erase everything"
            ResetTarget.SETTINGS -> "Reset settings"
            else -> "Delete"
        },
        icon = if (factory) Icons.Rounded.DeleteForever else Icons.Rounded.RestartAlt,
        destructive = true,
        confirmEnabled = canConfirm,
        onConfirm = onConfirm,
        onDismiss = onDismiss,
        extra = if (photoWarning == null && !factory) null else {
            {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    if (photoWarning != null) {
                        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                            Icon(Icons.Rounded.PhotoLibrary, contentDescription = null, tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(20.dp))
                            Text(photoWarning, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurface)
                        }
                    }
                    if (factory) {
                        OutlinedTextField(
                            value = typed,
                            onValueChange = { typed = it },
                            singleLine = true,
                            label = { Text("Type $confirmWord to confirm") },
                            colors = settingsFieldColors(),
                            shape = androidx.compose.foundation.shape.RoundedCornerShape(12.dp),
                            modifier = Modifier.fillMaxWidth()
                        )
                    }
                }
            }
        }
    )
}

/**
 * The targeted resets as a sheet, everything except [ResetTarget.FACTORY], which keeps its own
 * row on the root list. Picking one still goes through [ResetConfirmDialog].
 */
@Composable
internal fun ResetMenuDialog(
    onPick: (ResetTarget) -> Unit,
    onDismiss: () -> Unit
) {
    SettingsSheet("Reset", "Pick what to clear. Each one asks before it deletes anything.", onDismiss) {
        SettingsGroup {
            ResetTarget.entries.filter { it != ResetTarget.FACTORY }.forEach { target ->
                SettingsActionRow(target.label, target.message, resetGlyph(target)) { onPick(target) }
            }
        }
    }
}

private fun resetGlyph(target: ResetTarget): ImageVector = when (target) {
    ResetTarget.SESSIONS -> Icons.Rounded.FitnessCenter
    ResetTarget.TROPHIES -> Icons.Rounded.EmojiEvents
    ResetTarget.CARDIO -> Icons.AutoMirrored.Rounded.DirectionsRun
    ResetTarget.SETTINGS -> Icons.Rounded.Tune
    ResetTarget.FACTORY -> Icons.Rounded.DeleteForever
}
