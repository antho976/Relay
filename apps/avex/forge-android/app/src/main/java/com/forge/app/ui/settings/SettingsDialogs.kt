@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.forge.app.ui.common.clickableLabeled

@Composable
internal fun DataExportDialog(
    viewModel: SettingsViewModel,
    onBackup: () -> Unit,
    onRestore: () -> Unit,
    onExportCrashLogs: () -> Unit,
    onDismiss: () -> Unit
) {
    Dialog(onDismissRequest = onDismiss) {
        val onBg = MaterialTheme.colorScheme.onBackground
        val muted = MaterialTheme.colorScheme.onSurfaceVariant
        val outline = MaterialTheme.colorScheme.outline
        val bg = MaterialTheme.colorScheme.background

        Column(
            modifier = Modifier.fillMaxWidth().background(bg, RoundedCornerShape(8.dp)).padding(horizontal = 24.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text("DATA", style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.5.sp)

            // ── Backup & restore — the real safety net ───────────────────────────
            // Refresh the auto-backup slot and progress-photo stats on open (both may have changed since).
            androidx.compose.runtime.LaunchedEffect(Unit) {
                viewModel.refreshAutoBackupInfo()
                viewModel.refreshPhotoInfo()
            }
            val autoBackupSavedAt by viewModel.autoBackupSavedAt.collectAsState()
            val autoBackupFailed by viewModel.autoBackupFailed.collectAsState()
            val noBackupWarning by viewModel.noBackupWarning.collectAsState()
            val photoCount by viewModel.photoCount.collectAsState()
            val photoLastTakenMs by viewModel.photoLastTakenMs.collectAsState()
            var confirmAutoRestore by remember { mutableStateOf(false) }
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Backup & restore (.zip)", style = MaterialTheme.typography.bodyMedium, color = onBg)
                Text(
                    "Your whole database in one file. Save it somewhere safe; restore replaces all current data and restarts the app.",
                    style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
                )
                // No backup yet, but there's data worth protecting — nudge toward "Back up" (#5 P1).
                if (noBackupWarning) {
                    Text(
                        "⚠ You haven't backed up yet — your training lives only on this phone. Back up now to be safe.",
                        style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary, fontSize = 10.sp
                    )
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    PillChip("Back up", selected = false) { onBackup(); onDismiss() }
                    PillChip("Restore", selected = false) { onRestore(); onDismiss() }
                }
                // Recover from the silent weekly auto-backup without needing the file picker (#86).
                autoBackupSavedAt?.let { savedAt ->
                    Text(
                        "Restore last auto-backup · saved $savedAt",
                        style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp,
                        modifier = Modifier.clickableLabeled("Restore last auto-backup") { confirmAutoRestore = true }.padding(vertical = 2.dp)
                    )
                }
                // The weekly worker gave up (e.g. storage full) — say so instead of silently losing backups.
                if (autoBackupFailed) {
                    Text(
                        "⚠ Last auto-backup failed — free up storage, then back up manually above.",
                        style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.error, fontSize = 10.sp
                    )
                }
            }
            if (confirmAutoRestore) {
                AlertDialog(
                    onDismissRequest = { confirmAutoRestore = false },
                    title = { Text("Restore last auto-backup?") },
                    text = { Text("Replaces all current data with the weekly auto-backup (saved ${autoBackupSavedAt ?: ""}) and restarts the app.") },
                    confirmButton = {
                        TextButton(onClick = { confirmAutoRestore = false; viewModel.restoreAutoBackup(); onDismiss() }) { Text("Restore") }
                    },
                    dismissButton = { TextButton(onClick = { confirmAutoRestore = false }) { Text("Cancel") } }
                )
            }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))

            // ── Data stake indicator — what's at risk if this device is lost ─────
            if (photoCount > 0) {
                val lastTakenLabel = photoLastTakenMs?.let { formatShortDate(it) }
                val photoLine = photoCountLabel(photoCount) + (lastTakenLabel?.let { " · last $it" } ?: "")
                Text(
                    photoLine,
                    style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
                )
                HorizontalDivider(color = outline.copy(alpha = 0.2f))
            }

            // ── Quick export — one tap, format baked into each row ───────────────
            Text("Quick export", style = MaterialTheme.typography.bodyMedium, color = onBg)
            ExportRow("This week", "JSON", "summary for AI analysis", onBg, muted) { viewModel.exportWeeklyJson(); onDismiss() }
            ExportRow("All sessions", "CSV", "spreadsheet of every session", onBg, muted) { viewModel.exportSessionsCsv(); onDismiss() }
            ExportRow("All PRs", "CSV", "your best lift per exercise", onBg, muted) { viewModel.exportPrsCsv(); onDismiss() }
            ExportRow("Last session", "PDF", "printable session sheet", onBg, muted) { viewModel.exportLastSessionPdf(); onDismiss() }
            ExportRow("Crash logs", "ZIP", "diagnostics if something broke", onBg, muted) { onExportCrashLogs(); onDismiss() }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))

            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp, Alignment.End)) {
                Text("close", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f),
                    modifier = Modifier.clickableLabeled("Close", onClick = onDismiss).padding(4.dp))
            }
        }
    }
}

@Composable
private fun ExportRow(
    label: String,
    format: String,
    hint: String,
    onBg: androidx.compose.ui.graphics.Color,
    muted: androidx.compose.ui.graphics.Color,
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth().clickable { onClick() }.padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(hint, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f), fontSize = 9.sp)
        }
        Text(format, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp, letterSpacing = 1.sp)
    }
}

@Composable
internal fun ResetConfirmDialog(
    target: ResetTarget,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    /** Extra line shown below the main message — used to warn about progress photos on factory reset. */
    photoWarning: String? = null
) {
    // A factory reset is irreversible and wipes EVERYTHING — too dangerous behind a single tap on a
    // shared device. Gate it behind typing a word; lesser resets keep their one-tap confirm.
    val needsTyped = target == ResetTarget.FACTORY
    val confirmWord = "ERASE"
    var typed by remember { mutableStateOf("") }
    val canConfirm = !needsTyped || typed.trim().equals(confirmWord, ignoreCase = true)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(target.label) },
        text = {
            Column {
                Text(target.message)
                if (photoWarning != null) {
                    Spacer(Modifier.height(8.dp))
                    Text(photoWarning, style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error)
                }
                if (needsTyped) {
                    Spacer(Modifier.height(16.dp))
                    OutlinedTextField(
                        value = typed,
                        onValueChange = { typed = it },
                        singleLine = true,
                        label = { Text("Type $confirmWord to confirm") },
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }
        },
        confirmButton = {
            Button(
                onClick = onConfirm,
                enabled = canConfirm,
                colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error)
            ) { Text("Confirm") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}
