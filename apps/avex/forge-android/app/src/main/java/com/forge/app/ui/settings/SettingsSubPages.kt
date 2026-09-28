@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import android.content.Intent
import android.provider.Settings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.NotificationsOff
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material3.Icon
import androidx.compose.material3.InputChip
import androidx.compose.material3.InputChipDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.forge.app.data.prefs.SettingsSection

@Composable
internal fun AppearancePage(state: SettingsUiState, vm: SettingsViewModel, onBack: () -> Unit) {
    var showAppIconSheet by remember { mutableStateOf(false) }
    SettingsScaffold("Appearance", onBack) {
        ThemePreview()

        SettingsGroup("Theme") {
            SettingsSwitchRow(
                "Pure black",
                "True-black backgrounds. Saves battery on OLED screens.",
                state.amoledMode,
                onCheckedChange = vm::setAmoledMode
            )
        }

        SettingsGroup("Home") {
            SettingsSwitchRow(
                "Goals on Home",
                "Your pinned goals under the week strip. They stay in Goals either way.",
                com.forge.app.ui.overview.HOME_GOALS_TILE !in state.hiddenOverviewTiles,
                onCheckedChange = { vm.setTileHidden(com.forge.app.ui.overview.HOME_GOALS_TILE, !it) }
            )
        }

        val icon = com.forge.app.appicon.AppIcon.fromKey(state.appIconKey)
        // The icon is steering the accent right now: on, and the icon has a colour of its own.
        val iconAccent = state.accentFromIcon && icon.accentHex != null
        SettingsGroup(
            "Accent color",
            headerTrailing = when {
                !state.accentEnabled -> null
                iconAccent -> "From icon"
                else -> accentName(state.accentColorHex)
            }
        ) {
            SettingsSwitchRow(
                "Use an accent color",
                "Off keeps the app black and white.",
                state.accentEnabled,
                onCheckedChange = vm::setAccentEnabled
            )
            // The picker only means something while the accent is on and is not being taken from
            // the icon; the chosen colour is kept either way and comes back when it applies again.
            if (state.accentEnabled && !iconAccent) AccentColorPicker(state.accentColorHex, vm::setAccentColorHex)
        }

        SettingsGroup("Icon & startup") {
            AppIconRow(state.appIconKey) { showAppIconSheet = true }
            SettingsSwitchRow(
                "Icon-themed startup",
                "Plays the launch animation in your icon's style. Off shows the plain wordmark.",
                state.themedLaunchIntro,
                onCheckedChange = vm::setThemedLaunchIntro
            )
            SettingsSwitchRow(
                "Match accent to icon",
                when {
                    !state.accentEnabled -> "Turn on the accent color above to use this."
                    icon.accentHex == null -> "${icon.displayName} has no color of its own, so your picked accent stays."
                    else -> "Buttons and highlights take your icon's color. Off goes back to the accent you picked."
                },
                state.accentFromIcon,
                // Nothing to steer while the app is monochrome; the choice is kept for later.
                enabled = state.accentEnabled,
                onCheckedChange = vm::setAccentFromIcon
            )
        }

        SettingsResetGroup(SettingsSection.APPEARANCE, vm)
    }

    if (showAppIconSheet) {
        AppIconPickerSheet(
            selectedKey = state.appIconKey,
            onSelect = { vm.setAppIcon(it); showAppIconSheet = false },
            onDismiss = { showAppIconSheet = false },
        )
    }
}

@Composable
internal fun FormatPage(state: SettingsUiState, vm: SettingsViewModel, onBack: () -> Unit) {
    SettingsScaffold("Units & format", onBack) {
        // The preview IS the page's summary: it re-renders the moment any unit below flips.
        FormatPreview(state)

        SettingsGroup("Units") {
            val weightUnits = listOf("lb", "kg", "st")
            SettingsSegmentedRow("Weight", weightUnits, weightUnits.indexOf(state.weightUnit.label).coerceAtLeast(0)) {
                vm.setWeightUnit(com.forge.app.domain.units.WeightUnit.fromKey(weightUnits[it]))
            }
            SettingsSegmentedRow("Distance", listOf("km", "mi"), if (state.useMiles) 1 else 0) { vm.setUseMiles(it == 1) }
            SettingsSegmentedRow("Length", listOf("cm", "in"), if (state.useCm) 0 else 1) { vm.setUseCm(it == 0) }
        }

        // Only the controls the app honours (2026-09-26 audit, "Settings that do nothing"). Date
        // format and Timezone were saved but read by nothing; their pref keys stay so a restore and
        // a section reset still round-trip.
        SettingsGroup("Time") {
            SettingsSegmentedRow("Clock", listOf("12h", "24h"), if (state.timeFormat24h) 1 else 0) { vm.setTimeFormat24h(it == 1) }
            SettingsSegmentedRow(
                "Week starts",
                listOf("Mon", "Sun"),
                if (state.firstDayMonday) 0 else 1,
                supporting = "Orders Home's week and what counts as this week"
            ) { vm.setFirstDayMonday(it == 0) }
        }

        SettingsGroup("Strength standards", footer = "Scales only the bodyweight standards on Stats.") {
            val sexes = listOf("male", "female")
            SettingsSegmentedRow("Sex", listOf("Male", "Female"), sexes.indexOf(state.userSex).coerceAtLeast(0)) {
                vm.setUserSex(sexes[it])
            }
        }

        SettingsResetGroup(SettingsSection.FORMAT, vm)
    }
}

