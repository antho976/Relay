package com.forge.app.ui.settings

import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.ForgeSwitch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.health.connect.client.PermissionController
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle

/**
 * Settings → Recovery. Connects Health Connect so the coach can read sleep + resting heart rate
 * as recovery signals (finding #2/#3). Everything is opt-in and reversible — denying or never
 * connecting leaves the coach exactly as it is, reading only on-app signals.
 */
@Composable
internal fun RecoveryPage(modifier: Modifier = Modifier, viewModel: HealthConnectViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    // Health Connect's own permission flow — the result tells us what the user granted. Recovery
    // (sleep + resting HR) and bodyweight are separate launchers so each stays independently opt-in.
    val permissionLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val weightLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val calorieLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val stepsLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val exerciseLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 56.dp)
    ) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 20.dp, bottom = 12.dp)) {
            Text("RECOVERY", style = MaterialTheme.typography.headlineSmall, color = onBg)
            Text(
                "Let the coach read your sleep and resting heart rate",
                style = MaterialTheme.typography.labelSmall, color = muted
            )
        }
        SectionDivider()

        Paragraph(
            "Connect Health Connect and the coach can factor your recovery into its deload call — short " +
                "sleep or an elevated resting heart rate add to the fatigue score, alongside your effort, " +
                "moods and rest-day flags."
        )
        Paragraph(
            "Health Connect is Android's on-device hub. Apps like Samsung Health and your watch write to " +
                "it; Forge only reads sleep and resting heart rate from it. Nothing leaves your phone — Forge " +
                "still has no Internet permission."
        )
        Paragraph(
            "It's optional and additive: skip it and the coach works exactly as before. You can disconnect " +
                "any time from the Health Connect app."
        )

        SectionLabel("STATUS")
        when {
            state.loading -> StatusLine("Checking…", muted)
            state.needsUpdate -> {
                StatusLine("Health Connect needs an update.", muted)
                ActionButton("Update Health Connect") { openHealthConnectInStore(context) }
            }
            !state.available -> {
                StatusLine("Health Connect isn't installed on this device.", muted)
                ActionButton("Get Health Connect") { openHealthConnectInStore(context) }
            }
            state.granted -> {
                StatusLine("Connected — reading sleep & resting heart rate.", onBg)
                ActionButton("Manage in Health Connect") { openHealthConnectSettings(context) }
            }
            else -> {
                StatusLine("Health Connect is available but not connected yet.", muted)
                ActionButton("Connect Health Connect") { permissionLauncher.launch(viewModel.permissions) }
            }
        }

        // ─── Bodyweight sync (HC-2/HC-3) — only meaningful once a provider exists ──────────────
        if (state.available) {
            Spacer(Modifier.height(20.dp))
            SectionDivider()
            Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 4.dp)) {
                Text("BODYWEIGHT SYNC", style = MaterialTheme.typography.headlineSmall, color = onBg)
            }
            Paragraph(
                "Forge can read your latest weight from a smart scale that writes to Health Connect, so " +
                    "your bodyweight trend stays current without typing it. You can also write your Forge " +
                    "weigh-ins back, so other apps see them."
            )
            if (state.weightGranted) {
                StatusLine("Connected — Forge can read and write your weight.", onBg)
                ActionButton("Import latest weight now") { viewModel.importNow() }
                state.importMessage?.let { StatusLine(it, muted) }
                ToggleRow(
                    label = "Write my weigh-ins to Health Connect",
                    checked = state.writeBodyweight,
                    onCheckedChange = { viewModel.setWriteBodyweight(it) }
                )
            } else {
                StatusLine("Bodyweight isn't connected yet.", muted)
                ActionButton("Connect bodyweight") { weightLauncher.launch(viewModel.weightPermissions) }
            }

            // ─── Workout calories (HC-4) — write a session's estimated burn out to HC ─────────────
            Spacer(Modifier.height(20.dp))
            SectionDivider()
            Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 4.dp)) {
                Text("WORKOUT CALORIES", style = MaterialTheme.typography.headlineSmall, color = onBg)
            }
            Paragraph(
                "Forge can write each finished session's estimated active calories to Health Connect, so " +
                    "your daily energy total there includes your lifting. It's an estimate from session " +
                    "length, your intensity, and your latest logged bodyweight — Forge has no heart-rate stream."
            )
            if (state.calorieGranted) {
                StatusLine("Connected — Forge can write your session calories.", onBg)
                ToggleRow(
                    label = "Write my session calories to Health Connect",
                    checked = state.writeCalories,
                    onCheckedChange = { viewModel.setWriteCalories(it) }
                )
            } else {
                StatusLine("Calorie sync isn't connected yet.", muted)
                ActionButton("Connect calories") { calorieLauncher.launch(viewModel.caloriePermissions) }
            }

            // ─── Steps (read a watch/ring's step counts for the cardio screen) ─────────────────────
            Spacer(Modifier.height(20.dp))
            SectionDivider()
            Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 4.dp)) {
                Text("STEPS & ACTIVITY", style = MaterialTheme.typography.headlineSmall, color = onBg)
            }
            Paragraph(
                "Connect a watch or ring (via Samsung Health, Fitbit, etc.) that writes steps to Health " +
                    "Connect, and Forge shows your steps-through-the-day graph on a cardio session and the " +
                    "current week. Read-only — Forge never writes your steps anywhere."
            )
            if (state.stepsGranted) {
                StatusLine("Connected — Forge can read your steps.", onBg)
                ActionButton("Manage in Health Connect") { openHealthConnectSettings(context) }
            } else {
                StatusLine("Steps aren't connected yet.", muted)
                ActionButton("Connect steps") { stepsLauncher.launch(viewModel.stepsPermissions) }
            }

            // ─── GPS routes (read a watch's outdoor sessions to draw their route shape) ─────────────
            Spacer(Modifier.height(20.dp))
            SectionDivider()
            Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 4.dp)) {
                Text("GPS ROUTES", style = MaterialTheme.typography.headlineSmall, color = onBg)
            }
            Paragraph(
                "If your watch records outdoor runs or rides with GPS, Forge can draw the route's shape on " +
                    "the matching cardio session. There's no map — just the path, offline. Health Connect " +
                    "asks you to confirm each route the first time Forge draws it."
            )
            if (state.exerciseGranted) {
                StatusLine("Connected — Forge can offer your GPS routes.", onBg)
                ActionButton("Manage in Health Connect") { openHealthConnectSettings(context) }
            } else {
                StatusLine("GPS routes aren't connected yet.", muted)
                ActionButton("Connect GPS routes") { exerciseLauncher.launch(viewModel.exercisePermissions) }
            }
        }

        Spacer(Modifier.height(12.dp))
    }
}

