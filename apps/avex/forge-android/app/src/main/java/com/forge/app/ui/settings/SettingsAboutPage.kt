package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.rounded.BugReport
import androidx.compose.material.icons.rounded.CloudOff
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.NoAccounts
import androidx.compose.material.icons.rounded.Policy
import androidx.compose.material.icons.rounded.VisibilityOff
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.appicon.AppIcon

/**
 * Settings → About. Who the app is (the installed icon, name and real version, read from the
 * package), the privacy stance as four claims each verifiable from the manifest, the hidden
 * gestures, and diagnostics.
 */
@Composable
internal fun AboutPage(
    onBack: () -> Unit,
    viewModel: SettingsViewModel? = null,
    onOpenExport: () -> Unit = {},
    onOpenPrivacyPolicy: () -> Unit = {}
) {
    val context = LocalContext.current
    val version = remember {
        runCatching {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName
        }.getOrNull().orEmpty()
    }
    val iconKey = viewModel?.state?.collectAsStateWithLifecycle()?.value?.appIconKey

    var showCrashLogs by remember { mutableStateOf(false) }
    var showLicenses by remember { mutableStateOf(false) }
    val crashLogs by (viewModel?.crashLogs?.collectAsStateWithLifecycle()
        ?: remember { mutableStateOf<List<Pair<String, String>>?>(null) })

    if (showCrashLogs) {
        LaunchedEffect(Unit) { viewModel?.loadCrashLogs() }
        CrashLogViewerDialog(logs = crashLogs, onDismiss = { showCrashLogs = false })
    }
    if (showLicenses) LicensesDialog(onDismiss = { showLicenses = false })

    SettingsScaffold("About", onBack) {
        SettingsGroup {
            SettingsGroupBlock(padding = PaddingValues(20.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    AppIconPreviewImage(
                        AppIcon.fromKey(iconKey.orEmpty()).previewRes,
                        Modifier.size(56.dp).clip(RoundedCornerShape(14.dp))
                    )
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text("Avex", style = MaterialTheme.typography.headlineMedium, color = MaterialTheme.colorScheme.onSurface)
                        Text(
                            if (version.isBlank()) "Offline strength tracker" else "Version $version",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
                Spacer(Modifier.height(16.dp))
                Text(
                    "A personal gym companion: generated programs, an adaptive coach, progress stats, " +
                        "trophies and a rank ladder. Built for lifting, not for the cloud.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }

        // The privacy stance is four claims, each verifiable from the manifest.
        SettingsGroup("Your data stays on this device") {
            SettingsInfoRow("No internet permission", "Avex physically cannot upload anything", Icons.Rounded.CloudOff)
            SettingsInfoRow("No account, no sign-in", "Everything lives in a private database on this phone", Icons.Rounded.NoAccounts)
            SettingsInfoRow("No servers, analytics or tracking", "Nothing is collected, because nothing is sent", Icons.Rounded.VisibilityOff)
            SettingsInfoRow("Your data moves only when you move it", "Exports and backups go where you point them", Icons.Rounded.FolderOpen)
            SettingsNavigationRow("Privacy policy", "Offline use, permissions and deletion", Icons.Rounded.Policy, onClick = onOpenPrivacyPolicy)
            SettingsNavigationRow("Export or back up your data", icon = SettingsIcons.Export, onClick = onOpenExport)
        }

        SettingsGroup("Gestures & shortcuts") {
            SettingsInfoRow("Long-press an exercise card", "Skip it, swap it, or set its rest timer")
            SettingsInfoRow("Swipe a logged set left", "Delete that set")
            SettingsInfoRow("Long-press Log set", "Repeat your last set, same weight and reps")
            SettingsInfoRow("Tap the session sparkline", "Open the exercise's full history chart")
            SettingsInfoRow("Long-press a day on the Gym list", "Change its color, re-roll it, or edit that day")
        }

        SettingsGroup(
            "Diagnostics & licenses",
            footer = "Built on Jetpack Compose, Room, Hilt and Health Connect (Apache 2.0), with the anatomical " +
                "figures adapted from react-native-body-highlighter (MIT)."
        ) {
            if (viewModel != null) {
                SettingsNavigationRow("Crash logs", "Kept on this phone, never sent", Icons.Rounded.BugReport) { showCrashLogs = true }
            }
            SettingsNavigationRow("Open-source licenses", icon = Icons.Outlined.Description) { showLicenses = true }
        }

        Text(
            "Avex · a solo-built, offline-first project",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontStyle = FontStyle.Italic,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth()
        )
    }
}

@Composable
private fun CrashLogViewerDialog(
    logs: List<Pair<String, String>>?,
    onDismiss: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    AlertDialog(
        containerColor = MaterialTheme.colorScheme.surface,
        onDismissRequest = onDismiss,
        title = { Text("Crash logs") },
        text = {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                // null = still loading from disk; only show the all-clear once the read has returned,
                // so we never flash "No crashes recorded" before the logs actually load.
                if (logs == null) {
                    Text(
                        "Loading crash logs…",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted
                    )
                } else if (logs.isEmpty()) {
                    Text(
                        "No crashes recorded.",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted
                    )
                } else {
                    logs.forEachIndexed { index, (name, text) ->
                        if (index > 0) Spacer(Modifier.height(16.dp))
                        Text(
                            name,
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            letterSpacing = 0.5.sp
                        )
                        Spacer(Modifier.height(4.dp))
                        Text(
                            text,
                            style = MaterialTheme.typography.bodySmall.copy(
                                fontFamily = FontFamily.Monospace
                            ),
                            color = muted.copy(alpha = 0.7f)
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Close") }
        }
    )
}

@Composable
private fun LicensesDialog(onDismiss: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    AlertDialog(
        containerColor = MaterialTheme.colorScheme.surface,
        onDismissRequest = onDismiss,
        title = { Text("Open-source licenses") },
        text = {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                Text(
                    "react-native-body-highlighter",
                    style = MaterialTheme.typography.labelSmall,
                    color = onBg,
                    letterSpacing = 0.5.sp
                )
                Text(
                    "Anatomical front/back muscle figures.",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.7f)
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    MIT_LICENSE_BODY_HIGHLIGHTER,
                    style = MaterialTheme.typography.bodySmall.copy(
                        fontFamily = FontFamily.Monospace
                    ),
                    color = muted.copy(alpha = 0.7f)
                )
                Spacer(Modifier.height(16.dp))
                Text(
                    "Jetpack Compose, Room, Hilt, Health Connect and other AndroidX libraries are " +
                        "licensed under the Apache License 2.0 (© Google LLC and contributors).",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted
                )
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Close") }
        }
    )
}

private const val MIT_LICENSE_BODY_HIGHLIGHTER = """MIT License

Copyright (c) 2022 ELABBASSI Hicham

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""
