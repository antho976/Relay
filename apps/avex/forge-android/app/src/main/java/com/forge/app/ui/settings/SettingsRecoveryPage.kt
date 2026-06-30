package com.forge.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.health.connect.client.PermissionController
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle

/**
 * Settings → Recovery. Connects Health Connect so the coach and cardio screen can use what your
 * watch and scale already track — sleep, resting heart rate, bodyweight, calories, steps and GPS.
 * Each integration is its own [RecoveryCard]: a one-line summary up top, a "Why this?" expander for
 * the full detail, and a single action. Everything is opt-in and reversible — denying or never
 * connecting leaves the coach exactly as it is, reading only on-app signals.
 */
@Composable
internal fun RecoveryPage(modifier: Modifier = Modifier, viewModel: HealthConnectViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    // Health Connect's own permission flow — the result tells us what the user granted. Each
    // integration gets its own launcher so they stay independently opt-in.
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
        Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 20.dp, bottom = 8.dp)) {
            Text("RECOVERY", style = MaterialTheme.typography.headlineSmall, color = onBg)
            Text(
                "Let the coach and cardio screen use what your watch and scale already track. " +
                    "All opt-in, all on-device.",
                style = MaterialTheme.typography.bodySmall, color = muted,
                modifier = Modifier.padding(top = 6.dp)
            )
        }

        // ─── Sleep & resting HR — the core recovery signal (finding #2/#3) ─────────────────────
        val (recoveryStatus, recoveryConnected) = when {
            state.loading -> "…" to false
            state.needsUpdate -> "Update" to false
            !state.available -> "Get app" to false
            state.granted -> "On" to true
            else -> "Off" to false
        }
        RecoveryCard(
            title = "Sleep & heart rate",
            statusLabel = recoveryStatus,
            connected = recoveryConnected,
            details = "Health Connect is Android's on-device hub — apps like Samsung Health and your " +
                "watch write to it, and Forge only reads sleep and resting heart rate. Nothing leaves " +
                "your phone; Forge still has no Internet permission. Optional and additive: skip it and " +
                "the coach works exactly as before, and you can disconnect any time from the Health " +
                "Connect app."
        ) {
            CardText("Short sleep or an elevated resting heart rate add to the coach's fatigue score, sharpening its deload call.")
            when {
                state.loading -> {}
                state.needsUpdate -> CardButton("Update Health Connect") { openHealthConnectInStore(context) }
                !state.available -> CardButton("Get Health Connect") { openHealthConnectInStore(context) }
                state.granted -> CardButton("Manage in Health Connect") { openHealthConnectSettings(context) }
                else -> CardButton("Connect Health Connect") { permissionLauncher.launch(viewModel.permissions) }
            }
        }

        // The remaining integrations only mean anything once a Health Connect provider exists.
        if (state.available) {
            // ─── Bodyweight sync (HC-2/HC-3) ──────────────────────────────────────────────────
            RecoveryCard(
                title = "Bodyweight sync",
                statusLabel = if (state.weightGranted) "On" else "Off",
                connected = state.weightGranted,
                details = "Forge reads your latest weight from a smart scale that writes to Health " +
                    "Connect, so your bodyweight trend stays current without typing it. Turn on " +
                    "write-back and your Forge weigh-ins flow back to Health Connect for other apps to see."
            ) {
                CardText("Keep your weight trend current from a smart scale — both directions.")
                if (state.weightGranted) {
                    CardButton("Import latest weight now") { viewModel.importNow() }
                    state.importMessage?.let { CardText(it) }
                    CardToggleRow(
                        label = "Write my weigh-ins to Health Connect",
                        checked = state.writeBodyweight,
                        onCheckedChange = { viewModel.setWriteBodyweight(it) }
                    )
                } else {
                    CardButton("Connect bodyweight") { weightLauncher.launch(viewModel.weightPermissions) }
                }
            }

            // ─── Workout calories (HC-4) ──────────────────────────────────────────────────────
            RecoveryCard(
                title = "Workout calories",
                statusLabel = if (state.calorieGranted) "On" else "Off",
                connected = state.calorieGranted,
                details = "Forge writes each finished session's estimated active calories so your daily " +
                    "energy total includes lifting. It's an estimate from session length, your intensity, " +
                    "and your latest logged bodyweight — Forge has no heart-rate stream."
            ) {
                CardText("Add each session's estimated burn to your daily energy total.")
                if (state.calorieGranted) {
                    CardToggleRow(
                        label = "Write my session calories to Health Connect",
                        checked = state.writeCalories,
                        onCheckedChange = { viewModel.setWriteCalories(it) }
                    )
                } else {
                    CardButton("Connect calories") { calorieLauncher.launch(viewModel.caloriePermissions) }
                }
            }

            // ─── Steps & activity (read a watch/ring's step counts for the cardio screen) ─────
            RecoveryCard(
                title = "Steps & activity",
                statusLabel = if (state.stepsGranted) "On" else "Off",
                connected = state.stepsGranted,
                details = "Connect a watch or ring (via Samsung Health, Fitbit, etc.) that writes steps " +
                    "to Health Connect and Forge shows your steps-through-the-day graph on a cardio " +
                    "session and the current week. Read-only — Forge never writes your steps anywhere."
            ) {
                CardText("Show your steps-through-the-day graph on cardio sessions.")
                if (state.stepsGranted) {
                    CardButton("Manage in Health Connect") { openHealthConnectSettings(context) }
                } else {
                    CardButton("Connect steps") { stepsLauncher.launch(viewModel.stepsPermissions) }
                }
            }

            // ─── GPS routes (read a watch's outdoor sessions to draw their route shape) ────────
            RecoveryCard(
                title = "GPS routes",
                statusLabel = if (state.exerciseGranted) "On" else "Off",
                connected = state.exerciseGranted,
                details = "If your watch records outdoor runs or rides with GPS, Forge draws the route's " +
                    "shape on the matching cardio session. There's no map — just the path, offline. " +
                    "Health Connect asks you to confirm each route the first time Forge draws it."
            ) {
                CardText("Draw your outdoor run or ride's route shape on its session.")
                if (state.exerciseGranted) {
                    CardButton("Manage in Health Connect") { openHealthConnectSettings(context) }
                } else {
                    CardButton("Connect GPS routes") { exerciseLauncher.launch(viewModel.exercisePermissions) }
                }
            }
        }

        Spacer(Modifier.height(16.dp))
    }
}
