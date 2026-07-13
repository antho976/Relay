@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import androidx.compose.animation.AnimatedVisibility
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
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Icon
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
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
    var showAppIconSheet by remember { mutableStateOf(false) }
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp)
    ) {
        // Grouped by quiet mono anchors + air — no per-row hairlines (DESIGN §1/§7).
        SettingsSectionHeader("Display", top = 12.dp)
        ToggleRow("AMOLED pure black", "Pure-black backgrounds. Saves battery on OLED screens; on an LCD phone it just looks darker.", state.amoledMode, vm::setAmoledMode)
        ToggleRow("Compact set logging", "Denser set rows for experienced users", state.compactSetLogging, vm::setCompactSetLogging)
        ToggleRow("Privacy mode", "Hide the app preview in recent apps & block screenshots", state.privacyMode, vm::setPrivacyMode)

        SettingsSectionHeader("Accent")
        ToggleRow(
            "Use accent color",
            "Off makes the app monochrome; highlights use a neutral tone.",
            state.accentEnabled,
            vm::setAccentEnabled
        )
        // The picker is only meaningful while the accent is on, so it collapses away when off (the
        // chosen colour is kept and returns on re-enable).
        AnimatedVisibility(visible = state.accentEnabled) {
            AccentColorRow(state.accentColorHex, vm::setAccentColorHex)
        }

        SettingsSectionHeader("App icon")
        AppIconRow(state.appIconKey) { showAppIconSheet = true }
        ToggleRow(
            "Custom startup animation",
            "Off shows the plain black-and-white Avex instead of the icon-themed launch.",
            state.themedLaunchIntro,
            vm::setThemedLaunchIntro
        )

        Spacer(Modifier.height(8.dp))
        SectionResetRow(com.forge.app.data.prefs.SettingsSection.APPEARANCE, vm)
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
internal fun FormatPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    var showTzPicker by remember { mutableStateOf(false) }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 32.dp)
    ) {
        // Grouped by quiet mono anchors + air, no per-row hairlines (DESIGN §1/§7) —
        // the same rhythm the Appearance page already migrated to.
        SettingsSectionHeader("Units", top = 12.dp)
        InlineChipRow(
            "Weight",
            listOf("lb" to "lb", "kg" to "kg"),
            if (state.useKg) "kg" else "lb"
        ) { vm.setUseKg(it == "kg") }
        InlineChipRow(
            "Distance",
            listOf("km" to "km", "mi" to "mi"),
            if (state.useMiles) "mi" else "km"
        ) { vm.setUseMiles(it == "mi") }
        InlineChipRow(
            "Length",
            listOf("cm" to "cm", "in" to "in"),
            if (state.useCm) "cm" else "in"
        ) { vm.setUseCm(it == "cm") }
        // Live preview — updates the moment a unit is switched.
        CardFootnote(
            "e.g. ${com.forge.app.domain.units.formatWeight(135.0, state.useKg)} · " +
                "${com.forge.app.domain.units.formatDistance(5.0, state.useMiles)} · " +
                com.forge.app.domain.units.formatLength(90.0, state.useCm)
        )

        SettingsSectionHeader("Date & time")
        InlineChipRow(
            "Date",
            listOf("MMM d, yyyy" to "Jan 5", "dd/MM/yyyy" to "05/01", "MM/dd/yyyy" to "01/05"),
            state.dateFormat, vm::setDateFormat
        )
        InlineChipRow(
            "Time",
            listOf("12h" to "12h", "24h" to "24h"),
            if (state.timeFormat24h) "24h" else "12h"
        ) { vm.setTimeFormat24h(it == "24h") }
        InlineChipRow(
            "Week starts",
            listOf("Mon" to "Mon", "Sun" to "Sun"),
            if (state.firstDayMonday) "Mon" else "Sun"
        ) { vm.setFirstDayMonday(it == "Mon") }
        TimezoneRow(state.timezone) { showTzPicker = true }

        SettingsSectionHeader("Strength standards")
        InlineChipRow(
            "Sex",
            listOf("male" to "Male", "female" to "Female"),
            state.userSex, vm::setUserSex
        )
        CardFootnote("Only scales the bodyweight-relative strength standards on the Stats tab.")

        SectionResetRow(com.forge.app.data.prefs.SettingsSection.FORMAT, vm)
    }

    if (showTzPicker) {
        TimezonePickerDialog(
            current = state.timezone,
            favorites = state.favoriteTimezones,
            onPick = { vm.setTimezone(it); showTzPicker = false },
            onToggleFavorite = vm::toggleFavoriteTimezone,
            onDismiss = { showTzPicker = false }
        )
    }
}

