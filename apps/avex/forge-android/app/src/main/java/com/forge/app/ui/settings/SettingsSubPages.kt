@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.clickable
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
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp

private val TIMEZONE_OPTIONS = listOf(
    "America/Los_Angeles" to "PST −8",
    "America/Denver"      to "MST −7",
    "America/Chicago"     to "CST −6",
    "America/New_York"    to "EST −5",
    "America/Sao_Paulo"   to "BRT −3",
    "UTC"                 to "UTC ±0",
    "Europe/London"       to "GMT +0",
    "Europe/Paris"        to "CET +1",
    "Europe/Moscow"       to "MSK +3",
    "Asia/Kolkata"        to "IST +5:30",
    "Asia/Tokyo"          to "JST +9",
    "Australia/Sydney"    to "AEST +10"
)

@Composable
internal fun AppearancePage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxSize()) {
        ToggleRow("AMOLED pure black", "True black replaces dark background", state.amoledMode, vm::setAmoledMode)
        SectionDivider()
        ToggleRow("Compact set logging", "Denser set rows for experienced users", state.compactSetLogging, vm::setCompactSetLogging)
        SectionDivider()
        Spacer(Modifier.height(8.dp))
        AccentColorRow(state.accentColorHex, vm::setAccentColorHex)
        SectionDivider()
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
        item("weight") {
            ChipSection(
                "Weight unit",
                listOf("lb" to "lb", "kg" to "kg"),
                if (state.useKg) "kg" else "lb"
            ) { vm.setUseKg(it == "kg") }
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
    }
}

@Composable
internal fun SessionPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxSize()) {
        ToggleRow("Encouragement messages", "Motivational messages during sessions", state.showEncouragement, vm::setShowEncouragement)
        SectionDivider()
        ChipSection(
            "Haptic feedback",
            listOf("off" to "Off", "light" to "Light", "medium" to "Medium", "strong" to "Strong"),
            state.hapticStrength, vm::setHapticStrength
        )
        SectionDivider()
    }
}

@Composable
internal fun NotificationsPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxSize()) {
        ToggleRow("Quiet hours", "Suppress timer + recap notifications", state.quietHoursEnabled, vm::setQuietHoursEnabled)
        SectionDivider()
        if (state.quietHoursEnabled) {
            HourPickerRow("From", state.quietHoursStart, vm::setQuietHoursStart)
            HourPickerRow("Until", state.quietHoursEnd, vm::setQuietHoursEnd)
            SectionDivider()
        }
    }
}

@Composable
internal fun TilesPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val tileLabels = mapOf("gym" to "Gym", "cardio" to "Cardio", "trophies" to "Trophies")

    LazyColumn(modifier = modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 56.dp)) {
        item("order-header") { SubSectionLabel("Order") }
        itemsIndexed(state.overviewTileOrder) { idx, tileId ->
            TileOrderRow(
                label = tileLabels[tileId] ?: tileId,
                canMoveUp = idx > 0,
                canMoveDown = idx < state.overviewTileOrder.lastIndex,
                onMoveUp = {
                    val order = state.overviewTileOrder.toMutableList()
                    order.add(idx - 1, order.removeAt(idx))
                    vm.setOverviewTileOrder(order)
                },
                onMoveDown = {
                    val order = state.overviewTileOrder.toMutableList()
                    order.add(idx + 1, order.removeAt(idx))
                    vm.setOverviewTileOrder(order)
                }
            )
        }
        item("vis-header") {
            SectionDivider()
            SubSectionLabel("Visibility")
        }
        items(
            listOf("gym" to "Gym tile", "cardio" to "Cardio tile", "trophies" to "Trophies tile", "streak" to "Streak card", "deload" to "Deload banner"),
            key = { it.first }
        ) { (id, label) ->
            ToggleRow(
                label,
                if (id in state.hiddenOverviewTiles) "Hidden" else "Shown",
                id !in state.hiddenOverviewTiles
            ) { shown -> vm.setTileHidden(id, !shown) }
        }
    }
}

@Composable
internal fun EquipmentPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(modifier.fillMaxSize()) {
        Spacer(Modifier.height(16.dp))
        Text(
            "Swap suggestions will favour exercises you have access to.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(horizontal = 24.dp)
        )
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
                }
            }
        }
        Spacer(Modifier.height(16.dp))
        SectionDivider()
    }
}

@Composable
internal fun PrivacyPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxSize()) {
        ToggleRow("Privacy mode", "Blur app content in task switcher & screenshots", state.privacyMode, vm::setPrivacyMode)
        SectionDivider()
    }
}

// ─── LazyListScope helpers not in stdlib ─────────────────────────────────────

private fun androidx.compose.foundation.lazy.LazyListScope.items(
    items: List<Pair<String, String>>,
    key: (Pair<String, String>) -> Any,
    itemContent: @Composable (Pair<String, String>) -> Unit
) {
    // Prefix the key: the Tiles page renders an Order list and a Visibility list in the
    // same LazyColumn, both keyed by tile id — without namespacing they collide and crash.
    items.forEach { pair ->
        item("kv-${key(pair)}") { itemContent(pair) }
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.items(
    items: List<Pair<String, String>>,
    itemContent: @Composable (Pair<String, String>) -> Unit
) {
    items.forEach { pair ->
        item { itemContent(pair) }
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.itemsIndexed(
    items: List<String>,
    itemContent: @Composable (Int, String) -> Unit
) {
    items.forEachIndexed { idx, item ->
        item("order-$item") { itemContent(idx, item) }
    }
}
