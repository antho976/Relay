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
import com.forge.app.domain.health.WearableBrand

/**
 * Settings → Recovery. Connects Health Connect so the coach and cardio screen can read what your
 * watch and scale already track. Leads with the connection rail (one dot per integration, §12),
 * then a WEARABLE brand pick (Galaxy · Pixel · other) that tailors the setup pointers — each
 * brand's watch feeds Health Connect through its own companion app (Samsung Health / Fitbit), and
 * that app's sharing must be on before any grant here delivers data — then one SIGNALS list of
 * five quiet rows, each row's right side its state (`• ON`) or the connect pill the whole row
 * taps. The only capsule on the page is the page-end Get/Update Health Connect action when no
 * usable provider exists; managing or revoking access lives in one link at the foot.
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
    val bodyFatLauncher = rememberLauncherForActivityResult(
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
    // Row order = rail order: sleep · bodyweight · body fat · calories · steps · routes.
    val railStates = listOf(
        sleepConnected, state.weightGranted, state.bodyFatGranted,
        state.calorieGranted, state.stepsGranted, state.exerciseGranted
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

        // The brand pick tailors the pointers below (which companion app feeds Health Connect, and
        // which signals vary by its version). Advisory only — every read works for any wearable.
        SettingsSectionHeader("WEARABLE")
        Spacer(Modifier.height(4.dp))
        ChipFlow {
            WearableBrand.entries.forEach { brand ->
                PillChip(brand.label, state.wearableBrand == brand.key) {
                    viewModel.setWearableBrand(brand.key)
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(
            when (WearableBrand.fromKey(state.wearableBrand)) {
                WearableBrand.GALAXY -> "Syncs through Samsung Health. Turn on its Health Connect sharing first."
                WearableBrand.PIXEL -> "Syncs through the Fitbit app. Turn on its Health Connect sharing first."
                WearableBrand.NONE -> "Any watch or ring that feeds Health Connect works."
                null -> "Pick what you wear. Avex tailors the setup pointers to it."
            },
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
            onConnect = { sleepLauncher.launch(viewModel.permissions) },
            receiving = state.signalFlow?.sleepOrHr
        )
        RecoveryRow(
            title = "Bodyweight sync",
            explainer = "Keeps your weight trend current from a smart scale, both ways.",
            connected = state.weightGranted,
            connectable = connectable,
            onConnect = { weightLauncher.launch(viewModel.weightPermissions) },
            receiving = state.signalFlow?.weight
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
            title = "Body fat sync",
            explainer = "Pulls body fat % from a smart scale, and writes yours back both ways.",
            connected = state.bodyFatGranted,
            connectable = connectable,
            onConnect = { bodyFatLauncher.launch(viewModel.bodyFatPermissions) }
        )
        if (state.bodyFatGranted) {
            RecoveryToggleRow(
                label = "Write my body fat to Health Connect",
                checked = state.writeBodyFat,
                onCheckedChange = { viewModel.setWriteBodyFat(it) }
            )
            SettingsActionLink("Import latest body fat →") { viewModel.importBodyFatNow() }
            state.bodyFatImportMessage?.let {
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
            onConnect = { stepsLauncher.launch(viewModel.stepsPermissions) },
            receiving = state.signalFlow?.steps
        )
        RecoveryRow(
            title = "GPS routes",
            // The known per-brand gap lives here, on its signal: Samsung Health sends routes on
            // recent versions; Fitbit versions vary. Facts mirror the onboarding wearable step.
            explainer = when (WearableBrand.fromKey(state.wearableBrand)) {
                WearableBrand.GALAXY -> "Draws your outdoor route. Needs a recent Samsung Health to send routes."
                WearableBrand.PIXEL -> "Draws your outdoor route. Older Fitbit versions may not send routes."
                else -> "Draws your outdoor run or ride's route on its session."
            },
            connected = state.exerciseGranted,
            connectable = connectable,
            onConnect = { exerciseLauncher.launch(viewModel.exercisePermissions) },
            receiving = state.signalFlow?.route
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