// ─── Format-page building blocks ─────────────────────────────────────────────

/** A muted italic note — live previews and scope annotations sitting openly on the page. */
@Composable
private fun CardFootnote(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        fontStyle = FontStyle.Italic,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 10.dp)
    )
}

/** Label on the left, a chip group on the right — one setting per open row. */
@Composable
private fun InlineChipRow(
    label: String,
    options: List<Pair<String, String>>,
    selected: String,
    onSelect: (String) -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp),
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
    val label = when {
        timezone.isBlank() -> "${java.util.TimeZone.getDefault().id} · device"
        else -> TIMEZONE_OPTIONS.firstOrNull { it.first == timezone }?.second
            ?: timezone.substringAfterLast('/').replace('_', ' ')
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 8.dp),
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

/** One row in the searchable "all timezones" list. [label] is the friendly, offset-stamped display. */
private data class TzEntry(val id: String, val city: String, val offsetMinutes: Int, val label: String)

/** "+5:30" / "−8" / "±0" for a UTC offset given in minutes. */
private fun tzOffsetLabel(mins: Int): String {
    if (mins == 0) return "±0"
    val sign = if (mins > 0) "+" else "−"
    val abs = kotlin.math.abs(mins)
    val h = abs / 60
    val m = abs % 60
    return if (m == 0) "$sign$h" else "$sign$h:${m.toString().padStart(2, '0')}"
}

/**
 * Every IANA zone (minus the noisy Etc/SystemV aliases & bare 3-letter ids), stamped with its
 * *current* UTC offset and sorted west→east then alphabetically. Built once per open via remember.
 */
private fun allTimezones(): List<TzEntry> {
    val now = java.time.Instant.now()
    return java.time.ZoneId.getAvailableZoneIds()
        .asSequence()
        .filter { it.contains('/') && !it.startsWith("Etc/") && !it.startsWith("SystemV/") }
        .map { id ->
            val mins = java.time.ZoneId.of(id).rules.getOffset(now).totalSeconds / 60
            val city = id.substringAfterLast('/').replace('_', ' ')
            val region = id.substringBeforeLast('/').replace('_', ' ')
            TzEntry(id, city, mins, "$city · $region  (UTC${tzOffsetLabel(mins)})")
        }
        .sortedWith(compareBy({ it.offsetMinutes }, { it.city }))
        .toList()
}

/**
 * Timezone picker: a search box over the full IANA list. Tap ☆ to star a zone; starred zones (and
 * the current pick, if it isn't already pinned) sit at the top, followed by the common presets and
 * the full list. Tapping a row selects & closes; tapping its star just favorites and stays open.
 */
