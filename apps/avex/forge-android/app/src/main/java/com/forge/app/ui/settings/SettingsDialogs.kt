@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog

private data class ExportOption(val label: String, val format: String, val action: () -> Unit)

@Composable
internal fun DataExportDialog(viewModel: SettingsViewModel, onDismiss: () -> Unit) {
    var selectedIdx by remember { mutableStateOf<Int?>(null) }
    var selectedFormat by remember { mutableStateOf<String?>(null) }

    val options = remember(viewModel) {
        listOf(
            ExportOption("Sessions", "CSV") { viewModel.exportSessionsCsv() },
            ExportOption("Weekly summary", "JSON") { viewModel.exportWeeklyJson() },
            ExportOption("Full backup", "JSON") { viewModel.exportFullBackup() },
            ExportOption("Last session", "PDF") { viewModel.exportLastSessionPdf() }
        )
    }

    val validFormat = selectedIdx?.let { options[it].format }
    val canExport = selectedIdx != null && selectedFormat == validFormat

    Dialog(onDismissRequest = onDismiss) {
        val onBg = MaterialTheme.colorScheme.onBackground
        val muted = MaterialTheme.colorScheme.onSurfaceVariant
        val outline = MaterialTheme.colorScheme.outline
        val bg = MaterialTheme.colorScheme.background

        Column(
            modifier = Modifier.fillMaxWidth().background(bg, RoundedCornerShape(8.dp)).padding(horizontal = 24.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text("EXPORT DATA", style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.5.sp)

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("What?", style = MaterialTheme.typography.bodyMedium, color = onBg)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    options.forEachIndexed { idx, opt ->
                        PillChip("${opt.label}\n${opt.format}", selected = selectedIdx == idx) {
                            selectedIdx = idx; selectedFormat = opt.format
                        }
                    }
                }
            }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Format?", style = MaterialTheme.typography.bodyMedium, color = onBg)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("CSV", "JSON", "PDF").forEach { fmt ->
                        val isValid = validFormat == fmt
                        PillChip(fmt, selected = selectedFormat == fmt, enabled = isValid) {
                            if (isValid) selectedFormat = fmt
                        }
                    }
                }
            }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))

            Row(modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(20.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically) {
                Text("cancel", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f),
                    modifier = Modifier.clickable(onClick = onDismiss).padding(4.dp))
                Text("Export →", style = MaterialTheme.typography.bodyMedium,
                    color = if (canExport) onBg else onBg.copy(alpha = 0.3f),
                    modifier = if (canExport) Modifier.clickable { options[selectedIdx!!].action(); onDismiss() }.padding(4.dp)
                    else Modifier.padding(4.dp))
            }
        }
    }
}

@Composable
internal fun ResetConfirmDialog(target: ResetTarget, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(target.label) },
        text = { Text(target.message) },
        confirmButton = {
            Button(onClick = onConfirm, colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error)) { Text("Confirm") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}
