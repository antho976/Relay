package com.forge.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.History
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.clickableLabeled

/**
 * Backup settings (GYMAP-67): the weekly auto-backup switch and its last run, a user-picked folder
 * that keeps a copy through an uninstall, the backup password, and Back up now. A failed run or
 * unprotected data is the only thing that turns red.
 */
@Composable
internal fun BackupPage(vm: SettingsViewModel, onBack: () -> Unit) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val state by vm.state.collectAsStateWithLifecycle()
    val enabled by vm.autoBackupEnabled.collectAsStateWithLifecycle()
    val folderUri by vm.backupFolderUri.collectAsStateWithLifecycle()
    val savedAt by vm.autoBackupSavedAt.collectAsStateWithLifecycle()
    val failed by vm.autoBackupFailed.collectAsStateWithLifecycle()
    val noBackup by vm.noBackupWarning.collectAsStateWithLifecycle()

    // Fresh status whenever the page shows — the worker may have run (or failed) since last time.
    LaunchedEffect(Unit) { vm.refreshAutoBackupInfo() }

    val folderPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        uri?.let {
            if (state.galleryLockEnabled) authenticateSettingsAction(context, vm, "Unlock photos for this backup folder") { vm.setBackupFolder(it) }
            else vm.setBackupFolder(it)
        }
    }

    SettingsScaffold("Backup", onBack) {
        SettingsGroup("Auto-backup") {
            SettingsSwitchRow(
                "Weekly auto-backup",
                "A restorable copy of your data, kept automatically",
                enabled,
                onCheckedChange = vm::setAutoBackupEnabled
            )
            BackupStatusRow(savedAt = savedAt, failed = failed, noBackup = noBackup)
        }

        SettingsGroup("Backup folder", footer = "A folder keeps your backup even if you uninstall Avex. Internal copies don't survive it.") {
            // The whole row picks the folder; the pill is drawn, not separately tappable.
            SettingsAdaptiveRow(
                title = folderUri?.let(::folderLabel) ?: "No folder chosen",
                supporting = if (folderUri != null) "Each backup also lands here" else "Backups stay on this phone only",
                interaction = Modifier.clickableLabeled("Choose backup folder") { folderPicker.launch(null) },
                leading = { SettingsIconTile(Icons.Rounded.Folder) }
            ) { SettingsCompactButton(if (folderUri == null) "Choose" else "Change") }
        }

        BackupPasswordSection(vm, galleryLocked = state.galleryLockEnabled)

        // "Back up now" works with auto-backup off; dropping the folder is its sidekick.
        SettingsButtonBar {
            SettingsButton("Back up now") {
                if (state.galleryLockEnabled) authenticateSettingsAction(context, vm, "Unlock photos for this backup") { vm.backupNow() }
                else vm.backupNow()
            }
            if (folderUri != null) {
                SettingsButton("Remove folder", kind = ButtonKind.Tonal, onClick = vm::clearBackupFolder)
            }
        }
    }
}

/** The last run: when it was, or that it failed. Red only on a failed run or unprotected data. */
@Composable
private fun BackupStatusRow(savedAt: String?, failed: Boolean, noBackup: Boolean) {
    val flag = failed || noBackup
    SettingsRowContainer {
        SettingsIconTile(if (flag) Icons.Rounded.ErrorOutline else Icons.Rounded.History, if (flag) TileTone.Danger else TileTone.Neutral)
        SettingsRowText(
            when {
                failed -> "Last backup failed"
                savedAt != null -> "Last backup"
                else -> "No backup yet"
            },
            when {
                failed -> "Back up now to try again"
                savedAt != null -> savedAt
                else -> "Your data isn't protected by a copy yet"
            },
            supportingColor = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

/** A short, human folder name from a SAF tree URI ("…/tree/primary:Download/Backups" → "Backups"). */
private fun folderLabel(uri: String): String =
    android.net.Uri.decode(uri).substringAfterLast(':').substringAfterLast('/').ifBlank { "Selected folder" }
