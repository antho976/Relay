package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.clickableLabeled

/**
 * Settings → About. Shows the real installed version (read from PackageManager, so no BuildConfig
 * feature needed) and the app's privacy stance: Avex is fully offline and holds no INTERNET
 * permission, so every claim below is verifiable from the manifest.
 */
@Composable
internal fun AboutPage(
    modifier: Modifier = Modifier,
    viewModel: SettingsViewModel? = null,
    onOpenExport: () -> Unit = {},
    onOpenPrivacyPolicy: () -> Unit = {}
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val context = LocalContext.current
    val version = remember {
        runCatching {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName
        }.getOrNull().orEmpty()
    }

    var showCrashLogs by remember { mutableStateOf(false) }
    var showLicenses by remember { mutableStateOf(false) }
    val crashLogs by (viewModel?.crashLogs?.collectAsStateWithLifecycle()
        ?: remember { mutableStateOf<List<Pair<String, String>>?>(null) })

    if (showCrashLogs) {
        LaunchedEffect(Unit) { viewModel?.loadCrashLogs() }
        CrashLogViewerDialog(
            logs = crashLogs,
            onDismiss = { showCrashLogs = false }
        )
    }

    if (showLicenses) {
        LicensesDialog(onDismiss = { showLicenses = false })
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 56.dp)
    ) {
        SettingsSectionHeader("App", top = 12.dp)
        Text(
            if (version.isBlank()) "Offline strength tracker" else "Version $version · Offline strength tracker",
            style = MaterialTheme.typography.bodyMedium,
            color = muted,
            modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
        )

        // §4.3: "a sentences-only section is redesigned to data or cut". The privacy stance was
        // three 40-word paragraphs restating one another; it is four CLAIMS, each verifiable from
        // the manifest, so it renders as four claim rows with the §12 dot carrying present/absent —
        // the same vocabulary Recovery's connection rail uses. The prose said no more than these do.
        SettingsSectionHeader("Your data stays on this device")
        PrivacyClaim("No Internet permission", "Avex physically cannot upload anything.")
        PrivacyClaim("No account, no sign-in", "Everything lives in a private database on this phone.")
        PrivacyClaim("No servers, analytics or tracking", "Nothing is collected, because nothing is sent.")
        PrivacyClaim("Your data moves only when you move it", "Exports and backups go where you point them, through Android's own picker.")
        SettingsActionLink("Read the full privacy policy →", onOpenPrivacyPolicy)
        SettingsActionLink("Export or back up your data →", onOpenExport)

        SettingsSectionHeader("Gestures & shortcuts")
        SettingsExplainer(
            "Hidden gestures built into the training screen and workout logging.",
            Modifier.padding(start = SETTINGS_GUTTER, end = SETTINGS_GUTTER, bottom = 4.dp)
        )
        GestureRow(
            gesture = "Long-press an exercise card",
            action = "Quick actions: skip exercise, open swap picker, set rest timer"
        )
        GestureRow(
            gesture = "Swipe a logged set left",
            action = "Delete that set"
        )
        GestureRow(
            gesture = "Long-press LOG SET",
            action = "Repeat your last set (same weight and reps)"
        )
        GestureRow(
            gesture = "Tap the session strip (sparkline)",
            action = "Open the full exercise history chart sheet"
        )
        GestureRow(
            gesture = "Long-press a day card on the Gym list",
            action = "Change day color · re-roll exercises · edit program for this day"
        )
        SettingsSectionHeader("About")
        SettingsExplainer(
            "A personal gym companion: auto-generated programs, an adaptive coach, progress stats, " +
                "trophies and a rank ladder. Built for lifting, not for the cloud.",
            Modifier.padding(horizontal = SETTINGS_GUTTER)
        )
        Text(
            "Avex · a solo-built, offline-first project.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = SETTINGS_GUTTER, vertical = 8.dp)
        )

        // §8 ③: these open something, so they are `action →` links, not muted body sentences.
        SettingsSectionHeader("Diagnostics & licenses")
        SettingsExplainer(
            "Built on Jetpack Compose, Room, Hilt and Health Connect (Apache 2.0), with the " +
                "anatomical figures adapted from react-native-body-highlighter (MIT).",
            Modifier.padding(horizontal = SETTINGS_GUTTER)
        )
        if (viewModel != null) {
            SettingsActionLink("View crash logs →") { showCrashLogs = true }
        }
        SettingsActionLink("View licenses →") { showLicenses = true }
        Spacer(Modifier.height(8.dp))
    }
}

/** One verifiable privacy claim: the §12 dot carries "true of this build", the words say what of. */
@Composable
private fun PrivacyClaim(claim: String, detail: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Box(Modifier.padding(top = 5.dp)) { StatusDot(active = true, size = 7.dp) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(claim, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
            SettingsExplainer(detail)
        }
    }
}

/** A gesture and what it does. Each row carries a distinct action, which is what earns a list over
 *  one mark (§4.10) — the pair reads as a reference table, so it keeps the label/explainer shape. */
@Composable
private fun GestureRow(gesture: String, action: String) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD)
    ) {
        Text(gesture, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
        SettingsExplainer(action)
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
