@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
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
        ToggleRow("Privacy mode", "Hide the app preview in recent apps & block screenshots", state.privacyMode, vm::setPrivacyMode)
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
    var showTzPicker by remember { mutableStateOf(false) }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 32.dp)
    ) {
        Text(
            "Units, date & time, week start & timezone — and the basis for your strength standards.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 4.dp)
        )

        // ── Units ────────────────────────────────────────────────────────────────
        GroupHeader("Units")
        SettingsCard {
            InlineChipRow(
                "Weight",
                listOf("lb" to "lb", "kg" to "kg"),
                if (state.useKg) "kg" else "lb"
            ) { vm.setUseKg(it == "kg") }
            CardDivider()
            InlineChipRow(
                "Distance",
                listOf("km" to "km", "mi" to "mi"),
                if (state.useMiles) "mi" else "km"
            ) { vm.setUseMiles(it == "mi") }
        }
        // Live preview — updates the moment a unit is switched.
        CardCaption(
            "e.g. ${com.forge.app.domain.units.formatWeight(135.0, state.useKg)} · " +
                com.forge.app.domain.units.formatDistance(5.0, state.useMiles)
        )

        // ── Date & time ────────────────────────────────────────────────────────────
        GroupHeader("Date & time")
        SettingsCard {
            InlineChipRow(
                "Date",
                listOf("MMM d, yyyy" to "Jan 5", "dd/MM/yyyy" to "05/01", "MM/dd/yyyy" to "01/05"),
                state.dateFormat, vm::setDateFormat
            )
            CardDivider()
            InlineChipRow(
                "Time",
                listOf("12h" to "12h", "24h" to "24h"),
                if (state.timeFormat24h) "24h" else "12h"
            ) { vm.setTimeFormat24h(it == "24h") }
            CardDivider()
            InlineChipRow(
                "Week starts",
                listOf("Mon" to "Mon", "Sun" to "Sun"),
                if (state.firstDayMonday) "Mon" else "Sun"
            ) { vm.setFirstDayMonday(it == "Mon") }
            CardDivider()
            TimezoneRow(state.timezone) { showTzPicker = true }
        }

        // ── Strength standards ──────────────────────────────────────────────────────
        GroupHeader("Strength standards")
        SettingsCard {
            InlineChipRow(
                "Sex",
                listOf("male" to "Male", "female" to "Female"),
                state.userSex, vm::setUserSex
            )
        }
        CardCaption("Only scales the bodyweight-relative strength standards on the Stats tab.")

        Spacer(Modifier.height(8.dp))
        SectionResetRow(com.forge.app.data.prefs.SettingsSection.FORMAT, vm)
    }

    if (showTzPicker) {
        TimezonePickerDialog(
            current = state.timezone,
            onPick = { vm.setTimezone(it); showTzPicker = false },
            onDismiss = { showTzPicker = false }
        )
    }
}

// ─── Format-page building blocks (grouped "card" layout) ─────────────────────

/** Quiet uppercase section label that sits just above a [SettingsCard]. */
@Composable
private fun GroupHeader(text: String) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        letterSpacing = 1.5.sp,
        modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 8.dp)
    )
}

/** A small muted italic caption shown just below a card (live previews, scope notes). */
@Composable
private fun CardCaption(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        fontStyle = FontStyle.Italic,
        modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 8.dp)
    )
}

/** The rounded surface "card" that groups a few setting rows together. */
@Composable
private fun SettingsCard(content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant),
        content = content
    )
}

/** Inset hairline divider between rows inside a [SettingsCard]. */
@Composable
private fun CardDivider() {
    HorizontalDivider(
        modifier = Modifier.padding(start = 16.dp),
        color = MaterialTheme.colorScheme.outline.copy(alpha = 0.18f)
    )
}

/** Label on the left, a chip group on the right — one setting per line, inside a card. */
@Composable
private fun InlineChipRow(
    label: String,
    options: List<Pair<String, String>>,
    selected: String,
    onSelect: (String) -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.weight(1f)
        )
        Spacer(Modifier.width(12.dp))
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            options.forEach { (value, display) ->
                PillChip(display.uppercase(), selected == value) { onSelect(value) }
            }
        }
    }
}

/** Collapsed timezone selector — shows the current zone and opens [TimezonePickerDialog] on tap. */
@Composable
private fun TimezoneRow(timezone: String, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val label = TIMEZONE_OPTIONS.firstOrNull { it.first == timezone }?.second
        ?: "${timezone.ifBlank { java.util.TimeZone.getDefault().id }} · device"
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text("Timezone", style = MaterialTheme.typography.bodyMedium, color = onBg)
        Spacer(Modifier.weight(1f))
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.widthIn(max = 200.dp)
        )
        Text(" ▾", style = MaterialTheme.typography.bodyMedium, color = muted)
    }
}

