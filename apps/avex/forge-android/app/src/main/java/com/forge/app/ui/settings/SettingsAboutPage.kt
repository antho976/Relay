package com.forge.app.ui.settings

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
import com.forge.app.ui.theme.emphasized

/**
 * Settings → About. Shows the real installed version (read from PackageManager, so no BuildConfig
 * feature needed) and the app's privacy stance: Forge is fully offline and holds no INTERNET
 * permission, so every claim below is verifiable from the manifest.
 */
@Composable
internal fun AboutPage(modifier: Modifier = Modifier, viewModel: SettingsViewModel? = null) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val context = LocalContext.current
    val version = remember {
        runCatching {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName
        }.getOrNull().orEmpty()
    }

    var showCrashLogs by remember { mutableStateOf(false) }
    val crashLogs by (viewModel?.crashLogs?.collectAsStateWithLifecycle()
        ?: remember { mutableStateOf<List<Pair<String, String>>?>(null) })

    if (showCrashLogs) {
        LaunchedEffect(Unit) { viewModel?.loadCrashLogs() }
        CrashLogViewerDialog(
            logs = crashLogs,
            onDismiss = { showCrashLogs = false }
        )
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 56.dp)
    ) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 20.dp, bottom = 16.dp)) {
            Text("FORGE", style = MaterialTheme.typography.headlineSmall, color = emphasized(onBg))
            Text(
                if (version.isBlank()) "Offline strength tracker" else "Version $version · Offline strength tracker",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 11.sp
            )
        }
        SectionDivider()

        SectionLabel("YOUR DATA STAYS ON THIS DEVICE")
        AboutParagraph(
            "Everything you log lives in a private database on this phone. There's no account and no " +
                "sign-in — Forge doesn't even request the Internet permission, so it physically can't " +
                "upload your data anywhere."
        )
        AboutParagraph(
            "No servers, no analytics, no tracking. Your data only moves when you choose to move it — " +
                "exporting or sharing a file, or saving a backup — and it goes exactly where you point it " +
                "through Android's own share / file picker."
        )
        AboutParagraph(
            "Keep your own backups from Settings → Export data → full backup; restoring on a new phone " +
                "brings everything back. Turn on Privacy to block screenshots and hide the app preview in " +
                "recent apps."
        )
        SectionDivider()

        SectionLabel("GESTURES & SHORTCUTS")
        AboutCaption("Hidden gestures built into the training screen and workout logging.")
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
        SectionDivider()

        SectionLabel("ABOUT")
        AboutParagraph(
            "A personal gym companion — auto-generated programs, an adaptive coach, progress stats, " +
                "trophies and a rank ladder. Built for lifting, not for the cloud."
        )
        if (viewModel != null) {
            Text(
                "View crash logs",
                style = MaterialTheme.typography.bodySmall,
                color = muted,
                modifier = Modifier
                    .clickableLabeled("View crash logs") { showCrashLogs = true }
                    .padding(horizontal = 24.dp, vertical = 10.dp)
            )
        }
        Text(
            "Forge · a solo-built, offline-first project.",
            style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.6f),
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
        )
        Spacer(Modifier.height(8.dp))
    }
}

@Composable
private fun AboutParagraph(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
    )
}

@Composable
private fun AboutCaption(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
        modifier = Modifier.padding(start = 24.dp, end = 24.dp, bottom = 4.dp)
    )
}

@Composable
private fun GestureRow(gesture: String, action: String) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 6.dp)
    ) {
        Text(
            gesture,
            style = MaterialTheme.typography.bodySmall,
            color = muted
        )
        Text(
            action,
            style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.6f),
            fontSize = 10.sp
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
                        "No crashes recorded — nice.",
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
                                fontFamily = FontFamily.Monospace,
                                fontSize = 10.sp
                            ),
                            color = muted.copy(alpha = 0.85f)
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