private val COMPOUND_REST = listOf(120, 150, 180, 210, 240, 300)
private val ISOLATION_REST = listOf(45, 60, 90, 120, 150)

@Composable
internal fun SessionPage(state: SettingsUiState, vm: SettingsViewModel, onBack: () -> Unit) {
    SettingsScaffold("Session", onBack) {
        SettingsGroup("While you log") {
            val haptics = listOf("off", "light", "medium", "strong")
            SettingsSegmentedRow(
                "Haptic feedback",
                listOf("Off", "Light", "Medium", "Strong"),
                haptics.indexOf(state.hapticStrength).coerceAtLeast(0),
                supporting = "Set logged, PR hit, rest over",
                stacked = true
            ) { vm.setHapticStrength(haptics[it]) }
            SettingsSwitchRow(
                "Keep screen on",
                "The display stays awake between sets",
                state.keepScreenOn,
                onCheckedChange = vm::setKeepScreenOn
            )
        }

        SettingsGroup("Rest timer", footer = "Starting points. Hard sets add time, and the timer learns your pace.") {
            RestStepper("Compound lifts", "Squat, bench, deadlift, rows", state.restCompoundSeconds, COMPOUND_REST, vm::setRestCompoundSeconds)
            RestStepper("Isolation lifts", "Curls, raises, extensions", state.restIsolationSeconds, ISOLATION_REST, vm::setRestIsolationSeconds)
        }

        SettingsGroup("Note templates", footer = "One-tap starters under the note field when you log a set.") {
            NoteTemplatesEditor(state.noteTemplates, vm::addNoteTemplate, vm::removeNoteTemplate)
        }

        SettingsResetGroup(SettingsSection.SESSION, vm)
    }
}

/** A rest time walked along its fixed [steps]; an off-list stored value snaps to the nearest step. */
@Composable
private fun RestStepper(label: String, explainer: String, seconds: Int, steps: List<Int>, onChange: (Int) -> Unit) {
    val i = steps.indexOf(seconds).takeIf { it >= 0 }
        ?: steps.indices.minBy { kotlin.math.abs(steps[it] - seconds) }
    SettingsStepperRow(
        title = label,
        value = restLabel(steps[i]),
        canDecrease = i > 0,
        canIncrease = i < steps.lastIndex,
        onDecrease = { onChange(steps[i - 1]) },
        onIncrease = { onChange(steps[i + 1]) },
        supporting = explainer
    )
}

/**
 * The tap-to-insert note starters shown under the set note field (#540), as removable chips over
 * an add field. Each template drops onto its own line in the note with the cursor after it, so
 * prompt-style entries ("energy:", "tempo:") are the useful shape.
 */
@Composable
private fun NoteTemplatesEditor(
    templates: Set<String>,
    onAdd: (String) -> Unit,
    onRemove: (String) -> Unit
) {
    var input by remember { mutableStateOf("") }
    val sorted = remember(templates) { templates.filter { it.isNotBlank() }.sortedBy { it.trim().lowercase() } }
    val scheme = MaterialTheme.colorScheme
    SettingsGroupBlock {
        if (sorted.isEmpty()) {
            Text("None yet, so the note field starts blank.", style = MaterialTheme.typography.bodyMedium, color = scheme.onSurfaceVariant)
        } else {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                sorted.forEach { t ->
                    InputChip(
                        selected = false,
                        onClick = { onRemove(t) },
                        label = { Text(t.trim()) },
                        trailingIcon = {
                            Icon(Icons.Rounded.Close, contentDescription = "Remove ${t.trim()}", modifier = Modifier.height(18.dp))
                        },
                        colors = InputChipDefaults.inputChipColors(
                            containerColor = scheme.surfaceContainerHighest,
                            labelColor = scheme.onSurface,
                            trailingIconColor = scheme.onSurfaceVariant
                        ),
                        border = null
                    )
                }
            }
        }
        Spacer(Modifier.height(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = input,
                onValueChange = { input = it },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                textStyle = MaterialTheme.typography.bodyLarge,
                placeholder = { Text("Add a starter, e.g. tempo:", color = scheme.onSurfaceVariant) },
                colors = OutlinedTextFieldDefaults.colors(
                    focusedBorderColor = scheme.primary,
                    unfocusedBorderColor = scheme.outline,
                    cursorColor = scheme.primary
                ),
                modifier = Modifier.weight(1f)
            )
            Spacer(Modifier.width(10.dp))
            SettingsButton("Add", kind = ButtonKind.Tonal, enabled = input.isNotBlank()) {
                onAdd(input)
                input = ""
            }
        }
    }
}