@Composable
private fun TimezonePickerDialog(
    current: String,
    favorites: Set<String>,
    onPick: (String) -> Unit,
    onToggleFavorite: (String) -> Unit,
    onDismiss: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    var query by remember { mutableStateOf("") }
    val all = remember { allTimezones() }
    val allById = remember(all) { all.associate { it.id to it.label } }
    val commonIds = remember { TIMEZONE_OPTIONS.map { it.first }.toSet() }
    fun labelFor(id: String): String =
        allById[id] ?: TIMEZONE_OPTIONS.firstOrNull { it.first == id }?.second ?: id

    val q = query.trim()
    val filtered = remember(q) {
        if (q.isBlank()) emptyList()
        else all.filter { it.id.contains(q, ignoreCase = true) || it.label.contains(q, ignoreCase = true) }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Timezone") },
        text = {
            Column {
                // ── Search box ──
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant)
                        .padding(horizontal = 12.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    BasicTextField(
                        value = query,
                        onValueChange = { query = it },
                        textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
                        cursorBrush = SolidColor(onBg),
                        singleLine = true,
                        decorationBox = { inner ->
                            Box {
                                if (query.isEmpty()) {
                                    Text(
                                        "Search a city or region…",
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = muted.copy(alpha = 0.5f),
                                        fontStyle = FontStyle.Italic
                                    )
                                }
                                inner()
                            }
                        },
                        modifier = Modifier.weight(1f)
                    )
                    if (query.isNotEmpty()) {
                        Text(
                            "×",
                            style = MaterialTheme.typography.bodyLarge,
                            color = muted,
                            modifier = Modifier
                                .clickableLabeled("Clear search") { query = "" }
                                .padding(start = 8.dp)
                        )
                    }
                }
                Spacer(Modifier.height(8.dp))

                LazyColumn(modifier = Modifier.heightIn(max = 360.dp)) {
                    if (q.isNotBlank()) {
                        if (filtered.isEmpty()) {
                            item("empty") {
                                Text(
                                    "No timezone matches “$q”.",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = muted,
                                    modifier = Modifier.padding(vertical = 16.dp)
                                )
                            }
                        } else {
                            items(filtered, key = { it.id }) { e ->
                                TimezoneOption(
                                    e.label, selected = current == e.id,
                                    isFavorite = e.id in favorites,
                                    onToggleFavorite = { onToggleFavorite(e.id) }
                                ) { onPick(e.id) }
                            }
                        }
                    } else {
                        // The current pick, only when it isn't already shown under Favorites/Common.
                        if (current !in favorites && current !in commonIds) {
                            item("cur-label") { TzSectionLabel("Current") }
                            item("cur") {
                                TimezoneOption(
                                    labelFor(current), selected = true,
                                    isFavorite = false,
                                    onToggleFavorite = { onToggleFavorite(current) }
                                ) { onPick(current) }
                            }
                        }
                        if (favorites.isNotEmpty()) {
                            item("fav-label") { TzSectionLabel("Favorites") }
                            items(favorites.sortedBy { labelFor(it) }, key = { "fav-$it" }) { id ->
                                TimezoneOption(
                                    labelFor(id), selected = current == id,
                                    isFavorite = true,
                                    onToggleFavorite = { onToggleFavorite(id) }
                                ) { onPick(id) }
                            }
                        }
                        item("common-label") { TzSectionLabel("Common") }
                        items(TIMEZONE_OPTIONS, key = { "common-${it.first}" }) { (id, label) ->
                            TimezoneOption(
                                label, selected = current == id,
                                isFavorite = id in favorites,
                                onToggleFavorite = { onToggleFavorite(id) }
                            ) { onPick(id) }
                        }
                        item("all-label") { TzSectionLabel("All timezones") }
                        items(all, key = { it.id }) { e ->
                            TimezoneOption(
                                e.label, selected = current == e.id,
                                isFavorite = e.id in favorites,
                                onToggleFavorite = { onToggleFavorite(e.id) }
                            ) { onPick(e.id) }
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } }
    )
}

@Composable
private fun TzSectionLabel(text: String) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        letterSpacing = 1.sp,
        modifier = Modifier.padding(top = 10.dp, bottom = 4.dp)
    )
}

@Composable
private fun TimezoneOption(
    label: String,
    selected: Boolean,
    isFavorite: Boolean,
    onToggleFavorite: () -> Unit,
    onClick: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (selected) onBg else muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f)
        )
        if (selected) Text("●", style = MaterialTheme.typography.labelSmall, color = onBg, modifier = Modifier.padding(start = 8.dp))
        Text(
            if (isFavorite) "★" else "☆",
            style = MaterialTheme.typography.bodyLarge,
            color = if (isFavorite) onBg else muted.copy(alpha = 0.5f),
            modifier = Modifier
                .clickableLabeled(if (isFavorite) "Remove favorite" else "Add favorite", onClick = onToggleFavorite)
                .padding(start = 12.dp, top = 2.dp, bottom = 2.dp)
        )
    }
}