@Composable
private fun ToggleRow(label: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.weight(1f).padding(end = 16.dp)
        )
        ForgeSwitch(checked = checked, onCheckedChange = onCheckedChange)
    }
}

@Composable
private fun Paragraph(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
    )
}

@Composable
private fun StatusLine(text: String, color: Color) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = color,
        fontStyle = FontStyle.Italic,
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp)
    )
}

@Composable
private fun ActionButton(label: String, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    Button(
        onClick = onClick,
        shape = RoundedCornerShape(50),
        colors = ButtonDefaults.buttonColors(containerColor = onBg, contentColor = MaterialTheme.colorScheme.background),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp)
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
}

private const val HEALTH_CONNECT_PACKAGE = "com.google.android.apps.healthdata"

/** Open the Play store page for the Health Connect provider (web fallback if Play is absent). */
private fun openHealthConnectInStore(context: android.content.Context) {
    val market = Intent(Intent.ACTION_VIEW, Uri.parse("market://details?id=$HEALTH_CONNECT_PACKAGE"))
    val web = Intent(Intent.ACTION_VIEW, Uri.parse("https://play.google.com/store/apps/details?id=$HEALTH_CONNECT_PACKAGE"))
    runCatching { context.startActivity(market) }.recoverCatching { context.startActivity(web) }
}

/** Open the Health Connect app's management UI so the user can review or revoke access. */
private fun openHealthConnectSettings(context: android.content.Context) {
    runCatching {
        context.startActivity(Intent(androidx.health.connect.client.HealthConnectClient.ACTION_HEALTH_CONNECT_SETTINGS))
    }.recoverCatching {
        context.startActivity(context.packageManager.getLaunchIntentForPackage(HEALTH_CONNECT_PACKAGE)!!)
    }
}
