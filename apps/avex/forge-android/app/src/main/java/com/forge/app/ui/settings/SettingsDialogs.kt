@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
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
        // §1/§5: a modal KEEPS its surface. Painting it `background` made the sheet vanish into the
        // page it floats over, which is the one place the open-editorial rule does not apply.
        val surface = MaterialTheme.colorScheme.surface

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(surface, RoundedCornerShape(16.dp))
                // §14: the dialog must survive the biggest font. Eight export rows plus the backup
                // block do not fit a 200% viewport, and it had no scroll at all.
                .verticalScroll(rememberScrollState())
                .padding(vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            SettingsSectionHeader("Data", top = 0.dp)

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
            val dbSize by viewModel.dbSizeLabel.collectAsState()
            var confirmAutoRestore by remember { mutableStateOf(false) }
            Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)) {
                Text("Backup & restore (.zip)", style = MaterialTheme.typography.bodyMedium, color = onBg)
                SettingsExplainer("Your whole database in one file. Save it somewhere safe; restore replaces all current data and restarts the app.")
                // No backup yet, but there's data worth protecting — nudge toward "Back up" (#5 P1).
                // §14 bans accent-coloured body text: only Ember clears AA, the other four presets
                // measure 2.34–3.37:1. The accent moves to a DOT and the sentence stays onBg — the
                // one treatment that is correct under every accent choice, monochrome included.
                if (noBackupWarning) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        StatusDot(active = true, size = 7.dp)
                        Text(
                            "You haven't backed up yet. Your training lives only on this phone.",
                            style = MaterialTheme.typography.bodySmall, color = onBg
                        )
                    }
                }
                // Two fillMaxWidth capsules used to sit in a plain Row here, so "Back up" took the
                // whole line and "Restore" got the remainder. Gutterless capsules in a FlowRow wrap
                // instead of overflowing at large font scales. (Not SettingsActionRow — the parent
                // Column already owns the gutter inside a dialog.)
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    SettingsPrimaryAction("Back up") { onBackup(); onDismiss() }
                    SettingsOutlineAction("Restore") { onRestore(); onDismiss() }
                }
                // Recover from the silent weekly auto-backup without needing the file picker (#86).
                autoBackupSavedAt?.let { savedAt ->
                    Text(
                        "Restore last auto-backup · saved $savedAt",
                        style = MaterialTheme.typography.bodySmall, color = muted,
                        modifier = Modifier
                            .clickableLabeled("Restore last auto-backup") { confirmAutoRestore = true }
                            .padding(vertical = 10.dp)
                    )
                }
                // The weekly worker gave up (e.g. storage full) — say so instead of silently losing backups.
                // §12's inline error line — the one sanctioned use of error-as-text, kept quiet.
                if (autoBackupFailed) {
                    Text(
                        "Last auto-backup failed. Free up storage, then back up manually above.",
                        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error
                    )
                }
            }
            if (confirmAutoRestore) {
                AlertDialog(
                    containerColor = MaterialTheme.colorScheme.surface,
                    onDismissRequest = { confirmAutoRestore = false },
                    title = { Text("Restore last auto-backup?") },
                    text = { Text("Replaces all current data with the weekly auto-backup (saved ${autoBackupSavedAt ?: ""}) and restarts the app.") },
                    confirmButton = {
                        TextButton(onClick = { confirmAutoRestore = false; viewModel.restoreAutoBackup(); onDismiss() }) { Text("Restore") }
                    },
                    dismissButton = { TextButton(onClick = { confirmAutoRestore = false }) { Text("Cancel") } }
                )
            }

            // ── Data stake indicator — DB size + what's at risk if this device is lost ─────
            val stakeLine = buildString {
                append("Database")
                if (dbSize.isNotBlank()) append(" · $dbSize")
                if (photoCount > 0) {
                    append(" · ${photoCountLabel(photoCount)}")
                    photoLastTakenMs?.let { append(" · last ${formatShortDate(it)}") }
                }
            }
            SettingsExplainer(stakeLine, Modifier.padding(horizontal = SETTINGS_GUTTER))

            // ── Quick export — one tap, format baked into each row ───────────────
            // Their own Column: the dialog's spacedBy(16) is section rhythm, and applying it
            // BETWEEN eight sibling rows on top of each row's own padding pulls the group apart.
            // Each row carries a distinct format and action, which is what earns a list (§4.10).
            Column {
                SettingsSectionHeader("Quick export", top = 0.dp)
                ExportRow("All my data", "JSON", "every session, set & setting", onBg, muted) { viewModel.exportFullBackup(); onDismiss() }
                ExportRow("This week", "JSON", "summary for AI analysis", onBg, muted) { viewModel.exportWeeklyJson(); onDismiss() }
                ExportRow("All sessions", "CSV", "spreadsheet of every session", onBg, muted) { viewModel.exportSessionsCsv(); onDismiss() }
                ExportRow("All PRs", "CSV", "your best lift per exercise", onBg, muted) { viewModel.exportPrsCsv(); onDismiss() }
                ExportRow("Bodyweight", "CSV", "every weigh-in", onBg, muted) { viewModel.exportBodyweightCsv(); onDismiss() }
                ExportRow("Cardio", "CSV", "every cardio session", onBg, muted) { viewModel.exportCardioCsv(); onDismiss() }
                ExportRow("Last session", "PDF", "printable session sheet", onBg, muted) { viewModel.exportLastSessionPdf(); onDismiss() }
                ExportRow("Crash logs", "ZIP", "diagnostics if something broke", onBg, muted) { onExportCrashLogs(); onDismiss() }
            }

            // §5: a modal's dismiss is muted at FULL strength. It was `muted@0.6` (4.08:1) and
            // lower-case, i.e. the least legible text in the app on the least reversible screen.
            SettingsActionRow {
                SettingsOutlineAction("Close", onClick = onDismiss)
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
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled("Export $label as $format", onClick = onClick)
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            SettingsExplainer(hint)
        }
        Text(format, style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.sp)
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
        containerColor = MaterialTheme.colorScheme.surface,
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
            TextButton(onClick = onConfirm, enabled = canConfirm) {
                Text(
                    "Confirm",
                    color = MaterialTheme.colorScheme.onBackground.copy(alpha = if (canConfirm) 1f else 0.35f)
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    )
}

