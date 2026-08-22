package com.forge.app.ui.settings

import androidx.compose.animation.AnimatedVisibility
import com.forge.app.ui.common.clickableLabeled
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.notify.QuietWindow
import java.time.DayOfWeek
import java.time.format.TextStyle
import java.util.Locale

/**
 * The per-day quiet-hours editor (GYMAP-75) — one row per weekday showing its live window, tapping
 * expands the From/Until steppers for that day alone (accordion, one open at a time) so seven
 * windows never stack into a picker wall (DESIGN §3 settings: split a dense block, each row shows
 * its live value). Days follow the user's week-start preference; storage stays Monday-canonical.
 */
@Composable
internal fun QuietHoursDays(state: SettingsUiState, vm: SettingsViewModel) {
    val schedule = state.quietHoursSchedule
    val days = remember(state.firstDayMonday) { orderedDays(state.firstDayMonday) }
    var expanded by remember { mutableStateOf<DayOfWeek?>(null) }

    Column {
        days.forEach { day ->
            val window = schedule.windowFor(day)
            QuietDayRow(
                label = day.getDisplayName(TextStyle.SHORT, Locale.getDefault()),
                window = window,
                expanded = expanded == day,
                onToggle = { expanded = if (expanded == day) null else day }
            )
            AnimatedVisibility(visible = expanded == day) {
                Column {
                    HourPickerRow("From", window.start) { vm.setQuietWindow(day, it, window.end) }
                    HourPickerRow("Until", window.end) { vm.setQuietWindow(day, window.start, it) }
                }
            }
        }
    }
}

@Composable
private fun QuietDayRow(label: String, window: QuietWindow, expanded: Boolean, onToggle: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled("$label quiet hours", onClick = onToggle)
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        // Day label left / window value right — mirrors the From/Until rows it expands into, so the
        // two columns (muted labels, onBg values) read as one rhythm.
        Text(label, style = MaterialTheme.typography.bodyMedium, color = muted)
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                windowLabel(window),
                style = MaterialTheme.typography.bodyMedium,
                // An "off" day (a zero-length window) reads dim — it's the inactive state.
                color = if (window.isOff) muted else onBg
            )
            Text(if (expanded) "⌃" else "⌄", style = MaterialTheme.typography.bodyMedium, color = muted)
        }
    }
}

private fun hourText(h: Int) = "${h.toString().padStart(2, '0')}:00"

private fun windowLabel(w: QuietWindow) =
    if (w.isOff) "Off" else "${hourText(w.start)}–${hourText(w.end)}"

/** Weekday order for display only; when Sunday leads, storage still indexes Monday-first. */
private fun orderedDays(mondayFirst: Boolean): List<DayOfWeek> =
    if (mondayFirst) DayOfWeek.values().toList()
    else listOf(DayOfWeek.SUNDAY) + DayOfWeek.values().filter { it != DayOfWeek.SUNDAY }
