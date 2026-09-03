package com.forge.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.health.connect.client.PermissionController
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.health.BodyweightSync
import com.forge.app.domain.health.WearableBrand

/**
 * Settings → Wearable. Connects Health Connect so the coach and cardio screen can read what your
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

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 40.dp)
    ) {
        // The top bar never names the screen (§4.6), so the page names itself with its own mono
        // anchor before anything else — this page used to open on a bare dot rail with no title.
        SettingsSectionHeader("Wearable", top = 12.dp)
        // Then the mark (§12): the page's whole state in one glance, honest at zero.
        RecoveryConnectionRail(railStates)
        Spacer(Modifier.height(8.dp))
        Text(
            "Opt-in and on-device. Avex has no Internet permission.",
            style = MaterialTheme.typography.bodySmall, color = muted,
            modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
        )

        // The brand pick tailors the pointers below (which companion app feeds Health Connect, and
        // which signals vary by its version). Advisory only — every read works for any wearable.
        SettingsSectionHeader("Your device")
        Spacer(Modifier.height(4.dp))
        Row(
            modifier = Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            WearableBrand.entries.forEach { brand ->
                PillChip(
                    brand.label,
                    state.wearableBrand == brand.key,
                    modifier = Modifier.weight(1f)
                ) {
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
            modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
        )

        // One list, one rhythm — each explainer names what its signal feeds, so no per-feature headers.
        SettingsSectionHeader("Signals")
        Spacer(Modifier.height(4.dp))
        RecoveryRow(
            title = "Sleep & heart rate",
            explainer = "Short sleep or a high resting heart rate sharpen the deload call.",
            connected = sleepConnected,
            connectable = connectable,
            // HRV rides the same grant flow (W6): one sleep-and-heart row, one concept. The row's
            // connected state still keys on sleep + resting HR alone, so existing grants stay valid.
            onConnect = { sleepLauncher.launch(viewModel.permissions + viewModel.hrvPermissions) },
            receiving = state.signalFlow?.sleepOrHr
        )
        // Read and write are separate grants (M-23). With read alone the row says so and offers the
        // write grant instead of a toggle that would imply weigh-ins mirror while the write path
        // silently skips. The explainer is the one home for that fact; the link is only the action.
        val weightReadOnly = state.weightGranted && !state.weightWriteGranted
        RecoveryRow(
            title = "Bodyweight sync",
            explainer = if (weightReadOnly) "Reads your weight from a smart scale. Weigh-ins stay in Avex until you allow write-back."
                        else "Keeps your weight trend current from a smart scale, both ways.",
            connected = state.weightGranted,
            connectable = connectable,
            // History access rides the first connect (H-05) so the one-time backfill can reach past
            // Health Connect's 30-day window; declining it still connects the row. Asked for only
            // where the provider implements it — an unsupported permission is a consent screen the
            // user cannot say yes to, so requesting it can only make the connect look like it failed.
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
                RecoveryToggleRow(
                    label = "Write my weigh-ins to Health Connect",
                    checked = state.writeBodyweight,
                    onCheckedChange = { viewModel.setWriteBodyweight(it) }
                )
            } else {
                SettingsActionLink("Allow write-back →") { weightLauncher.launch(viewModel.weightPermissions) }
            }
            SettingsActionLink("Import latest weight →") { viewModel.importNow() }
            // Two different facts, two different lines. A grant the user has not given yet comes
            // with the action that gives it; a provider that has no extended history at all comes
            // with none, because there is nothing to tap. Offering the retry regardless is what left
            // an unsupported provider with a permanent link that could never do anything.
            when (state.historyAffordance) {
                BodyweightSync.HistoryAffordance.RETRY -> {
                    Text(
                        "Only the last 30 days came over. Older weigh-ins need history access.",
                        style = MaterialTheme.typography.bodySmall, color = muted,
                        modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
                    )
                    SettingsActionLink("Import older weight →") { historyLauncher.launch(viewModel.historyPermissions) }
                }
                BodyweightSync.HistoryAffordance.UNSUPPORTED -> Text(
                    "This Health Connect app only shares the last 30 days of weight.",
                    style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
                )
                BodyweightSync.HistoryAffordance.NONE -> Unit
            }
            state.importMessage?.let {
                Text(
                    it, style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
                )
            }
        }
        val bodyFatReadOnly = state.bodyFatGranted && !state.bodyFatWriteGranted
        RecoveryRow(
            title = "Body fat sync",
            explainer = if (bodyFatReadOnly) "Pulls body fat % from a smart scale. Entries stay in Avex until you allow write-back."
                        else "Pulls body fat % from a smart scale, and writes yours back both ways.",
            connected = state.bodyFatGranted,
            connectable = connectable,
            onConnect = { bodyFatLauncher.launch(viewModel.bodyFatPermissions) }
        )
        if (state.bodyFatGranted) {
            if (state.bodyFatWriteGranted) {
                RecoveryToggleRow(
                    label = "Write my body fat to Health Connect",
                    checked = state.writeBodyFat,
                    onCheckedChange = { viewModel.setWriteBodyFat(it) }
                )
            } else {
                SettingsActionLink("Allow write-back →") { bodyFatLauncher.launch(viewModel.bodyFatPermissions) }
            }
            SettingsActionLink("Import latest body fat →") { viewModel.importBodyFatNow() }
            state.bodyFatImportMessage?.let {
                Text(
                    it, style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
                )
            }
        }
        RecoveryRow(
            title = "Muscle mass sync",
            explainer = "Pulls your watch's body-composition muscle reading into Profile.",
            connected = state.leanMassGranted,
            connectable = connectable,
            onConnect = { leanMassLauncher.launch(viewModel.leanMassPermissions) }
        )
        if (state.leanMassGranted) {
            SettingsActionLink("Import latest muscle mass →") { viewModel.importLeanMassNow() }
            state.leanMassImportMessage?.let {
                Text(
                    it, style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
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
            title = "Workout sessions",
            explainer = "Puts each finished session in Samsung Health or Google Fit, as itself.",
            connected = state.sessionGranted,
            connectable = connectable,
            onConnect = { sessionLauncher.launch(viewModel.sessionWritePermissions) }
        )
        if (state.sessionGranted) {
            RecoveryToggleRow(
                label = "Write my workouts to Health Connect",
                checked = state.writeSessions,
                onCheckedChange = { viewModel.setWriteSessions(it) }
            )
        }
        RecoveryRow(
            title = "Watch workouts",
            explainer = "Shows heart rate and real stats on watch-recorded sessions, and offers imports.",
            connected = state.watchWorkoutGranted,
            connectable = connectable,
            onConnect = { watchWorkoutLauncher.launch(viewModel.watchWorkoutPermissions) }
        )
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
            state.available -> SettingsActionRow {
                SettingsOutlineAction("Manage in Health Connect") { openHealthConnectSettings(context) }
            }
            else -> {
                Text(
                    if (state.needsUpdate) "Your Health Connect app needs an update first."
                    else "Connecting needs the free Health Connect app.",
                    style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
                )
                Spacer(Modifier.height(10.dp))
                SettingsActionRow {
                    SettingsPrimaryAction(
                        if (state.needsUpdate) "Update Health Connect" else "Get Health Connect"
                    ) { openHealthConnectInStore(context) }
                }
            }
        }

        Spacer(Modifier.height(24.dp))
    }
}