@Composable
internal fun SessionPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp)
    ) {
        // Grouped by quiet mono anchors + air, no per-row hairlines (DESIGN §1/§7) — the rhythm
        // Appearance/Format/Notifications already migrated to.
        SettingsSectionHeader("Haptics", top = 12.dp)
        ChipField(
            label = "Feedback strength",
            explainer = "Buzzes when you log a set, hit a PR, or rest runs out.",
            options = listOf("off" to "Off", "light" to "Light", "medium" to "Medium", "strong" to "Strong"),
            selected = state.hapticStrength,
            onSelect = vm::setHapticStrength
        )

        SettingsSectionHeader("Screen")
        ToggleRow(
            "Keep screen on",
            "Keeps the display awake while logging so it won't lock between sets.",
            state.keepScreenOn,
            vm::setKeepScreenOn
        )

        SettingsSectionHeader("Rest timer")
        ChipField(
            label = "Compound lifts",
            explainer = "Multi-joint lifts like squat, bench, deadlift, and rows.",
            options = listOf("120" to "2:00", "150" to "2:30", "180" to "3:00", "210" to "3:30", "240" to "4:00", "300" to "5:00"),
            selected = state.restCompoundSeconds.toString(),
            onSelect = { vm.setRestCompoundSeconds(it.toInt()) }
        )
        ChipField(
            label = "Isolation lifts",
            explainer = "Single-muscle lifts like curls, raises, and extensions.",
            options = listOf("45" to "0:45", "60" to "1:00", "90" to "1:30", "120" to "2:00", "150" to "2:30"),
            selected = state.restIsolationSeconds.toString(),
            onSelect = { vm.setRestIsolationSeconds(it.toInt()) }
        )
        CardFootnote("Starting points · hard sets add time, and the timer learns your pace.")

        SettingsSectionHeader("Note templates")
        NoteTemplatesEditor(state.noteTemplates, vm::addNoteTemplate, vm::removeNoteTemplate)

        Spacer(Modifier.height(8.dp))
        SectionResetRow(com.forge.app.data.prefs.SettingsSection.SESSION, vm)
    }
}

/** A chip-group control: sans label (+ optional one-line explainer) over its option pills —
 *  the label sits above because these option rows are too wide for the label-left layout. */
@Composable
private fun ChipField(
    label: String,
    explainer: String?,
    options: List<Pair<String, String>>,
    selected: String,
    onSelect: (String) -> Unit
) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 10.dp)) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
        if (explainer != null) {
            Spacer(Modifier.height(2.dp))
            Text(
                explainer,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontSize = 10.sp
            )
        }
        Spacer(Modifier.height(10.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            options.forEach { (value, display) ->
                PillChip(display.uppercase(), selected == value) { onSelect(value) }
            }
        }
    }
}

/**
 * Edit the tap-to-insert note starters shown under the set note field (#540). Each template drops
 * onto its own line in the note with the cursor parked after it, so prompt-style entries
 * ("energy:", "tempo:") are the useful shape. Removing every template just leaves the field bare.
 */
