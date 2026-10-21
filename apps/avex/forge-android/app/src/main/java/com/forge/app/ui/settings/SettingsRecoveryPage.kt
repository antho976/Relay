package com.forge.app.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.HealthAndSafety
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.health.connect.client.PermissionController
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.health.BodyweightSync
import com.forge.app.domain.health.WearableBrand
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.toggleableLabeled
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenType

/** The nine Health Connect readings, in the page's order: how many are switched on (granted). */
internal fun connectedReadings(s: HealthConnectViewModel.UiState): Int = listOf(
    s.granted && s.available, s.weightGranted, s.bodyFatGranted, s.leanMassGranted,
    s.calorieGranted, s.sessionGranted, s.watchWorkoutGranted, s.stepsGranted, s.exerciseGranted
).count { it }

/** How many readings the page offers. */
internal const val READING_COUNT = 9

/**
 * Settings → Watch & health data (Relay design "sw"). Opens on Health Connect itself (connected or
 * not, and an Open button into it), then each reading the coach and the cardio screen can use,
 * switched on or off, with what it is doing now (receiving, or nothing arriving yet), and once on
 * its own write-back and import options under it. Switching a reading on asks Health Connect for
 * it; switching one off opens Health Connect, which is the only place a grant can be taken back.
 */
@Composable
internal fun RecoveryPage(onBack: () -> Unit, viewModel: HealthConnectViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val context = LocalContext.current

    // "Manage in Health Connect" leaves the app with no result callback, so grants changed there
    // would otherwise not show until a launcher here happened to fire.
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) viewModel.refresh()
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // Health Connect's own permission flow — the result tells us what the user granted. Each
    // integration gets its own launcher so they stay independently opt-in.
    val sleepLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    val weightLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    // The "import older weight" retry (H-05): re-asks for history access alone, then re-runs the
    // backfill whatever the latches say, so a declined-then-granted history still comes over.
    val historyLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.importOlderWeight() }
    val bodyFatLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    val calorieLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    val sessionLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    val watchWorkoutLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    val leanMassLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    val stepsLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    val exerciseLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    val allLauncher = rememberLauncherForActivityResult(PermissionController.createRequestPermissionResultContract()) { viewModel.refresh() }
    val watchAppPaired by viewModel.watchAppPaired.collectAsStateWithLifecycle()

    RecoveryContent(
        state = state,
        watchAppPaired = watchAppPaired,
        a = RecoveryActions(
            connectAll = { allLauncher.launch(viewModel.allPermissions(state.historySupported)) },
            // HRV rides the same grant flow (W6): one sleep-and-heart row, one concept. The
            // connected state still keys on sleep + resting HR alone, so old grants stay valid.
            connectSleep = { sleepLauncher.launch(viewModel.permissions + viewModel.hrvPermissions) },
            // History access rides the first connect (H-05) so the one-time backfill can reach
            // past Health Connect's 30-day window, asked for only where the provider has it.
            connectWeight = {
                weightLauncher.launch(
                    if (state.historySupported) viewModel.weightPermissions + viewModel.historyPermissions
                    else viewModel.weightPermissions
                )
            },
            allowWeightWrite = { weightLauncher.launch(viewModel.weightPermissions) },
            allowHistory = { historyLauncher.launch(viewModel.historyPermissions) },
            connectBodyFat = { bodyFatLauncher.launch(viewModel.bodyFatPermissions) },
            connectLeanMass = { leanMassLauncher.launch(viewModel.leanMassPermissions) },
            connectCalories = { calorieLauncher.launch(viewModel.caloriePermissions) },
            connectSessions = { sessionLauncher.launch(viewModel.sessionWritePermissions) },
            connectWatchWorkouts = { watchWorkoutLauncher.launch(viewModel.watchWorkoutPermissions) },
            connectSteps = { stepsLauncher.launch(viewModel.stepsPermissions) },
            connectRoutes = { exerciseLauncher.launch(viewModel.exercisePermissions) },
            setWriteBodyweight = viewModel::setWriteBodyweight,
            setWriteBodyFat = viewModel::setWriteBodyFat,
            setWriteCalories = viewModel::setWriteCalories,
            setWriteSessions = viewModel::setWriteSessions,
            importWeight = viewModel::importNow,
            importBodyFat = viewModel::importBodyFatNow,
            importLeanMass = viewModel::importLeanMassNow,
            setBrand = viewModel::setWearableBrand,
            manage = { openHealthConnectSettings(context) },
            getHealthConnect = { openHealthConnectInStore(context) },
            getWatchApp = { openAvexInStore(context) }
        ),
        onBack = onBack
    )
}

