@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.forge.app.ui.common.clickableLabeled

private val TIMEZONE_OPTIONS = listOf(
    "America/Los_Angeles" to "Los Angeles (PST −8)",
    "America/Denver"      to "Denver (MST −7)",
    "America/Chicago"     to "Chicago (CST −6)",
    "America/New_York"    to "New York (EST −5)",
    "America/Sao_Paulo"   to "São Paulo (BRT −3)",
    "UTC"                 to "UTC (±0)",
    "Europe/London"       to "London (GMT +0)",
    "Europe/Paris"        to "Paris (CET +1)",
    "Europe/Moscow"       to "Moscow (MSK +3)",
    "Asia/Kolkata"        to "Kolkata (IST +5:30)",
    "Asia/Tokyo"          to "Tokyo (JST +9)",
    "Australia/Sydney"    to "Sydney (AEST +10)"
)

@Composable
internal fun AppearancePage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxSize()) {
        ToggleRow("AMOLED pure black", "Pure-black backgrounds — saves battery on OLED screens; on an LCD phone it just looks darker.", state.amoledMode, vm::setAmoledMode)
        SectionDivider()
        ToggleRow("Compact set logging", "Denser set rows for experienced users", state.compactSetLogging, vm::setCompactSetLogging)
        SectionDivider()
        Spacer(Modifier.height(8.dp))
        AccentColorRow(state.accentColorHex, vm::setAccentColorHex)
        SectionDivider()
        SectionResetRow(com.forge.app.data.prefs.SettingsSection.APPEARANCE, vm)
    }
}

@Composable
internal fun FormatPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = 56.dp)
    ) {
        item("intro") {
            Text(
                "Units, date & time formats, week start, your timezone — and the basis for your strength standards.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp)
            )
        }
        item("weight") {
            ChipSection(
                "Weight unit",
                listOf("lb" to "lb", "kg" to "kg"),
                if (state.useKg) "kg" else "lb"
            ) { vm.setUseKg(it == "kg") }
            // Live preview — updates the moment the unit is switched.
            Text(
                "Everything shows in ${if (state.useKg) "kilograms" else "pounds"} — e.g. ${com.forge.app.domain.units.formatWeight(135.0, state.useKg)}.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
            Spacer(Modifier.height(8.dp))
            SectionDivider()
        }
        item("sex") {
            ChipSection(
                "Sex",
                listOf("male" to "Male", "female" to "Female"),
                state.userSex, vm::setUserSex
            )
            Text(
                "Only scales the bodyweight-relative strength standards on the Stats tab.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
            Spacer(Modifier.height(8.dp))
            SectionDivider()
        }
        item("date") {
            ChipSection(
                "Date format",
                listOf("MMM d, yyyy" to "Jan 5", "dd/MM/yyyy" to "05/01", "MM/dd/yyyy" to "01/05"),
                state.dateFormat, vm::setDateFormat
            )
            SectionDivider()
        }
        item("time") {
            ChipSection(
                "Time",
                listOf("12h" to "12h", "24h" to "24h"),
                if (state.timeFormat24h) "24h" else "12h"
            ) { vm.setTimeFormat24h(it == "24h") }
            SectionDivider()
        }
        item("week") {
            ChipSection(
                "Week starts",
                listOf("Mon" to "Mon", "Sun" to "Sun"),
                if (state.firstDayMonday) "Mon" else "Sun"
            ) { vm.setFirstDayMonday(it == "Mon") }
            SectionDivider()
        }
        item("timezone-header") {
            SubSectionLabel("Timezone")
        }
        item("timezone-current") {
            // If the saved/device timezone isn't one of the presets, show it as a selected row so
            // there's always a visible selection (was: nothing highlighted).
            if (TIMEZONE_OPTIONS.none { it.first == state.timezone }) {
                val current = state.timezone.ifBlank { java.util.TimeZone.getDefault().id }
                Row(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("$current (device)", style = MaterialTheme.typography.bodyMedium, color = onBg)
                    Text("●", style = MaterialTheme.typography.labelSmall, color = onBg)
                }
                HorizontalDivider(color = outline.copy(alpha = 0.12f), modifier = Modifier.padding(horizontal = 24.dp))
            }
        }
        items(TIMEZONE_OPTIONS, { it.first }) { (id, label) ->
            val selected = state.timezone == id
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { vm.setTimezone(id) }
                    .padding(horizontal = 24.dp, vertical = 12.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    label,
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (selected) onBg else muted.copy(alpha = 0.6f)
                )
                if (selected) {
                    Text("●", style = MaterialTheme.typography.labelSmall, color = onBg)
                }
            }
            HorizontalDivider(color = outline.copy(alpha = 0.12f), modifier = Modifier.padding(horizontal = 24.dp))
        }
        item("timezone-footer") { Spacer(Modifier.height(8.dp)) }
        item("reset") { SectionResetRow(com.forge.app.data.prefs.SettingsSection.FORMAT, vm) }
    }
}