/**
 * Each page's quiet last group: put this page back to its defaults. Scoped to the page's own
 * preferences (#544), so no data is touched.
 */
@Composable
internal fun SettingsResetGroup(section: SettingsSection, vm: SettingsViewModel) {
    SettingsGroup {
        SettingsActionRow(
            "Restore defaults",
            "Resets only this page. Your data stays.",
            Icons.Rounded.RestartAlt
        ) { vm.resetSection(section) }
    }
}

/**
 * Whether Android is blocking Avex's notifications (the Android 13+ permission denied, or blocked in system settings). Every phone
 * alert on the Notifications page is inert until it is granted, so the page shows why and turns
 * those rows off with their values kept. Re-checked on resume, so it clears the moment someone
 * comes back from system settings with notifications on.
 */
@Composable
private fun rememberNotificationsBlocked(): Boolean {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    // Also covers Android 8-12, where there is no runtime permission but the user can still block
    // Avex's notifications in system settings.
    fun granted() = com.forge.app.data.repo.NotificationFeed.osNotificationsEnabled(context)
    var blocked by remember { mutableStateOf(!granted()) }
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) blocked = !granted()
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    return blocked
}

@Composable
internal fun NotificationsPage(state: SettingsUiState, vm: SettingsViewModel, onBack: () -> Unit) {
    // Two groups named by where the thing arrives. Quiet hours sits with the phone alerts, the only
    // ones it silences. A blocked permission silences exactly those groups, never the in-app feed.
    val blocked = rememberNotificationsBlocked()
    val context = LocalContext.current
    SettingsScaffold("Notifications", onBack) {
        if (blocked) {
            SettingsNotice(
                icon = Icons.Rounded.NotificationsOff,
                title = "Phone notifications are off for Avex",
                body = "Reminders, the weekly recap and rest alerts can't reach you until you turn them on. In-app notices still work.",
                action = "Open system settings"
            ) {
                context.startActivity(
                    Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
                )
            }
        }

        SettingsGroup("On your phone") {
            SettingsSwitchRow(
                "Training reminders",
                "A daily nudge on the days you plan to train",
                state.trainingReminderEnabled,
                enabled = !blocked,
                onCheckedChange = vm::setTrainingReminderEnabled
            )
            if (state.trainingReminderEnabled) {
                // The clock format is Units & format's, read app-wide rather than from this page.
                SettingsHourRow(
                    "Remind me at",
                    state.trainingReminderHour,
                    com.forge.app.ui.theme.LocalForgeSettings.current.timeFormat24h,
                    enabled = !blocked,
                    indent = 16.dp,
                    onChange = vm::setTrainingReminderHour
                )
            }
            SettingsSwitchRow(
                "Weekly recap",
                "A summary of your week's workouts, volume and streak",
                state.weeklyRecapEnabled,
                enabled = !blocked,
                onCheckedChange = vm::setWeeklyRecapEnabled
            )
            SettingsSwitchRow(
                "Rest timer alerts",
                "Buzzes when rest ends while Avex is in the background",
                state.restTimerAlertEnabled,
                enabled = !blocked,
                onCheckedChange = vm::setRestTimerAlertEnabled
            )
        }

        SettingsGroup("Quiet hours") {
            SettingsSwitchRow(
                "Silence during quiet hours",
                "Mutes the phone alerts above inside the window",
                state.quietHoursEnabled,
                enabled = !blocked,
                onCheckedChange = vm::setQuietHoursEnabled
            )
            if (state.quietHoursEnabled) QuietHoursDays(state, vm, enabled = !blocked)
        }

        SettingsGroup("In the app") {
            com.forge.app.data.repo.NoticeKind.entries.forEach { kind ->
                SettingsSwitchRow(
                    kind.label,
                    kind.explainer,
                    kind.key !in state.disabledNoticeKinds,
                    onCheckedChange = { enabled -> vm.setNoticeKindEnabled(kind.key, enabled) }
                )
            }
        }

        SettingsResetGroup(SettingsSection.NOTIFICATIONS, vm)
    }
}