/** The full timezone list, opened from [TimezoneRow] so the page isn't 13 stacked rows. */
@Composable
private fun TimezonePickerDialog(
    current: String,
    onPick: (String) -> Unit,
    onDismiss: () -> Unit
) {
    val outline = MaterialTheme.colorScheme.outline
    // If the saved/device zone isn't a preset, surface it first so there's always a visible selection.
    val showDevice = TIMEZONE_OPTIONS.none { it.first == current }
    val deviceTz = current.ifBlank { java.util.TimeZone.getDefault().id }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Timezone") },
        text = {
            LazyColumn {
                if (showDevice) {
                    item {
                        TimezoneOption("$deviceTz · device", selected = true) { onPick(deviceTz) }
                        HorizontalDivider(color = outline.copy(alpha = 0.12f))
                    }
                }
                items(TIMEZONE_OPTIONS, key = { it.first }) { (id, label) ->
                    TimezoneOption(label, selected = current == id) { onPick(id) }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } }
    )
}

@Composable
private fun TimezoneOption(label: String, selected: Boolean, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = if (selected) onBg else muted)
        if (selected) Text("●", style = MaterialTheme.typography.labelSmall, color = onBg)
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

    // Search-first flat list: a search box + filter chips over one scrolling list with muscle
    // sub-headers, plus a Custom section for user-created movements. No more drill-down.
    var query by remember { mutableStateOf("") }
    var filter by remember { mutableStateOf(PrefFilter.ALL) }
    val q = query.trim()

    val visibleByMuscle = remember(byMuscle, q, filter, state.liked, state.disliked) {
        byMuscle
            .mapValues { (_, defs) -> defs.filter { libVisible(it, q, filter, state.liked, state.disliked) } }
            .filterValues { it.isNotEmpty() }
    }
    val visibleCustom = remember(state.customExercises, q, filter, state.liked, state.disliked) {
        state.customExercises.filter { customVisible(it, q, filter, state.liked, state.disliked) }
    }
    val nothingMatches = visibleByMuscle.isEmpty() && visibleCustom.isEmpty()

    Column(modifier.fillMaxSize()) {
        PrefSearchField(query, onQuery = { query = it })
        FlowRow(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            PrefFilter.entries.forEach { f ->
                PillChip(f.label, filter == f) { filter = f }
            }
        }
        Text(
            "♥ shows up more often in generated programs · ✕ is never picked.",
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 8.dp)
        )

        LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(bottom = 32.dp)) {
            visibleByMuscle.forEach { (m, defs) ->
                item("hdr-${m.code}") {
                    val likedN = defs.count { it.id in state.liked }
                    val dislikedN = defs.count { it.id in state.disliked }
                    PrefSectionHeader(m.displayName.uppercase(), prefSummary(likedN, dislikedN, defs.size))
                }
                items(defs, key = { "lib-${it.id}" }) { def ->
                    ExercisePrefRow(
                        name = def.name,
                        subtitle = libSubtitle(def),
                        liked = def.id in state.liked,
                        disliked = def.id in state.disliked,
                        onLike = { vm.setExercisesLiked(setOf(def.id), def.id !in state.liked) },
                        onDislike = { vm.setExercisesDisliked(setOf(def.id), def.id !in state.disliked) }
                    )
                }
            }
            if (visibleCustom.isNotEmpty()) {
                item("hdr-custom") {
                    PrefSectionHeader(
                        "★ CUSTOM",
                        "${visibleCustom.size} ${if (visibleCustom.size == 1) "exercise" else "exercises"}"
                    )
                }
                // Group id-sets are disjoint, so the smallest id is a unique, stable per-group key
                // (name+muscle could collide on malformed data and crash the list).
                items(visibleCustom, key = { "cus-${it.ids.minOrNull()}" }) { ref ->
                    val liked = ref.ids.any { it in state.liked }
                    val disliked = ref.ids.any { it in state.disliked }
                    ExercisePrefRow(
                        name = ref.name,
                        subtitle = "Custom · ${ref.muscle.displayName}",
                        liked = liked,
                        disliked = disliked,
                        onLike = { vm.setExercisesLiked(ref.ids, !liked) },
                        onDislike = { vm.setExercisesDisliked(ref.ids, !disliked) }
                    )
                }
            }
            if (nothingMatches) {
                item("empty") {
                    Text(
                        if (q.isNotEmpty()) "No exercises match “$q”." else "No exercises here yet.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = muted,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 48.dp)
                    )
                }
            }
            // The post-swap dislike prompt toggle lives here too — it's about likes/dislikes.
            item("swap-toggle") {
                Spacer(Modifier.height(12.dp))
                HorizontalDivider(color = outline.copy(alpha = 0.12f), modifier = Modifier.padding(horizontal = 24.dp))
                ToggleRow(
                    label = "Ask to dislike after swapping",
                    subtitle = "After a \"Make default\" swap, offer to hide the old exercise.",
                    checked = state.swapDislikePromptEnabled,
                    onCheckedChange = { vm.setSwapDislikePromptEnabled(it) }
                )
            }
        }
    }
}

