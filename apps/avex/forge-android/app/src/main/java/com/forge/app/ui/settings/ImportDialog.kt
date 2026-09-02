package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
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
import com.forge.app.data.importer.foundImportSummary
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
        // §1/§5: a modal keeps its SURFACE — painting it `background` dissolved the sheet into
        // the page it floats over.
        val surface = MaterialTheme.colorScheme.surface

        val granted by viewModel.importFolderGranted.collectAsState()
        val scanning by viewModel.scanningImports.collectAsState()
        val found by viewModel.foundImports.collectAsState()

        // Re-scan whenever the sheet opens with access already granted (new exports may have landed).
        LaunchedEffect(granted) { if (granted) viewModel.scanImportFolder() }

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(surface, RoundedCornerShape(16.dp))
                .verticalScroll(rememberScrollState())   // §14 — survives 200% and a long found-file list
                .padding(vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            SettingsSectionHeader("Import", top = 0.dp)
            SettingsExplainer(
                "Add history from Strong, Hevy, FitNotes, or any CSV export. Added to your log, not replacing it.",
                Modifier.padding(horizontal = SETTINGS_GUTTER)
            )

            // ── Found files — the headline path ──────────────────────────────────
            if (!granted) {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    SettingsActionRow {
                        SettingsOutlineAction("Find my exports") { onGrantFolder() }
                    }
                    SettingsExplainer(
                        "Point Avex at your Downloads folder once; it lists the exports it finds there.",
                        Modifier.padding(horizontal = SETTINGS_GUTTER)
                    )
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    when {
                        scanning -> SettingsExplainer("Scanning…", Modifier.padding(horizontal = SETTINGS_GUTTER))
                        found.isEmpty() -> SettingsExplainer(
                            "No exports found in that folder.",
                            Modifier.padding(horizontal = SETTINGS_GUTTER)
                        )
                        else -> found.forEach { file ->
                            FoundFileRow(file, onBg, muted) { onImportFound(file.uri); onDismiss() }
                        }
                    }
                    // §8 ③ is the mono accent `action →`, not an accent-coloured sentence (§14).
                    SettingsActionLink("Choose a different folder →") { onGrantFolder() }
                }
            }

            // ── Fallbacks: pick a file, or share in from the other app ────────────
            Text(
                "Choose a file",
                style = MaterialTheme.typography.bodyMedium, color = onBg,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickableLabeled("Choose a file") { onManualPick(); onDismiss() }
                    .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD)
            )
            SettingsExplainer(
                "Or in the other app: Export, then Share to Avex.",
                Modifier.padding(horizontal = SETTINGS_GUTTER)
            )

            SettingsActionRow {
                SettingsOutlineAction("Close", onClick = onDismiss)
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
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled("Import ${file.name}", onClick = onClick)
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(1.dp), modifier = Modifier.padding(end = 12.dp)) {
            // No maxLines: a file NAME is user content and wraps rather than truncating (§14).
            Text(file.name, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(
                // What the file HOLDS, not just its workout count (L-01): a bodyweight CSV carries
                // no workouts at all, and describing it as "0 workouts" is why it read as empty.
                "${foundImportSummary(file)} · ${formatShortDate(file.lastModified)}",
                style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp
            )
        }
        Text("IMPORT", style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.sp)
    }
}