@Composable
private fun NoteTemplatesEditor(
    templates: Set<String>,
    onAdd: (String) -> Unit,
    onRemove: (String) -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    var input by remember { mutableStateOf("") }
    val sorted = remember(templates) {
        templates.filter { it.isNotBlank() }.sortedBy { it.trim().lowercase() }
    }
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 10.dp)) {
        Text(
            "One-tap starters under the note field when you log a set.",
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 10.sp
        )
        Spacer(Modifier.height(10.dp))
        if (sorted.isEmpty()) {
            Text(
                "No templates · the note field starts blank.",
                style = MaterialTheme.typography.bodySmall,
                color = muted,
                fontStyle = FontStyle.Italic
            )
        } else {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                sorted.forEach { t ->
                    Row(
                        Modifier
                            .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(50))
                            .padding(start = 12.dp, end = 8.dp, top = 6.dp, bottom = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Text(t.trim(), style = MaterialTheme.typography.bodySmall, color = onBg)
                        Text(
                            "✕", style = MaterialTheme.typography.labelSmall, color = muted,
                            modifier = Modifier.clickableLabeled("Remove ${t.trim()}") { onRemove(t) }.padding(2.dp)
                        )
                    }
                }
            }
        }
        Spacer(Modifier.height(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            OutlinedTextField(
                value = input,
                onValueChange = { input = it },
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyMedium,
                placeholder = {
                    Text(
                        "Add a starter, e.g. tempo:",
                        style = MaterialTheme.typography.bodyMedium,
                        color = muted.copy(alpha = 0.5f)
                    )
                },
                modifier = Modifier.weight(1f)
            )
            SettingsOutlineAction("Add", enabled = input.isNotBlank()) {
                onAdd(input)
                input = ""
            }
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
            .padding(horizontal = 24.dp, vertical = 14.dp)
    ) {
        Text("Notifications are turned off", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
        Spacer(Modifier.height(2.dp))
        Text(
            "Avex can't send any of the below until you turn them on for the app. Tap to open system settings →",
            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
    Spacer(Modifier.height(8.dp))
}

@Composable
internal fun NotificationsPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxSize()) {
        NotificationsBlockedBanner()

        // Quiet mono anchors + air, no per-row hairlines (DESIGN §1/§7) — same rhythm as Format.
        // "Alerts" (what Avex sends) reads cleaner than echoing the page name "Notifications" (§2).
        SettingsSectionHeader("Alerts", top = 12.dp)
        ToggleRow(
            "Training reminders",
            "A daily nudge to train on scheduled days and keep your streak alive",
            state.trainingReminderEnabled, vm::setTrainingReminderEnabled
        )
        if (state.trainingReminderEnabled) {
            HourPickerRow("Remind me at", state.trainingReminderHour, vm::setTrainingReminderHour)
        }
        ToggleRow(
            "Weekly recap",
            "A weekly summary of workouts, volume, and streak",
            state.weeklyRecapEnabled, vm::setWeeklyRecapEnabled
        )
        ToggleRow(
            "Rest timer alerts",
            "Buzz + notify when your rest ends while the app is in the background",
            state.restTimerAlertEnabled, vm::setRestTimerAlertEnabled
        )

        // Own group: the mute window is a suppressor, not an alert. Toggle is "Silence alerts" so it
        // doesn't echo the "Quiet hours" header directly above it (§4.3 one-home).
        SettingsSectionHeader("Quiet hours")
        ToggleRow(
            "Silence alerts",
            "Mute timer and recap notifications during the hours below",
            state.quietHoursEnabled, vm::setQuietHoursEnabled
        )
        if (state.quietHoursEnabled) {
            HourPickerRow("From", state.quietHoursStart, vm::setQuietHoursStart)
            HourPickerRow("Until", state.quietHoursEnd, vm::setQuietHoursEnd)
        }

        SectionResetRow(com.forge.app.data.prefs.SettingsSection.NOTIFICATIONS, vm)
    }
}

@Composable
internal fun ExercisePrefsPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
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

    // Search-first flat list: a search box + two independent filter dimensions — WHERE (a muscle,
    // or your custom moves) and STATUS (preferred/hidden) — over one scrolling list with muscle
    // sub-headers. Both combine, so "Chest + Preferred" or "Custom + Hidden" are one tap each.
    var query by remember { mutableStateOf("") }
    var scope by remember { mutableStateOf<PrefScope>(PrefScope.All) }
    var status by remember { mutableStateOf<Pref?>(null) }
    val q = query.trim()

    val visibleByMuscle = remember(byMuscle, q, scope, status, state.liked, state.disliked) {
        if (scope == PrefScope.Custom) emptyMap()
        else byMuscle
            .filterKeys { m -> (scope as? PrefScope.Muscle)?.let { it.m == m } ?: true }
            .mapValues { (_, defs) -> defs.filter { libVisible(it, q, status, state.liked, state.disliked) } }
            .filterValues { it.isNotEmpty() }
    }
    val visibleCustom = remember(state.customExercises, q, scope, status, state.liked, state.disliked) {
        state.customExercises.filter { customVisible(it, q, scope, status, state.liked, state.disliked) }
    }
    val nothingMatches = visibleByMuscle.isEmpty() && visibleCustom.isEmpty()

    Column(modifier.fillMaxSize()) {
        // The top bar never names the screen (§2) — the page opens with its own mono anchor.
        SettingsSectionHeader("Exercise likes", top = 12.dp)
        PrefSearchField(query, onQuery = { query = it })
        // Two compact selectors — WHERE (muscle / custom) and STATUS (preferred / hidden). A chip
        // per muscle turned into a wall that scrolled off screen; dropdowns keep both dimensions
        // one tap deep and always fully visible.
        val muscles = remember(byMuscle) { byMuscle.keys.toList() }
        Row(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            PrefSelector(
                value = when (val s = scope) {
                    PrefScope.All -> "Any muscle"
                    is PrefScope.Muscle -> s.m.displayName
                    PrefScope.Custom -> "Custom"
                },
                isDefault = scope == PrefScope.All,
                options = buildList {
                    add("Any muscle")
                    addAll(muscles.map { it.displayName })
                    if (state.customExercises.isNotEmpty()) add("Custom")
                },
                selectedIndex = when (val s = scope) {
                    PrefScope.All -> 0
                    is PrefScope.Muscle -> muscles.indexOf(s.m) + 1
                    PrefScope.Custom -> muscles.size + 1
                },
                modifier = Modifier.weight(1f)
            ) { i ->
                scope = when {
                    i == 0 -> PrefScope.All
                    i <= muscles.size -> PrefScope.Muscle(muscles[i - 1])
                    else -> PrefScope.Custom
                }
            }
            PrefSelector(
                value = when (status) {
                    Pref.PREFERRED -> "Preferred"
                    Pref.HIDDEN -> "Hidden"
                    else -> "Any status"
                },
                isDefault = status == null,
                options = listOf("Any status", "Preferred", "Hidden"),
                selectedIndex = when (status) {
                    Pref.PREFERRED -> 1
                    Pref.HIDDEN -> 2
                    else -> 0
                },
                modifier = Modifier.weight(1f)
            ) { i ->
                status = when (i) {
                    1 -> Pref.PREFERRED
                    2 -> Pref.HIDDEN
                    else -> null
                }
            }
        }
        Text(
            "Preferred movements appear more often in generated programs · Hidden ones are never picked.",
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 12.dp)
        )

        LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(bottom = 32.dp)) {
            visibleByMuscle.forEach { (m, defs) ->
                item("hdr-${m.code}") {
                    val likedN = defs.count { it.id in state.liked }
                    val dislikedN = defs.count { it.id in state.disliked }
                    PrefSectionHeader(m.displayName.uppercase(), prefSummary(likedN, dislikedN, defs.size))
                }
                items(defs, key = { "lib-${it.id}" }) { def ->
                    val pref = prefOf(def.id in state.liked, def.id in state.disliked)
                    ExercisePrefRow(
                        name = def.name,
                        subtitle = libSubtitle(def),
                        icon = com.forge.app.ui.common.ExerciseIcons.forEquipment(def.equipment),
                        pref = pref,
                        onSet = { applyPref(it, pref, setOf(def.id), vm) }
                    )
                }
            }
            if (visibleCustom.isNotEmpty()) {
                item("hdr-custom") {
                    PrefSectionHeader(
                        "CUSTOM",
                        "${visibleCustom.size} ${if (visibleCustom.size == 1) "exercise" else "exercises"}"
                    )
                }
                // Group id-sets are disjoint, so the smallest id is a unique, stable per-group key
                // (name+muscle could collide on malformed data and crash the list).
                items(visibleCustom, key = { "cus-${it.ids.minOrNull()}" }) { ref ->
                    val pref = prefOf(
                        liked = ref.ids.any { it in state.liked },
                        disliked = ref.ids.any { it in state.disliked }
                    )
                    ExercisePrefRow(
                        name = ref.name,
                        // Null muscle ⇒ the stored code was missing/unparseable; show a plain "Custom"
                        // label rather than fabricating a (wrong) muscle.
                        subtitle = ref.muscle?.let { "Custom · ${it.displayName}" } ?: "Custom",
                        icon = com.forge.app.ui.common.ExerciseIcons.Custom,
                        pref = pref,
                        onSet = { applyPref(it, pref, ref.ids, vm) }
                    )
                }
            }
            if (nothingMatches) {
                item("empty") {
                    Text(
                        when {
                            q.isNotEmpty() -> "No exercises match “$q”."
                            status != null || scope != PrefScope.All -> "Nothing matches these filters."
                            else -> "No exercises here yet."
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = muted,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 48.dp)
                    )
                }
            }
            // The post-swap dislike prompt toggle lives here too — it's about likes/dislikes.
            item("swap-toggle") {
                Spacer(Modifier.height(24.dp))
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

/** WHERE the Exercise likes list looks: the whole pool, one muscle, or the user's custom moves. */
private sealed interface PrefScope {
    data object All : PrefScope
    data class Muscle(val m: com.forge.app.program.MuscleGroup) : PrefScope
    data object Custom : PrefScope
}

/** The mutually-exclusive preference an exercise can carry (mirrors the liked/disliked data model). */
private enum class Pref(val label: String) {
    PREFERRED("Preferred"), NEUTRAL("Neutral"), HIDDEN("Hidden")
}

/** Collapse the two independent liked/disliked flags into the single 3-state preference the UI shows. */
private fun prefOf(liked: Boolean, disliked: Boolean): Pref = when {
    liked -> Pref.PREFERRED
    disliked -> Pref.HIDDEN
    else -> Pref.NEUTRAL
}

/**
 * Move a row from its [current] preference to [target] using the existing mutually-exclusive toggles
 * (liking clears dislike and vice-versa), so the data layer stays the source of truth. The toggles key
 * "on" off whether ANY id is set — matching how [prefOf] derives the row state — so this round-trips
 * cleanly for grouped custom exercises too.
 */
private fun applyPref(target: Pref, current: Pref, ids: Set<String>, vm: SettingsViewModel) {
    if (target == current) return
    when (target) {
        Pref.PREFERRED -> vm.toggleExercisesLiked(ids)      // adds liked, clears any dislike
        Pref.HIDDEN -> vm.toggleExercisesDisliked(ids)      // adds disliked, clears any like
        Pref.NEUTRAL -> when (current) {
            Pref.PREFERRED -> vm.toggleExercisesLiked(ids)  // toggles the like back off
            Pref.HIDDEN -> vm.toggleExercisesDisliked(ids)  // toggles the dislike back off
            Pref.NEUTRAL -> Unit
        }
    }
}

private fun matchesQuery(name: String, q: String): Boolean =
    q.isEmpty() || name.contains(q, ignoreCase = true)

/** Whether [status] (null = any) admits a row whose flags collapse to [pref]. */
private fun matchesStatus(status: Pref?, pref: Pref): Boolean = status == null || status == pref

private fun libVisible(
    def: com.forge.app.program.ExerciseDef,
    q: String,
    status: Pref?,
    liked: Set<String>,
    disliked: Set<String>
): Boolean {
    // Search reaches past the name into what the user actually thinks in: the muscle
    // ("chest") and the implement ("dumbbell") — GYMAP-13's findability complaint.
    val hit = matchesQuery(def.name, q) ||
        (q.isNotEmpty() && def.muscle.displayName.contains(q, ignoreCase = true)) ||
        (q.isNotEmpty() && def.equipment.any { it.display.contains(q, ignoreCase = true) })
    return hit && matchesStatus(status, prefOf(def.id in liked, def.id in disliked))
}

private fun customVisible(
    ref: com.forge.app.data.repo.CustomExerciseRef,
    q: String,
    scope: PrefScope,
    status: Pref?,
    liked: Set<String>,
    disliked: Set<String>
): Boolean {
    // A muscle scope includes custom moves OF that muscle — scoping to Chest should surface
    // your custom chest move next to the library's, not hide it behind the Custom chip.
    val inScope = when (scope) {
        PrefScope.All, PrefScope.Custom -> true
        is PrefScope.Muscle -> ref.muscle == scope.m
    }
    if (!inScope) return false
    val hit = matchesQuery(ref.name, q) ||
        (q.isNotEmpty() && ref.muscle?.displayName?.contains(q, ignoreCase = true) == true)
    return hit && matchesStatus(status, prefOf(ref.ids.any { it in liked }, ref.ids.any { it in disliked }))
}

/** Per-muscle header summary: preferred/hidden tallies, or a plain move count when neither is set. */
private fun prefSummary(likedN: Int, dislikedN: Int, total: Int): String =
    buildList {
        if (likedN > 0) add("$likedN preferred")
        if (dislikedN > 0) add("$dislikedN hidden")
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
            .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(8.dp))
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

/**
 * One filter dimension as a compact dropdown — a bordered field (same rounded-8 language as the
 * search box above it) showing the current pick, opening the [options] menu. [isDefault] = the
 * dimension is not filtering; the value renders muted and the border stays quiet, so an ACTIVE
 * filter is the visible exception (onBg text + border).
 */
@Composable
private fun PrefSelector(
    value: String,
    options: List<String>,
    selectedIndex: Int,
    isDefault: Boolean,
    modifier: Modifier = Modifier,
    onSelect: (Int) -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        Row(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(8.dp))
                .border(1.dp, if (isDefault) outline.copy(alpha = 0.35f) else onBg, RoundedCornerShape(8.dp))
                .clickableLabeled("Change filter") { open = true }
                .padding(horizontal = 12.dp, vertical = 9.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Text(
                value,
                style = MaterialTheme.typography.bodySmall,
                color = if (isDefault) muted else onBg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f)
            )
            Text("▾", style = MaterialTheme.typography.labelMedium, color = muted.copy(alpha = 0.65f))
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            options.forEachIndexed { i, label ->
                DropdownMenuItem(
                    text = {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            // A leading dot marks the active pick; a blank keeps the labels aligned.
                            Box(Modifier.width(8.dp)) {
                                if (i == selectedIndex) Text("•", color = onBg)
                            }
                            Text(
                                label,
                                color = if (i == selectedIndex) onBg else muted,
                                style = MaterialTheme.typography.bodyMedium
                            )
                        }
                    },
                    onClick = { onSelect(i); open = false }
                )
            }
        }
    }
}