/** Top-level filter for the Exercise likes list. */
private enum class PrefFilter(val label: String) {
    ALL("All"), LIKED("♥ Liked"), DISLIKED("✕ Disliked"), CUSTOM("★ Custom")
}

private fun matchesQuery(name: String, q: String): Boolean =
    q.isEmpty() || name.contains(q, ignoreCase = true)

private fun libVisible(
    def: com.forge.app.program.ExerciseDef,
    q: String,
    filter: PrefFilter,
    liked: Set<String>,
    disliked: Set<String>
): Boolean {
    if (!matchesQuery(def.name, q)) return false
    return when (filter) {
        PrefFilter.ALL -> true
        PrefFilter.LIKED -> def.id in liked
        PrefFilter.DISLIKED -> def.id in disliked
        PrefFilter.CUSTOM -> false
    }
}

private fun customVisible(
    ref: com.forge.app.data.repo.CustomExerciseRef,
    q: String,
    filter: PrefFilter,
    liked: Set<String>,
    disliked: Set<String>
): Boolean {
    if (!matchesQuery(ref.name, q)) return false
    return when (filter) {
        PrefFilter.ALL, PrefFilter.CUSTOM -> true
        PrefFilter.LIKED -> ref.ids.any { it in liked }
        PrefFilter.DISLIKED -> ref.ids.any { it in disliked }
    }
}

/** Per-muscle header summary: liked/disliked tallies, or a plain move count when neither is set. */
private fun prefSummary(likedN: Int, dislikedN: Int, total: Int): String =
    buildList {
        if (likedN > 0) add("$likedN ♥")
        if (dislikedN > 0) add("$dislikedN ✕")
    }.joinToString(" · ").ifEmpty { "$total ${if (total == 1) "move" else "moves"}" }

/** "Barbell · Compound"-style secondary line for a library movement. */
private fun libSubtitle(def: com.forge.app.program.ExerciseDef): String {
    val equip = def.equipment.firstOrNull()?.display ?: "Bodyweight"
    val kind = when {
        com.forge.app.program.ExerciseTag.COMPOUND in def.tags -> "Compound"
        com.forge.app.program.ExerciseTag.ISOLATION in def.tags -> "Isolation"
        else -> def.difficulty.displayName
    }
    return "$equip · $kind"
}

@Composable
private fun PrefSearchField(query: String, onQuery: (String) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 12.dp)
            .border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(8.dp))
            .padding(horizontal = 14.dp, vertical = 11.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Text("⌕", style = MaterialTheme.typography.bodyLarge, color = muted)
        BasicTextField(
            value = query,
            onValueChange = onQuery,
            singleLine = true,
            textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
            cursorBrush = SolidColor(onBg),
            modifier = Modifier.weight(1f),
            decorationBox = { inner ->
                Box {
                    if (query.isEmpty()) {
                        Text(
                            "Search exercises…",
                            style = MaterialTheme.typography.bodyMedium,
                            color = muted.copy(alpha = 0.5f)
                        )
                    }
                    inner()
                }
            }
        )
        if (query.isNotEmpty()) {
            Text(
                "×",
                style = MaterialTheme.typography.bodyLarge,
                color = muted,
                modifier = Modifier.clickable { onQuery("") }
            )
        }
    }
}

@Composable
private fun PrefSectionHeader(title: String, summary: String) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 24.dp, end = 24.dp, top = 18.dp, bottom = 6.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(title, style = MaterialTheme.typography.labelSmall, color = onBg)
        Text(summary, style = MaterialTheme.typography.labelSmall, color = muted)
    }
    HorizontalDivider(color = outline.copy(alpha = 0.2f), modifier = Modifier.padding(horizontal = 24.dp))
}

@Composable
private fun ExercisePrefRow(
    name: String,
    subtitle: String,
    liked: Boolean,
    disliked: Boolean,
    onLike: () -> Unit,
    onDislike: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(name, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(subtitle, style = MaterialTheme.typography.labelSmall, color = muted)
        }
        PillChip("♥", liked, onClick = onLike)
        PillChip("✕", disliked, onClick = onDislike)
    }
}
