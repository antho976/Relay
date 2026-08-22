package com.forge.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.clickableLabeled

/**
 * Backup settings (GYMAP-67): the weekly auto-backup toggle, its last-run status with a backup-health
 * dot, a user-picked folder that keeps a copy through an uninstall, and a one-tap "Back up now". A
 * settings page — mono anchors + air, no dividers, the do-it-now action grouped at the END (§3/§8).
 * The health dot follows §8: a red exception dot ONLY when a run failed or data sits unprotected.
 */
@Composable
internal fun BackupPage(vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val enabled by vm.autoBackupEnabled.collectAsStateWithLifecycle()
    val folderUri by vm.backupFolderUri.collectAsStateWithLifecycle()
    val savedAt by vm.autoBackupSavedAt.collectAsStateWithLifecycle()
    val failed by vm.autoBackupFailed.collectAsStateWithLifecycle()
    val noBackup by vm.noBackupWarning.collectAsStateWithLifecycle()

    // Fresh status whenever the page shows — the worker may have run (or failed) since last time.
    LaunchedEffect(Unit) { vm.refreshAutoBackupInfo() }

    val folderPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        uri?.let { vm.setBackupFolder(it) }
    }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp)
    ) {
        SettingsSectionHeader("Auto-backup", top = 12.dp)
        ToggleRow(
            label = "Weekly auto-backup",
            subtitle = "A restorable copy of your data, kept automatically.",
            checked = enabled,
            onCheckedChange = vm::setAutoBackupEnabled
        )
        BackupStatusRow(savedAt = savedAt, failed = failed, noBackup = noBackup)

        SettingsSectionHeader("Backup folder")
        Text(
            "A folder keeps your backup even if you uninstall Avex. Internal copies don't survive it.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            modifier = Modifier.padding(start = SETTINGS_GUTTER, end = SETTINGS_GUTTER, top = 2.dp, bottom = 6.dp)
        )
        // Whole row taps to (re)pick the folder; the outlined pill is drawn, not separately clickable (§8).
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickableLabeled("Choose backup folder") { folderPicker.launch(null) }
                .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text("Backup folder", style = MaterialTheme.typography.bodyMedium, color = onBg)
                SettingsExplainer(folderUri?.let(::folderLabel) ?: "None, backups stay on this phone")
            }
            ConnectPill(if (folderUri == null) "Choose" else "Change")
        }
        // Page actions, grouped at the END (§8) — "Back up now" is always available (a manual backup
        // works with auto-backup off), and dropping the folder joins them as the outlined sidekick
        // rather than sitting mid-scroll as a bare text link, which is where it used to live.
        Spacer(Modifier.height(24.dp))
        SettingsActionRow {
            SettingsPrimaryAction(label = "Back up now", onClick = vm::backupNow)
            if (folderUri != null) {
                SettingsOutlineAction("Remove folder", onClick = vm::clearBackupFolder)
            }
        }
    }
}

/** Last-run status + a §8 backup-health dot: red ONLY on a failed run or unprotected data. */
@Composable
private fun BackupStatusRow(savedAt: String?, failed: Boolean, noBackup: Boolean) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val error = MaterialTheme.colorScheme.error
    val flag = failed || noBackup
    val text = when {
        failed -> "Last backup failed"
        savedAt != null -> "Last backup · $savedAt"
        else -> "No backup yet"
    }
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (flag) Box(Modifier.size(7.dp).background(error, CircleShape))
        Text(
            text,
            style = MaterialTheme.typography.bodyMedium,
            // A hard failure reads in error; the "no backup yet" nudge stays muted (a prompt, not a fault).
            color = if (failed) error else muted
        )
    }
}

/** A short, human folder name from a SAF tree URI ("…/tree/primary:Download/Backups" → "Backups"). */
private fun folderLabel(uri: String): String =
    android.net.Uri.decode(uri).substringAfterLast(':').substringAfterLast('/').ifBlank { "Selected folder" }
