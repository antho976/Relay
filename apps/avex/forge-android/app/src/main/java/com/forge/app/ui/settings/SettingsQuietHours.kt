package com.forge.app.ui.settings

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.unit.dp
import com.forge.app.domain.notify.QuietWindow
import com.forge.app.domain.units.formatClockHour
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.DayOfWeek
import java.time.format.TextStyle

/**
 * The quiet-hours window (GYMAP-75). Most people keep one window every night, so that is the
 * default shape: From and Until for every day. "Different hours each day", above them, opens the seven days,
 * each row showing its window and opening its own From and Until. Days follow the week-start
 * preference; storage stays Monday-first. [enabled] follows the notification permission: while the
 * phone blocks Avex's alerts, the window is kept but cannot be edited.
 */
@Composable
internal fun QuietHoursDays(state: SettingsUiState, vm: SettingsViewModel, enabled: Boolean = true) {
    val schedule = state.quietHoursSchedule
    val days = remember(state.firstDayMonday) { orderedDays(state.firstDayMonday) }
    var perDay by rememberSaveable { mutableStateOf(!schedule.isUniform) }
    var expanded by remember { mutableStateOf<DayOfWeek?>(null) }
    val locale = currentLocale()
    val use24h = LocalForgeSettings.current.timeFormat24h

    // The switch leads what it governs, so turning it on adds rows below it and never moves it.
    SettingsSwitchRow(
        "Different hours each day",
        if (perDay) "Tap a day to change its window" else "Otherwise every night uses the window below",
        perDay,
        indent = 16.dp,
        enabled = enabled
    ) { on ->
        perDay = on
        // Back to one window: every day takes the first day's, so what shows is what applies.
        if (!on) {
            val w = schedule.windowFor(days.first())
            DayOfWeek.entries.forEach { vm.setQuietWindow(it, w.start, w.end) }
            expanded = null
        }
    }
    if (!perDay) {
        val w = schedule.windowFor(days.first())
        SettingsHourRow("From", w.start, use24h, enabled, indent = 16.dp) { h -> DayOfWeek.entries.forEach { vm.setQuietWindow(it, h, w.end) } }
        SettingsHourRow("Until", w.end, use24h, enabled, indent = 16.dp) { h -> DayOfWeek.entries.forEach { vm.setQuietWindow(it, w.start, h) } }
    } else {
        days.forEach { day ->
            val window = schedule.windowFor(day)
            val open = expanded == day
            val label = day.getDisplayName(TextStyle.FULL, locale)
            SettingsAdaptiveRow(
                title = label,
                indent = 16.dp,
                enabled = enabled,
                interaction = Modifier.clickableLabeled("$label quiet hours, ${windowLabel(window, use24h)}") {
                    expanded = if (open) null else day
                }
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        windowLabel(window, use24h),
                        style = MaterialTheme.typography.bodyMedium,
                        // A day with no window is the inactive state, so it reads quieter.
                        color = if (window.isOff) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface
                    )
                    val turn by animateFloatAsState(if (open) 180f else 0f, ForgeMotion.standardTween(), label = "chevron")
                    Icon(
                        Icons.Rounded.ExpandMore,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.rotate(turn)
                    )
                }
            }
            AnimatedVisibility(
                visible = open && enabled,
                enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
                exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween(ForgeMotion.DurationFast))
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    SettingsHourRow("From", window.start, use24h, enabled, indent = 32.dp) { vm.setQuietWindow(day, it, window.end) }
                    SettingsHourRow("Until", window.end, use24h, enabled, indent = 32.dp) { vm.setQuietWindow(day, window.start, it) }
                }
            }
        }
    }
}

/**
 * A time of day at hour precision (reminders and quiet hours store whole hours), picked from a
 * menu of the day's hours in the user's clock format.
 */
@Composable
internal fun SettingsHourRow(
    title: String,
    hour: Int,
    use24h: Boolean,
    enabled: Boolean = true,
    indent: androidx.compose.ui.unit.Dp = 0.dp,
    onChange: (Int) -> Unit
) {
    val hours = remember(use24h) { (0..23).map { formatClockHour(it, use24h) } }
    SettingsDropdownRow(
        title = title,
        value = hours[hour.coerceIn(0, 23)],
        options = hours,
        selectedIndex = hour.coerceIn(0, 23),
        indent = indent,
        enabled = enabled,
        onSelect = onChange
    )
}

/** "22:00–07:00", or "10 PM–7 AM" on a 12h clock (Settings → Format → Clock, 2026-09-26 audit). */
internal fun windowLabel(w: QuietWindow, use24h: Boolean) =
    if (w.isOff) "Off" else "${formatClockHour(w.start, use24h)}–${formatClockHour(w.end, use24h)}"

/** Weekday order for display only; when Sunday leads, storage still indexes Monday-first. */
private fun orderedDays(mondayFirst: Boolean): List<DayOfWeek> =
    if (mondayFirst) DayOfWeek.values().toList()
    else listOf(DayOfWeek.SUNDAY) + DayOfWeek.values().filter { it != DayOfWeek.SUNDAY }
