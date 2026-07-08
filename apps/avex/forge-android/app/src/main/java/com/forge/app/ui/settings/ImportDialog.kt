package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.forge.app.data.importer.FoundImport
import com.forge.app.ui.common.clickableLabeled

/**
 * Import-from-another-app modal (#GYMAP-17). Leads with the files it FOUND for you: once you've
 * pointed Avex at a folder (Downloads), it lists the recognised exports for a one-tap import. Manual
 * file pick and the share-in route are the fallbacks below. Modal archetype — surface fill, dry copy.
 */
@Composable
internal fun ImportDialog(
    viewModel: SettingsViewModel,
    onGrantFolder: () -> Unit,
    onManualPick: () -> Unit,
    onImportFound: (android.net.Uri) -> Unit,
    onDismiss: () -> Unit
) {
    Dialog(onDismissRequest = onDismiss) {
        val onBg = MaterialTheme.colorScheme.onBackground
        val muted = MaterialTheme.colorScheme.onSurfaceVariant
        val outline = MaterialTheme.colorScheme.outline
        val bg = MaterialTheme.colorScheme.background

        val granted by viewModel.importFolderGranted.collectAsState()
        val scanning by viewModel.scanningImports.collectAsState()
        val found by viewModel.foundImports.collectAsState()

        // Re-scan whenever the sheet opens with access already granted (new exports may have landed).
        LaunchedEffect(granted) { if (granted) viewModel.scanImportFolder() }

        Column(
            modifier = Modifier.fillMaxWidth().background(bg, RoundedCornerShape(8.dp)).padding(horizontal = 24.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text("IMPORT", style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.5.sp)
            Text(
                "Add history from Strong, Hevy, FitNotes, or any CSV export. Added to your log, not replacing it.",
                style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
            )

            // ── Found files — the headline path ──────────────────────────────────
            if (!granted) {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    PillChip("Find my exports", selected = false) { onGrantFolder() }
                    Text(
                        "Point Avex at your Downloads folder once; it lists the exports it finds there.",
                        style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f), fontSize = 10.sp
                    )
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    when {
                        scanning -> Text("Scanning…", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
                        found.isEmpty() -> Text(
                            "No exports found in that folder.",
                            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
                        )
                        else -> found.forEach { file ->
                            FoundFileRow(file, onBg, muted) { onImportFound(file.uri); onDismiss() }
                        }
                    }
                    Text(
                        "Choose a different folder",
                        style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary, fontSize = 10.sp,
                        modifier = Modifier.clickableLabeled("Choose a different folder") { onGrantFolder() }.padding(vertical = 2.dp)
                    )
                }
            }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))

            // ── Fallbacks: pick a file, or share in from the other app ────────────
            Text(
                "Choose a file",
                style = MaterialTheme.typography.bodyMedium, color = onBg,
                modifier = Modifier.fillMaxWidth().clickable { onManualPick(); onDismiss() }.padding(vertical = 2.dp)
            )
            Text(
                "Or in the other app: Export, then Share to Avex.",
                style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
            )

            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp, Alignment.End)) {
                Text("close", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.6f),
                    modifier = Modifier.clickableLabeled("Close", onClick = onDismiss).padding(4.dp))
            }
        }
    }
}

/** One recognised export: filename + "app · N workouts · date", tap to import. */
@Composable
private fun FoundFileRow(
    file: FoundImport,
    onBg: androidx.compose.ui.graphics.Color,
    muted: androidx.compose.ui.graphics.Color,
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth().clickable { onClick() }.padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(1.dp), modifier = Modifier.padding(end = 12.dp)) {
            Text(file.name, style = MaterialTheme.typography.bodyMedium, color = onBg, maxLines = 1)
            Text(
                "${file.source.displayName} · ${file.sessionCount} ${if (file.sessionCount == 1) "workout" else "workouts"} · ${formatShortDate(file.lastModified)}",
                style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f), fontSize = 9.sp
            )
        }
        Text("import", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp, letterSpacing = 1.sp)
    }
}
