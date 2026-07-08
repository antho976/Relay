package com.forge.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
 * Settings → Recovery. Connects Health Connect so the coach and cardio screen can read what your
 * watch and scale already track. Leads with the connection rail (one dot per integration, §12),
 * then one SIGNALS list of five quiet rows — each row's right side is its state (`• ON`) or the
 * mono accent `connect →` that the whole row taps. The only capsule on the page is the page-end
 * Get/Update Health Connect action when no usable provider exists; managing or revoking access
 * lives in one link at the foot.
 */
@Composable
internal fun RecoveryPage(modifier: Modifier = Modifier, viewModel: HealthConnectViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    // Health Connect's own permission flow — the result tells us what the user granted. Each
    // integration gets its own launcher so they stay independently opt-in.
    val sleepLauncher = rememberLauncherForActivityResult(
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

    val sleepConnected = state.granted && state.available
    // Row order = rail order: sleep · bodyweight · calories · steps · routes.
    val railStates = listOf(
        sleepConnected, state.weightGranted, state.calorieGranted, state.stepsGranted, state.exerciseGranted
    )
    val connectable = state.available && !state.loading

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 40.dp)
    ) {
        // The mark first (§12): the page's whole state in one glance, honest at zero.
        Spacer(Modifier.height(18.dp))
        RecoveryConnectionRail(railStates)
        Spacer(Modifier.height(8.dp))
        Text(
            "Opt-in and on-device. Avex has no Internet permission.",
            style = MaterialTheme.typography.bodySmall, color = muted,
            modifier = Modifier.padding(horizontal = 24.dp)
        )

        // One list, one rhythm — each explainer names what its signal feeds, so no per-feature headers.
        SettingsSectionHeader("SIGNALS")
        Spacer(Modifier.height(4.dp))
        RecoveryRow(
            title = "Sleep & heart rate",
            explainer = "Short sleep or a high resting heart rate sharpen the deload call.",
            connected = sleepConnected,
            connectable = connectable,
            onConnect = { sleepLauncher.launch(viewModel.permissions) }
        )
        RecoveryRow(
            title = "Bodyweight sync",
            explainer = "Keeps your weight trend current from a smart scale, both ways.",
            connected = state.weightGranted,
            connectable = connectable,
            onConnect = { weightLauncher.launch(viewModel.weightPermissions) }
        )
        if (state.weightGranted) {
            RecoveryToggleRow(
                label = "Write my weigh-ins to Health Connect",
                checked = state.writeBodyweight,
                onCheckedChange = { viewModel.setWriteBodyweight(it) }
            )
            SettingsActionLink("Import latest weight →") { viewModel.importNow() }
            state.importMessage?.let {
                Text(
                    it, style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
            }
        }
        RecoveryRow(
            title = "Workout calories",
            explainer = "Adds each session's estimated burn to your daily energy total.",
            connected = state.calorieGranted,
            connectable = connectable,
            onConnect = { calorieLauncher.launch(viewModel.caloriePermissions) }
        )
        if (state.calorieGranted) {
            RecoveryToggleRow(
                label = "Write my session calories to Health Connect",
                checked = state.writeCalories,
                onCheckedChange = { viewModel.setWriteCalories(it) }
            )
        }
        RecoveryRow(
            title = "Steps & activity",
            explainer = "Shows your steps through the day on cardio sessions.",
            connected = state.stepsGranted,
            connectable = connectable,
            onConnect = { stepsLauncher.launch(viewModel.stepsPermissions) }
        )
        RecoveryRow(
            title = "GPS routes",
            explainer = "Draws your outdoor run or ride's route on its session.",
            connected = state.exerciseGranted,
            connectable = connectable,
            onConnect = { exerciseLauncher.launch(viewModel.exercisePermissions) }
        )

        // Page-end actions (§3/§8): manage-access link when a provider exists, otherwise the one
        // capsule on the page — installing or updating the provider is the only way forward.
        Spacer(Modifier.height(20.dp))
        when {
            state.loading -> Unit
            state.available -> Row(Modifier.padding(horizontal = 24.dp)) {
                SettingsOutlineAction("Manage in Health Connect") { openHealthConnectSettings(context) }
            }
            else -> {
                Text(
                    if (state.needsUpdate) "Your Health Connect app needs an update first."
                    else "Connecting needs the free Health Connect app.",
                    style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
                Spacer(Modifier.height(10.dp))
                Row(Modifier.padding(horizontal = 24.dp)) {
                    SettingsPrimaryAction(
                        if (state.needsUpdate) "Update Health Connect" else "Get Health Connect"
                    ) { openHealthConnectInStore(context) }
                }
            }
        }

        Spacer(Modifier.height(24.dp))
    }
}