@Composable
internal fun SessionPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        ChipSection(
            "Haptic feedback",
            listOf("off" to "Off", "light" to "Light", "medium" to "Medium", "strong" to "Strong"),
            state.hapticStrength, vm::setHapticStrength
        )
        SectionDivider()
        ChipSection(
            "Rest · compound lifts",
            listOf("120" to "2:00", "150" to "2:30", "180" to "3:00", "210" to "3:30", "240" to "4:00", "300" to "5:00"),
            state.restCompoundSeconds.toString()
        ) { vm.setRestCompoundSeconds(it.toInt()) }
        SectionDivider()
        ChipSection(
            "Rest · isolation lifts",
            listOf("45" to "0:45", "60" to "1:00", "90" to "1:30", "120" to "2:00", "150" to "2:30"),
            state.restIsolationSeconds.toString()
        ) { vm.setRestIsolationSeconds(it.toInt()) }
        SectionDivider()
        Text(
            "Your starting rest per set. The timer adapts from here — heavier sets and a brutal rating add time, " +
                "and it learns how long you actually rest. Set a per-exercise override on its card mid-workout.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp)
        )
        SectionDivider()
        NoteTemplatesEditor(state.noteTemplates, vm::addNoteTemplate, vm::removeNoteTemplate)
        SectionResetRow(com.forge.app.data.prefs.SettingsSection.SESSION, vm)
    }
}

/** Edit the quick-insert note templates shown under the note field when logging a set (#540). */
@Composable
private fun NoteTemplatesEditor(
    templates: Set<String>,
    onAdd: (String) -> Unit,
    onRemove: (String) -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    var input by remember { mutableStateOf("") }
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp)) {
        Text("NOTE TEMPLATES", style = MaterialTheme.typography.labelMedium, color = muted)
        Spacer(Modifier.height(4.dp))
        Text(
            "Quick-insert chips that show under the note field when you log a set.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(10.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            templates.forEach { t ->
                Row(
                    Modifier.background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(50))
                        .padding(start = 12.dp, end = 8.dp, top = 6.dp, bottom = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Text(t.trim(), style = MaterialTheme.typography.bodySmall, color = onBg)
                    Text("✕", style = MaterialTheme.typography.labelSmall, color = muted,
                        modifier = Modifier.clickableLabeled("Remove") { onRemove(t) }.padding(2.dp))
                }
            }
        }
        Spacer(Modifier.height(10.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = input,
                onValueChange = { input = it },
                singleLine = true,
                label = { Text("New template") },
                modifier = Modifier.weight(1f)
            )
            Text(
                "Add",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .clickableLabeled("Add") { if (input.isNotBlank()) { onAdd(input); input = "" } }
                    .padding(8.dp)
            )
        }
    }
}

/** A scoped "reset to defaults" affordance for a settings sub-page (#544) — clears only this page's
 *  preferences (no data loss), so a user can clean-slate one area without a global/factory reset. */
@Composable
internal fun SectionResetRow(section: com.forge.app.data.prefs.SettingsSection, vm: SettingsViewModel) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Text(
        "Reset this page to defaults",
        style = MaterialTheme.typography.labelSmall,
        color = muted.copy(alpha = 0.8f),
        modifier = Modifier.fillMaxWidth().clickableLabeled("Reset to defaults") { vm.resetSection(section) }.padding(horizontal = 24.dp, vertical = 16.dp)
    )
}