/**
 * Chooser for the targeted resets — everything except [ResetTarget.FACTORY], which keeps its own
 * dedicated (more dangerous) button. Picking an option here still routes through [ResetConfirmDialog]
 * so each reset keeps its own confirmation.
 */
@Composable
internal fun ResetMenuDialog(
    onPick: (ResetTarget) -> Unit,
    onDismiss: () -> Unit
) {
    Dialog(onDismissRequest = onDismiss) {
        val onBg = MaterialTheme.colorScheme.onBackground
        val surface = MaterialTheme.colorScheme.surface
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(surface, RoundedCornerShape(16.dp))
                .verticalScroll(rememberScrollState())   // §14 — four reset rows + explainers at 200%
                .padding(vertical = 20.dp)
        ) {
            SettingsSectionHeader("Reset", top = 0.dp)
            // Air separates these rows, not a rule (§1: a line exists only as data). The hairline
            // that used to sit between them is the "hairline habit" named in FAILURES.md.
            ResetTarget.entries.filter { it != ResetTarget.FACTORY }.forEach { target ->
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickableLabeled(target.label) { onPick(target) }
                        .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
                    verticalArrangement = Arrangement.spacedBy(2.dp)
                ) {
                    Text(target.label, style = MaterialTheme.typography.bodyMedium, color = onBg)
                    SettingsExplainer(target.message)
                }
            }
            Spacer(Modifier.height(12.dp))
            SettingsActionRow {
                SettingsOutlineAction("Cancel", onClick = onDismiss)
            }
        }
    }
}
