package com.forge.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.DirectionsWalk
import androidx.compose.material.icons.rounded.Bedtime
import androidx.compose.material.icons.rounded.FitnessCenter
import androidx.compose.material.icons.rounded.HealthAndSafety
import androidx.compose.material.icons.rounded.LocalFireDepartment
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material.icons.rounded.MonitorWeight
import androidx.compose.material.icons.rounded.SyncAlt
import androidx.compose.material.icons.rounded.Watch
import androidx.compose.material.icons.rounded.WaterDrop
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.health.connect.client.PermissionController
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.health.BodyweightSync
import com.forge.app.domain.health.WearableBrand

/**
 * Settings → Wearable. Connects Health Connect so the coach and cardio screen can read what your
 * watch and scale already track. Opens on how many signals are connected, then the device you wear
 * (which companion app has to share with Health Connect first), then one row per signal with its
 * state on the right and, once connected, its own write-back and import options indented under it.
 */
@Composable
internal fun RecoveryPage(onBack: () -> Unit, viewModel: HealthConnectViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val context = LocalContext.current

    // Health Connect's own permission flow — the result tells us what the user granted. Each
    // integration gets its own launcher so they stay independently opt-in.
    val sleepLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val weightLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    // The "import older weight" retry (H-05): re-asks for history access alone, then re-runs the
    // backfill whatever the latches say, so a declined-then-granted history still comes over.
    val historyLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.importOlderWeight() }
    val bodyFatLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val calorieLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val sessionLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val watchWorkoutLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val leanMassLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val stepsLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }
    val exerciseLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }

    val sleepConnected = state.granted && state.available
    // Row order = rail order: sleep · bodyweight · body fat · muscle mass · calories · sessions ·
    // watch workouts · steps · routes.
    val railStates = listOf(
        sleepConnected, state.weightGranted, state.bodyFatGranted, state.leanMassGranted,
        state.calorieGranted, state.sessionGranted, state.watchWorkoutGranted,
        state.stepsGranted, state.exerciseGranted
    )
    val connectable = state.available && !state.loading
    val brand = WearableBrand.fromKey(state.wearableBrand)

    SettingsScaffold("Wearable", onBack) {

        // Without a provider nothing below can connect; installing or updating it is the only way on.
        if (!state.loading && !state.available) {
            SettingsNotice(
                icon = Icons.Rounded.HealthAndSafety,
                title = if (state.needsUpdate) "Health Connect needs an update" else "Health Connect isn't on this phone",
                body = "Avex reads your watch and scale through Google's free Health Connect app.",
                action = if (state.needsUpdate) "Update Health Connect" else "Get Health Connect",
                critical = false
            ) { openHealthConnectInStore(context) }
        }

        // Advisory only: every read works for any wearable. The pick names the companion app.
        SettingsGroup(
            "Your device",
            footer = when (brand) {
                WearableBrand.GALAXY -> "Syncs through Samsung Health. Turn on its Health Connect sharing first."
                WearableBrand.PIXEL -> "Syncs through the Fitbit app. Turn on its Health Connect sharing first."
                WearableBrand.NONE -> "Any watch or ring that feeds Health Connect works."
                null -> "Pick what you wear and Avex tailors the setup notes to it."
            }
        ) {
            SettingsGroupBlock(padding = PaddingValues(12.dp)) {
                val brands = WearableBrand.entries
                SettingsSegmented(
                    options = brands.map { brandShortLabel(it) },
                    selectedIndex = brands.indexOfFirst { it.key == state.wearableBrand },
                    onSelect = { viewModel.setWearableBrand(brands[it].key) },
                    contentDescription = "Your device"
                )
            }
        }

        SettingsGroup(
            "Signals",
            headerTrailing = "${railStates.count { it }} of ${railStates.size} connected",
            footer = "Read through Health Connect, on this phone. Avex has no internet permission."
        ) {
            RecoveryRow(
                title = "Sleep & heart rate",
                explainer = "Short sleep or a high resting heart rate sharpen the deload call",
                icon = Icons.Rounded.Bedtime,
                connected = sleepConnected,
                connectable = connectable,
                // HRV rides the same grant flow (W6): one sleep-and-heart row, one concept. The
                // connected state still keys on sleep + resting HR alone, so old grants stay valid.
                onConnect = { sleepLauncher.launch(viewModel.permissions + viewModel.hrvPermissions) },
                receiving = state.signalFlow?.sleepOrHr
            )

            // Read and write are separate grants (M-23). With read alone the row says so and offers
            // the write grant instead of a switch that would imply weigh-ins mirror.
            val weightReadOnly = state.weightGranted && !state.weightWriteGranted
            RecoveryRow(
                title = "Bodyweight",
                explainer = if (weightReadOnly) "Reads your weight from a smart scale. Weigh-ins stay in Avex until you allow write-back"
                            else "Keeps your weight trend current from a smart scale, both ways",
                icon = Icons.Rounded.MonitorWeight,
                connected = state.weightGranted,
                connectable = connectable,
                // History access rides the first connect (H-05) so the one-time backfill can reach
                // past Health Connect's 30-day window, asked for only where the provider has it.
                onConnect = {
                    weightLauncher.launch(
                        if (state.historySupported) viewModel.weightPermissions + viewModel.historyPermissions
                        else viewModel.weightPermissions
                    )
                },
                receiving = state.signalFlow?.weight
            )
            if (state.weightGranted) {
                if (state.weightWriteGranted) {
                    SettingsSwitchRow("Write my weigh-ins to Health Connect", checked = state.writeBodyweight, indent = SignalDetailIndent) {
                        viewModel.setWriteBodyweight(it)
                    }
                } else {
                    SignalAction("Write-back is off", "Allow it so weigh-ins you log reach Health Connect", "Allow") {
                        weightLauncher.launch(viewModel.weightPermissions)
                    }
                }
                // A grant not given yet comes with the action that gives it; a provider with no
                // extended history says so on the import itself, because there is nothing to tap.
                SignalAction(
                    "Latest weight",
                    state.importMessage ?: when (state.historyAffordance) {
                        BodyweightSync.HistoryAffordance.UNSUPPORTED -> "This Health Connect app only shares the last 30 days"
                        else -> "Bring in your newest weigh-in now"
                    },
                    "Import"
                ) { viewModel.importNow() }
                if (state.historyAffordance == BodyweightSync.HistoryAffordance.RETRY) {
                    SignalAction("Older weight", "Only the last 30 days came over. Older weigh-ins need history access", "Allow") {
                        historyLauncher.launch(viewModel.historyPermissions)
                    }
                }
            }

            val bodyFatReadOnly = state.bodyFatGranted && !state.bodyFatWriteGranted
            RecoveryRow(
                title = "Body fat",
                explainer = if (bodyFatReadOnly) "Pulls body fat % from a smart scale. Entries stay in Avex until you allow write-back"
                            else "Pulls body fat % from a smart scale, and writes yours back",
                icon = Icons.Rounded.WaterDrop,
                connected = state.bodyFatGranted,
                connectable = connectable,
                onConnect = { bodyFatLauncher.launch(viewModel.bodyFatPermissions) }
            )
            if (state.bodyFatGranted) {
                if (state.bodyFatWriteGranted) {
                    SettingsSwitchRow("Write my body fat to Health Connect", checked = state.writeBodyFat, indent = SignalDetailIndent) {
                        viewModel.setWriteBodyFat(it)
                    }
                } else {
                    SignalAction("Write-back is off", "Allow it so body fat you log reaches Health Connect", "Allow") {
                        bodyFatLauncher.launch(viewModel.bodyFatPermissions)
                    }
                }
                SignalAction("Latest body fat", state.bodyFatImportMessage ?: "Bring in your newest reading now", "Import") {
                    viewModel.importBodyFatNow()
                }
            }

            RecoveryRow(
                title = "Muscle mass",
                explainer = "Pulls your watch's body-composition muscle reading into Profile",
                icon = Icons.Rounded.FitnessCenter,
                connected = state.leanMassGranted,
                connectable = connectable,
                onConnect = { leanMassLauncher.launch(viewModel.leanMassPermissions) }
            )
            if (state.leanMassGranted) {
                SignalAction("Latest muscle mass", state.leanMassImportMessage ?: "Bring in your newest reading now", "Import") {
                    viewModel.importLeanMassNow()
                }
            }

            RecoveryRow(
                title = "Workout calories",
                explainer = "Adds each session's estimated burn to your daily energy total",
                icon = Icons.Rounded.LocalFireDepartment,
                connected = state.calorieGranted,
                connectable = connectable,
                onConnect = { calorieLauncher.launch(viewModel.caloriePermissions) }
            )
            if (state.calorieGranted) {
                SettingsSwitchRow("Write my session calories to Health Connect", checked = state.writeCalories, indent = SignalDetailIndent) {
                    viewModel.setWriteCalories(it)
                }
            }

            RecoveryRow(
                title = "Workout sessions",
                explainer = "Puts each finished session in Samsung Health or Google Fit, as itself",
                icon = Icons.Rounded.SyncAlt,
                connected = state.sessionGranted,
                connectable = connectable,
                onConnect = { sessionLauncher.launch(viewModel.sessionWritePermissions) }
            )
            if (state.sessionGranted) {
                SettingsSwitchRow("Write my workouts to Health Connect", checked = state.writeSessions, indent = SignalDetailIndent) {
                    viewModel.setWriteSessions(it)
                }
            }

            RecoveryRow(
                title = "Watch workouts",
                explainer = "Shows heart rate and real stats on watch-recorded sessions, and offers imports",
                icon = Icons.Rounded.Watch,
                connected = state.watchWorkoutGranted,
                connectable = connectable,
                onConnect = { watchWorkoutLauncher.launch(viewModel.watchWorkoutPermissions) }
            )
            RecoveryRow(
                title = "Steps & activity",
                explainer = "Shows your steps through the day on cardio sessions",
                icon = Icons.AutoMirrored.Rounded.DirectionsWalk,
                connected = state.stepsGranted,
                connectable = connectable,
                onConnect = { stepsLauncher.launch(viewModel.stepsPermissions) },
                receiving = state.signalFlow?.steps
            )
            RecoveryRow(
                title = "GPS routes",
                // The known per-brand gap lives on its signal: Samsung Health sends routes on
                // recent versions; Fitbit versions vary.
                explainer = when (brand) {
                    WearableBrand.GALAXY -> "Draws your outdoor route. Needs a recent Samsung Health to send routes"
                    WearableBrand.PIXEL -> "Draws your outdoor route. Older Fitbit versions may not send routes"
                    else -> "Draws your outdoor run or ride's route on its session"
                },
                icon = Icons.Rounded.Map,
                connected = state.exerciseGranted,
                connectable = connectable,
                onConnect = { exerciseLauncher.launch(viewModel.exercisePermissions) },
                receiving = state.signalFlow?.route
            )
        }

        if (!state.loading && state.available) {
            SettingsGroup {
                SettingsNavigationRow(
                    "Manage in Health Connect",
                    "Review or revoke what Avex can read and write",
                    Icons.Rounded.HealthAndSafety,
                    external = true
                ) { openHealthConnectSettings(context) }
            }
        }
    }
}