/**
 * Shown at the top of Notifications when the OS notification permission is denied (Android 13+) —
 * every toggle below is inert until it's granted. The in-app rationale (N1) is one-time, so this is
 * the re-enable path for a user who declined it: tapping opens the OS app-notification settings,
 * which always works regardless of how many times the permission was denied. Re-checks on resume so
 * it disappears the moment the user flips notifications on and returns.
 */
@Composable
private fun NotificationsBlockedBanner() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    fun granted() = context.checkSelfPermission(
        Manifest.permission.POST_NOTIFICATIONS
    ) == PackageManager.PERMISSION_GRANTED
    var blocked by remember { mutableStateOf(!granted()) }
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) blocked = !granted()
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    if (!blocked) return
    Column(
        Modifier
            .fillMaxWidth()
            .clickable(onClickLabel = "Open notification settings") {
                context.startActivity(
                    Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                        .putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
                )
            }
            .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.12f))
            .padding(horizontal = 24.dp, vertical = 14.dp)
    ) {
        Text("Notifications are turned off", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
        Spacer(Modifier.height(2.dp))
        Text(
            "Forge can't send any of the below until you turn them on for the app. Tap to open system settings.",
            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
    SectionDivider()
}

@Composable
internal fun NotificationsPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxSize()) {
        NotificationsBlockedBanner()
        ToggleRow(
            "Training reminders",
            "A daily nudge to train on your scheduled days — keeps your streak alive",
            state.trainingReminderEnabled, vm::setTrainingReminderEnabled
        )
        SectionDivider()
        if (state.trainingReminderEnabled) {
            HourPickerRow("Remind me at", state.trainingReminderHour, vm::setTrainingReminderHour)
            SectionDivider()
        }
        ToggleRow(
            "Weekly recap",
            "A weekly summary of your training — workouts, volume, streak",
            state.weeklyRecapEnabled, vm::setWeeklyRecapEnabled
        )
        SectionDivider()
        ToggleRow(
            "Rest timer alerts",
            "Buzz + notify when your rest ends while the app is in the background",
            state.restTimerAlertEnabled, vm::setRestTimerAlertEnabled
        )
        SectionDivider()
        ToggleRow("Quiet hours", "Suppress timer + recap notifications", state.quietHoursEnabled, vm::setQuietHoursEnabled)
        SectionDivider()
        if (state.quietHoursEnabled) {
            HourPickerRow("From", state.quietHoursStart, vm::setQuietHoursStart)
            HourPickerRow("Until", state.quietHoursEnd, vm::setQuietHoursEnd)
            SectionDivider()
        }
        SectionResetRow(com.forge.app.data.prefs.SettingsSection.NOTIFICATIONS, vm)
    }
}

