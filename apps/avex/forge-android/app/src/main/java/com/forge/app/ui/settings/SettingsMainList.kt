package com.forge.app.ui.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.ui.unit.sp

@Composable
internal fun MainList(
    state: SettingsUiState,
    displayRows: List<SettingsRow>,
    searchQuery: String,
    modifier: Modifier,
    onOpenPage: (SettingsPage) -> Unit,
    onOpenDataDialog: () -> Unit,
    onResetTarget: (ResetTarget) -> Unit
) {
    LazyColumn(modifier = modifier, contentPadding = PaddingValues(bottom = 56.dp)) {
        if (searchQuery.isBlank()) {
            displayRows.forEach { row ->
                item(row.page.name) {
                    SettingsNavRow(row.label, rowSubtitle(row.page, state)) { onOpenPage(row.page) }
                    SectionDivider()
                }
            }
            item("vacation") {
                SettingsNavRow("Holiday / Vacation", "Streak & deload counter pause") { }
                SectionDivider()
            }
            item("data") {
                SectionLabel("DATA")
                SettingsNavRow("Export data", "Sessions · weekly · full backup · PDF") { onOpenDataDialog() }
                SectionDivider()
            }
            item("reset") {
                SectionLabel("RESET")
                ResetTarget.entries.forEach { target ->
                    DestructiveRow(target.label, isFactory = target == ResetTarget.FACTORY) { onResetTarget(target) }
                }
                Spacer(Modifier.height(8.dp))
            }
        } else {
            val q = searchQuery.lowercase()
            val grouped = ALL_ITEMS
                .filter { q in it.name.lowercase() || q in it.tags }
                .groupBy { it.page }
                .entries
                .sortedBy { it.key.ordinal }

            if (grouped.isEmpty()) {
                item("empty") {
                    Text("No results", style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f),
                        fontStyle = FontStyle.Italic,
                        modifier = Modifier.padding(horizontal = 24.dp, vertical = 24.dp))
                }
            } else {
                grouped.forEach { (page, items) ->
                    item("group-${page.name}") {
                        SearchPageGroup(page = page, items = items, onClick = { onOpenPage(page) })
                    }
                }
            }
        }
    }
}

@Composable
internal fun SearchPageGroup(page: SettingsPage, items: List<SettingsItem>, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(top = 20.dp, bottom = 8.dp)) {
        Row(modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text(page.title.uppercase(), style = MaterialTheme.typography.labelSmall, color = onBg, letterSpacing = 1.5.sp)
            Text("→", style = MaterialTheme.typography.bodyMedium, color = muted)
        }
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        items.forEach { item ->
            Text(item.name, style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.75f),
                fontStyle = FontStyle.Italic, modifier = Modifier.padding(top = 8.dp, start = 4.dp))
        }
    }
}

internal fun rowSubtitle(page: SettingsPage, s: SettingsUiState): String = when (page) {
    SettingsPage.Appearance -> "AMOLED ${if (s.amoledMode) "on" else "off"} · compact ${if (s.compactSetLogging) "on" else "off"}"
    SettingsPage.Format -> "${if (s.useKg) "kg" else "lb"} · ${dateShort(s.dateFormat)} · ${if (s.timeFormat24h) "24h" else "12h"} · ${tzShort(s.timezone)}"
    SettingsPage.Session -> "Haptic: ${s.hapticStrength}"
    SettingsPage.Notifications -> if (s.quietHoursEnabled)
        "Quiet ${s.quietHoursStart.toString().padStart(2, '0')}:00–${s.quietHoursEnd.toString().padStart(2, '0')}:00"
    else "Off"
    SettingsPage.Tiles -> "${s.overviewTileOrder.count { it !in s.hiddenOverviewTiles }} of ${s.overviewTileOrder.size} visible"
    SettingsPage.Equipment -> if (s.availableEquipment.isEmpty()) "All equipment" else "${s.availableEquipment.size} selected"
    SettingsPage.Privacy -> if (s.privacyMode) "Screen blur on" else "Screen blur off"
}

internal fun dateShort(f: String) = when (f) {
    "dd/MM/yyyy" -> "05/01"
    "MM/dd/yyyy" -> "01/05"
    else -> "Jan 5"
}

internal fun tzShort(id: String) = when (id) {
    "America/Los_Angeles" -> "PST"; "America/Denver" -> "MST"; "America/Chicago" -> "CST"
    "America/New_York" -> "EST"; "America/Sao_Paulo" -> "BRT"; "UTC" -> "UTC"
    "Europe/London" -> "GMT"; "Europe/Paris" -> "CET"; "Europe/Moscow" -> "MSK"
    "Asia/Kolkata" -> "IST"; "Asia/Tokyo" -> "JST"; "Australia/Sydney" -> "AEST"
    else -> id.substringAfterLast("/")
}