/** Everything the Watch & health page can do, so the page draws without its view model (goldens). */
internal class RecoveryActions(
    val connectAll: () -> Unit = {},
    val connectSleep: () -> Unit = {},
    val connectWeight: () -> Unit = {},
    val allowWeightWrite: () -> Unit = {},
    val allowHistory: () -> Unit = {},
    val connectBodyFat: () -> Unit = {},
    val connectLeanMass: () -> Unit = {},
    val connectCalories: () -> Unit = {},
    val connectSessions: () -> Unit = {},
    val connectWatchWorkouts: () -> Unit = {},
    val connectSteps: () -> Unit = {},
    val connectRoutes: () -> Unit = {},
    val setWriteBodyweight: (Boolean) -> Unit = {},
    val setWriteBodyFat: (Boolean) -> Unit = {},
    val setWriteCalories: (Boolean) -> Unit = {},
    val setWriteSessions: (Boolean) -> Unit = {},
    val importWeight: () -> Unit = {},
    val importBodyFat: () -> Unit = {},
    val importLeanMass: () -> Unit = {},
    val setBrand: (String) -> Unit = {},
    val manage: () -> Unit = {},
    val getHealthConnect: () -> Unit = {},
    val getWatchApp: () -> Unit = {}
)

/** The page as drawn (Relay design "sw"). */
@Composable
internal fun RecoveryContent(
    state: HealthConnectViewModel.UiState,
    watchAppPaired: Boolean,
    a: RecoveryActions,
    onBack: () -> Unit
) {
    val sleepConnected = state.granted && state.available
    val connectable = state.available && !state.loading
    val connectedCount = connectedReadings(state)
    val brand = WearableBrand.fromKey(state.wearableBrand)

    SettingsScaffold("Watch & health data", onBack) {

        // Health Connect itself: whether it is here and connected, and the way into it. Without a
        // provider nothing below can connect, so installing or updating it is the only way on.
        Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            HealthConnectRow(
                live = connectable && connectedCount > 0,
                reading = when {
                    state.loading -> "Checking"
                    state.needsUpdate -> "Needs an update"
                    !state.available -> "Not on this phone"
                    connectedCount == 0 -> "Installed · not connected"
                    connectedCount == READING_COUNT -> "Connected · everything on"
                    else -> "Connected · $connectedCount of $READING_COUNT on"
                },
                action = when {
                    state.loading -> null
                    !state.available -> if (state.needsUpdate) "Update" else "Get"
                    else -> "Open"
                },
                onAction = { if (state.available) a.manage() else a.getHealthConnect() }
            )
            if (!state.loading && !state.available) {
                Text(
                    "Avex reads your watch and scale through Google's free Health Connect app.",
                    style = OpenType.bodyS,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            if (connectable && connectedCount == 0) {
                Text(
                    "Your watch or ring's own app (Samsung Health, Fitbit, Garmin Connect and others) shares " +
                        "sleep, heart rate, steps and workouts with Health Connect. Avex reads them from there.",
                    style = OpenType.bodyS,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            if (connectable && connectedCount < READING_COUNT) {
                SettingsButton(
                    if (connectedCount == 0) "Connect all" else "Connect the rest",
                    kind = ButtonKind.Primary
                ) { a.connectAll() }
            }
        }

        SettingsGroup(
            "Readings the coach uses",
            headerTrailing = "$connectedCount of $READING_COUNT on"
        ) {
            ReadingRow(
                title = "Sleep & heart rate",
                off = "Short sleep or a high resting heart rate sharpen the deload call",
                connected = sleepConnected,
                connectable = connectable,
                onConnect = a.connectSleep,
                onManage = a.manage,
                receiving = state.signalFlow?.sleepOrHr
            )

            // Read and write are separate grants (M-23). With read alone the row says so and offers
            // the write grant instead of a switch that would imply weigh-ins mirror.
            ReadingRow(
                title = "Bodyweight",
                off = "Keeps your weight trend current from a smart scale",
                connected = state.weightGranted,
                connectable = connectable,
                onConnect = a.connectWeight,
                onManage = a.manage,
                receiving = state.signalFlow?.weight
            )
            if (state.weightGranted) {
                if (state.weightWriteGranted) {
                    SettingsSwitchRow("Write weigh-ins back", "Weigh-ins you log reach Health Connect", checked = state.writeBodyweight, indent = SignalDetailIndent) {
                        a.setWriteBodyweight(it)
                    }
                } else {
                    SignalAction("Write-back is off", "Allow it so weigh-ins you log reach Health Connect", "Allow") {
                        a.allowWeightWrite()
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
                ) { a.importWeight() }
                if (state.historyAffordance == BodyweightSync.HistoryAffordance.RETRY) {
                    SignalAction("Older weight", "Only the last 30 days came over. Older weigh-ins need history access", "Allow") {
                        a.allowHistory()
                    }
                }
            }

            ReadingRow(
                title = "Body fat",
                off = "Pulls body fat % from a smart scale, or log it by hand",
                connected = state.bodyFatGranted,
                connectable = connectable,
                onConnect = a.connectBodyFat,
                onManage = a.manage
            )
            if (state.bodyFatGranted) {
                if (state.bodyFatWriteGranted) {
                    SettingsSwitchRow("Write body fat back", "Body fat you log reaches Health Connect", checked = state.writeBodyFat, indent = SignalDetailIndent) {
                        a.setWriteBodyFat(it)
                    }
                } else {
                    SignalAction("Write-back is off", "Allow it so body fat you log reaches Health Connect", "Allow") {
                        a.connectBodyFat()
                    }
                }
                SignalAction("Latest body fat", state.bodyFatImportMessage ?: "Bring in your newest reading now", "Import") {
                    a.importBodyFat()
                }
            }

            ReadingRow(
                title = "Muscle mass",
                off = "Your watch's body-composition reading, into Profile",
                connected = state.leanMassGranted,
                connectable = connectable,
                onConnect = a.connectLeanMass,
                onManage = a.manage
            )
            if (state.leanMassGranted) {
                SignalAction("Latest muscle mass", state.leanMassImportMessage ?: "Bring in your newest reading now", "Import") {
                    a.importLeanMass()
                }
            }

            ReadingRow(
                title = "Workout calories",
                off = "Adds each session's burn to your daily energy",
                connected = state.calorieGranted,
                connectable = connectable,
                onConnect = a.connectCalories,
                onManage = a.manage
            )
            if (state.calorieGranted) {
                SettingsSwitchRow("Write calories back", "Each session's burn reaches Health Connect", checked = state.writeCalories, indent = SignalDetailIndent) {
                    a.setWriteCalories(it)
                }
            }

            ReadingRow(
                title = "Workout sessions",
                off = "Puts each finished session in Samsung Health or Google Fit",
                connected = state.sessionGranted,
                connectable = connectable,
                onConnect = a.connectSessions,
                onManage = a.manage
            )
            if (state.sessionGranted) {
                SettingsSwitchRow("Write workouts back", "Finished sessions reach Health Connect", checked = state.writeSessions, indent = SignalDetailIndent) {
                    a.setWriteSessions(it)
                }
            }

            ReadingRow(
                title = "Workouts from your watch",
                off = "Heart rate and real stats on watch sessions, offered as imports",
                connected = state.watchWorkoutGranted,
                connectable = connectable,
                onConnect = a.connectWatchWorkouts,
                onManage = a.manage
            )
            ReadingRow(
                title = "Steps & activity",
                off = "Your steps through the day, on cardio sessions",
                connected = state.stepsGranted,
                connectable = connectable,
                onConnect = a.connectSteps,
                onManage = a.manage,
                receiving = state.signalFlow?.steps
            )
            ReadingRow(
                title = "GPS routes",
                // The known per-brand gap lives on its signal: Samsung Health sends routes on
                // recent versions; Fitbit versions vary.
                off = when (brand) {
                    WearableBrand.GALAXY -> "Your outdoor route. Needs a recent Samsung Health"
                    WearableBrand.PIXEL -> "Your outdoor route. Older Fitbit versions may not send it"
                    else -> "Your outdoor run or ride's route on its session"
                },
                connected = state.exerciseGranted,
                connectable = connectable,
                onConnect = a.connectRoutes,
                onManage = a.manage,
                receiving = state.signalFlow?.route
            )
        }

        // The Wear OS app had no mention anywhere on the phone, so the only way to learn it existed
        // was the Play Store listing.
        SettingsGroup(
            "Avex on your watch",
            footer = if (watchAppPaired) null
            else "On a Wear OS watch, install Avex from the Play Store on the watch, or from its listing on this phone."
        ) {
            if (watchAppPaired) {
                SettingsInfoRow("Watch app paired", "Start on either device and the other follows")
            } else {
                SettingsNavigationRow(
                    "Get the watch app",
                    "Sets, rest timer and live heart rate on your wrist",
                    external = true
                ) { a.getWatchApp() }
            }
        }

        // Advisory only: every read works for any wearable. The pick names the companion app.
        SettingsGroup(
            "What you wear",
            footer = when (brand) {
                WearableBrand.GALAXY -> "Syncs through Samsung Health. Turn on its Health Connect sharing first."
                WearableBrand.PIXEL -> "Syncs through the Fitbit app. Turn on its Health Connect sharing first."
                WearableBrand.NONE -> "Any watch or ring that feeds Health Connect works."
                null -> "Nothing arriving? Pick what you wear for the step most people miss."
            }
        ) {
            val brands = WearableBrand.entries
            SettingsSegmented(
                options = brands.map { brandShortLabel(it) },
                selectedIndex = brands.indexOfFirst { it.key == state.wearableBrand },
                onSelect = { a.setBrand(brands[it].key) },
                contentDescription = "Your device"
            )
        }

        if (!state.loading && state.available) {
            SettingsGroup {
                SettingsNavigationRow(
                    "Manage in Health Connect",
                    "Review or take back what Avex can read and write",
                    Icons.Rounded.HealthAndSafety,
                    external = true
                ) { a.manage() }
            }
        }

        OpenLabel("Everything stays on this phone · Avex has no internet permission", style = OpenType.monoXS)
    }
}

/** Health Connect's own row: a dot (red while readings flow through it), its state, and one button. */
@Composable
private fun HealthConnectRow(live: Boolean, reading: String, action: String?, onAction: () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(
            Modifier
                .size(10.dp)
                .clip(CircleShape)
                .then(
                    if (live) Modifier.background(MaterialTheme.colorScheme.primary)
                    else Modifier.border(1.5.dp, OpenColors.Rule, CircleShape)
                )
        )
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text("Health Connect", style = OpenType.rowTitleL, color = MaterialTheme.colorScheme.onBackground)
            SettingsReading(reading)
        }
        if (action != null) SettingsCompactButton(action, onClick = onAction)
    }
}

/**
 * One reading, switched on or off. On is Health Connect's grant: its line says what is arriving
 * (receiving, or nothing yet, which usually means the companion app's sharing is off). Off says what
 * it would feed. Tapping an off reading asks for it; tapping an on one opens Health Connect, where
 * grants are taken back. Without a provider the row is inert.
 */
@Composable
private fun ReadingRow(
    title: String,
    off: String,
    connected: Boolean,
    connectable: Boolean,
    onConnect: () -> Unit,
    onManage: () -> Unit,
    receiving: Boolean? = null
) {
    val reading = when {
        !connected -> "Off · $off"
        receiving == true -> "On · receiving"
        receiving == false -> "On · nothing arriving yet"
        else -> "On"
    }
    val enabled = connectable
    val alpha = if (enabled) 1f else 0.35f
    SettingsRowContainer(
        interaction = if (enabled) Modifier.toggleableLabeled(title, connected) { if (connected) onManage() else onConnect() } else Modifier
    ) {
        SettingsRowText(
            title,
            reading,
            titleColor = MaterialTheme.colorScheme.onBackground.copy(alpha = alpha),
            supportingColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = alpha)
        )
        SettingsSwitch(connected)
    }
}