@Composable
internal fun EquipmentPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    // Becomes true once equipment is touched this visit, so the regenerate prompt only nags after a change.
    var equipmentEdited by remember { mutableStateOf(false) }
    Column(modifier.fillMaxSize()) {
        Spacer(Modifier.height(16.dp))
        Text(
            "Generated programs only pick exercises you can do with the equipment you select here.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(Modifier.height(12.dp))
        // Quick presets — one-tap fill. The Developer's preset is curated (a locked exercise pool).
        FlowRow(
            modifier = Modifier.padding(horizontal = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            com.forge.app.program.equipmentPresets.forEach { preset ->
                val selected = state.availableEquipment == preset.equipment &&
                    state.frozenExerciseIds == preset.frozenIds
                PillChip(preset.label, selected) { vm.selectEquipmentPreset(preset); equipmentEdited = true }
            }
        }
        if (state.frozenExerciseIds != null) {
            Spacer(Modifier.height(10.dp))
            Text(
                "This is a curated preset — its exercise list is locked and won't change. " +
                    "Tap any equipment below to switch to a custom set.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
        }
        Spacer(Modifier.height(12.dp))
        FlowRow(
            modifier = Modifier.padding(horizontal = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            com.forge.app.program.Equipment.entries.forEach { equip ->
                val selected = equip.name in state.availableEquipment
                PillChip(equip.display.uppercase(), selected) {
                    val current = state.availableEquipment.toMutableSet()
                    if (selected) current.remove(equip.name) else current.add(equip.name)
                    vm.setAvailableEquipment(current)
                    equipmentEdited = true
                }
            }
        }
        Spacer(Modifier.height(20.dp))
        Text(
            if (equipmentEdited) "You changed your equipment — regenerate so your program only uses what you've got."
            else "Changed your equipment? Regenerate so the program matches what you've got.",
            style = MaterialTheme.typography.bodySmall,
            color = if (equipmentEdited) accent else muted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(Modifier.height(10.dp))
        FlowRow(modifier = Modifier.padding(horizontal = 24.dp)) {
            PillChip("Regenerate for this equipment", selected = equipmentEdited) {
                vm.generateProgram(state.daysPerWeek); equipmentEdited = false
            }
        }
        Spacer(Modifier.height(24.dp))

        // ── Plate weight ────────────────────────────────────────────────────────
        Text("Weight per plate", style = MaterialTheme.typography.titleSmall, color = onBg,
            modifier = Modifier.padding(horizontal = 24.dp))
        Spacer(Modifier.height(4.dp))
        Text(
            "For machines that load by counting plates (not a numbered stack): exercises are entered and " +
                "shown as a plate count. This is what one plate weighs, used for PRs and volume.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(Modifier.height(12.dp))
        FlowRow(
            modifier = Modifier.padding(horizontal = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            listOf(5.0, 10.0, 15.0, 20.0, 25.0, 45.0).forEach { w ->
                PillChip(com.forge.app.domain.units.formatWeight(w, state.useKg), state.plateWeightLb == w) { vm.setPlateWeightLb(w) }
            }
        }
        Spacer(Modifier.height(24.dp))

        // ── Heaviest dumbbell (auto-coach Phase 0) ──────────────────────────────
        Text("Heaviest dumbbell", style = MaterialTheme.typography.titleSmall, color = onBg,
            modifier = Modifier.padding(horizontal = 24.dp))
        Spacer(Modifier.height(4.dp))
        Text(
            "If your dumbbells max out (adjustable sets), heavy lifts in generated programs lean on " +
                "the plate stack instead, and weight suggestions switch to rep progression at the ceiling.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        Spacer(Modifier.height(12.dp))
        FlowRow(
            modifier = Modifier.padding(horizontal = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            PillChip("No limit", state.maxDbWeightLb == null) { vm.setMaxDbWeightLb(null) }
            listOf(15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 75.0, 100.0).forEach { w ->
                PillChip(com.forge.app.domain.units.formatWeight(w, state.useKg), state.maxDbWeightLb == w) { vm.setMaxDbWeightLb(w) }
            }
        }
        Spacer(Modifier.height(16.dp))
        SectionDivider()
    }
}

@Composable
internal fun PrivacyPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        ToggleRow("Privacy mode", "Hide the app preview in recent apps & block screenshots", state.privacyMode, vm::setPrivacyMode)
        SectionDivider()
        Text(
            "When on, the recent-apps switcher shows a blank thumbnail instead of your workout, " +
                "and screenshots / screen recording are blocked by the system.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp)
        )

        // What Forge stores — a plain data inventory, so someone handed the app knows exactly what's
        // kept and that none of it leaves the phone (there is no internet permission).
        Column(Modifier.padding(horizontal = 24.dp, vertical = 12.dp)) {
            Text("WHAT FORGE STORES", style = MaterialTheme.typography.labelMedium, color = muted)
            Spacer(Modifier.height(8.dp))
            listOf(
                "Workouts — sets, reps, weights & notes",
                "Bodyweight history & progress photos",
                "Mood check-ins & cardio",
                "Trophies, XP & rank",
                "What the coach has learned from you",
                "Your settings & generated program"
            ).forEach { line ->
                Text("·  $line", style = MaterialTheme.typography.bodySmall, color = onBg,
                    modifier = Modifier.padding(vertical = 2.dp))
            }
            Spacer(Modifier.height(12.dp))
            Text(
                "All of it stays in this app's private storage on this device. Forge holds no internet " +
                    "permission, so nothing is ever uploaded or shared — use Export / Backup to keep your own copy.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
            )
        }

        SectionDivider()

        // Export shortcut — privacy-aware users expect a data-export entry under Privacy.
        // Reuses the existing exportFullBackup() action (exportFullDataJson → share sheet).
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickableLabeled("Export all my data as JSON") { vm.exportFullBackup() }
                .padding(horizontal = 24.dp, vertical = 14.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text("Export all my data", style = MaterialTheme.typography.bodyMedium, color = onBg)
                Text(
                    "Human-readable JSON of every session, set & setting",
                    style = MaterialTheme.typography.labelSmall, color = muted
                )
            }
            Text("JSON →", style = MaterialTheme.typography.labelSmall, color = muted)
        }
    }
}

@Composable
internal fun ExercisePrefsPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    // Only show exercises the user can actually do — the available pool (curated freeze when a
    // preset like the Developer's is active, otherwise equipment-filtered). Muscle groups with no
    // available movement drop out entirely. An empty equipment set means "all" (no filter yet).
    val available = remember(state.availableEquipment) {
        state.availableEquipment.mapNotNull {
            runCatching { com.forge.app.program.Equipment.valueOf(it) }.getOrNull()
        }.toSet()
    }
    val byMuscle = remember(available, state.frozenExerciseIds) {
        com.forge.app.program.ExerciseLibrary
            .availablePool(available, state.frozenExerciseIds)
            .groupBy { it.muscle }
    }

    // Two-level navigation: pick a body part, then like/dislike its movements — so the page
    // isn't one giant scroll of every exercise at once.
    var selectedMuscle by remember { mutableStateOf<com.forge.app.program.MuscleGroup?>(null) }
    BackHandler(enabled = selectedMuscle != null) { selectedMuscle = null }

    val muscle = selectedMuscle
    if (muscle == null) {
        LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
            item("intro") {
                Text(
                    "Tell the generator what to favour or avoid. ♥ shows up more often · ✕ is never picked. Pick a body part to set its movements.",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted,
                    fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp)
                )
            }
            items(byMuscle.keys.toList(), key = { it.code }) { m ->
                val defs = byMuscle[m].orEmpty()
                val likedN = defs.count { it.id in state.liked }
                val dislikedN = defs.count { it.id in state.disliked }
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { selectedMuscle = m }
                        .padding(horizontal = 24.dp, vertical = 14.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column {
                        Text(m.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        val summary = buildList {
                            if (likedN > 0) add("$likedN ♥")
                            if (dislikedN > 0) add("$dislikedN ✕")
                        }.joinToString(" · ").ifEmpty { "${defs.size} exercises" }
                        Text(summary, style = MaterialTheme.typography.labelSmall, color = muted)
                    }
                    Text("→", style = MaterialTheme.typography.bodyMedium, color = muted)
                }
                HorizontalDivider(color = outline.copy(alpha = 0.12f), modifier = Modifier.padding(horizontal = 24.dp))
            }
        }
    } else {
        val defs = byMuscle[muscle].orEmpty()
        LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
            item("back") {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { selectedMuscle = null }
                        .padding(horizontal = 24.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text("←", style = MaterialTheme.typography.bodyMedium, color = muted)
                    Text(
                        muscle.displayName.uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        color = onBg
                    )
                }
                HorizontalDivider(color = outline.copy(alpha = 0.25f), modifier = Modifier.padding(horizontal = 24.dp))
            }
            items(defs, key = { it.id }) { def ->
                Row(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text(
                        def.name,
                        style = MaterialTheme.typography.bodyMedium,
                        color = onBg,
                        modifier = Modifier.weight(1f)
                    )
                    PillChip("♥", def.id in state.liked) { vm.toggleLike(def.id) }
                    PillChip("✕", def.id in state.disliked) { vm.toggleDislike(def.id) }
                }
            }
        }
    }
}

// ─── LazyListScope helpers not in stdlib ─────────────────────────────────────

private fun androidx.compose.foundation.lazy.LazyListScope.items(
    items: List<Pair<String, String>>,
    key: (Pair<String, String>) -> Any,
    itemContent: @Composable (Pair<String, String>) -> Unit
) {
    items.forEach { pair ->
        item("kv-${key(pair)}") { itemContent(pair) }
    }
}