// Air + the mono header ARE the separator (DESIGN §1/§7) — no hairline under the anchor.
@Composable
private fun PrefSectionHeader(title: String, summary: String) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 24.dp, end = 24.dp, top = 22.dp, bottom = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(title, style = MaterialTheme.typography.labelSmall, color = onBg)
        Text(summary, style = MaterialTheme.typography.labelSmall, color = muted)
    }
}

@Composable
private fun ExercisePrefRow(
    name: String,
    subtitle: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    pref: Pref,
    onSet: (Pref) -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    var menuOpen by remember { mutableStateOf(false) }

    Box {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickableLabeled("Set preference") { menuOpen = true }
                .padding(horizontal = 24.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Equipment glyph — quiet wayfinding (DESIGN §8), same muted tint as the subtitle.
            Icon(icon, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Text(name, style = MaterialTheme.typography.bodyMedium, color = onBg)
                Text(subtitle, style = MaterialTheme.typography.labelSmall, color = muted)
            }
            // Editorial status: the current preference as a quiet word + a caret hinting it's tappable.
            // Neutral renders its honest word, dimmest of the three so the set states stand out.
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                Text(
                    pref.label,
                    style = MaterialTheme.typography.labelMedium,
                    color = when (pref) {
                        Pref.PREFERRED -> onBg
                        Pref.HIDDEN -> muted
                        Pref.NEUTRAL -> muted.copy(alpha = 0.65f)
                    },
                    letterSpacing = 0.3.sp
                )
                Text("▾", style = MaterialTheme.typography.labelMedium, color = muted.copy(alpha = 0.65f))
            }
        }
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            Pref.entries.forEach { option ->
                DropdownMenuItem(
                    text = {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            // A leading dot marks the active state; a blank keeps the labels aligned.
                            Box(Modifier.width(8.dp)) {
                                if (option == pref) Text("•", color = onBg)
                            }
                            Text(
                                option.label,
                                color = if (option == pref) onBg else muted,
                                style = MaterialTheme.typography.bodyMedium
                            )
                        }
                    },
                    onClick = { onSet(option); menuOpen = false }
                )
            }
        }
    }
}
