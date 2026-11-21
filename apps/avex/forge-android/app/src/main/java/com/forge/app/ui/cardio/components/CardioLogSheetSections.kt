package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import com.forge.app.ui.common.window.AlertDialog
import androidx.compose.material3.DatePicker
import com.forge.app.ui.common.window.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SelectableDates
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import java.time.Instant
import java.time.ZoneId
import java.time.ZoneOffset

/**
 * The cardio date picker — no future days (a session can't have happened tomorrow, and a future date
 * would corrupt the "this week" counts, the cardio streak and the week aggregations). [onPicked] is
 * given the chosen calendar day with the entry's original time-of-day preserved.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CardioDatePickerDialog(dateMs: Long, onPicked: (Long) -> Unit, onDismiss: () -> Unit) {
    val zone = ZoneId.systemDefault()
    val maxDateMs = remember { System.currentTimeMillis() }
    // Material3 canonicalises DatePickerState to UTC midnight — combineDay below already reads the
    // RESULT back that way. The seed did not: it passed a local instant straight in, so for anyone
    // west of UTC the picker opened highlighting the PREVIOUS day. Since the result was converted
    // correctly, opening the picker and tapping OK without touching anything silently moved the
    // entry back a day. BodyweightLogSheet's picker already converts here; this one did not.
    val seedUtcMs = remember(dateMs) {
        Instant.ofEpochMilli(dateMs).atZone(zone).toLocalDate()
            .atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
    }
    // "Today" has to be measured as a calendar DAY in the user's own zone. Comparing a UTC-midnight
    // candidate against a local `now` meant users east of UTC could not select today until their
    // offset had elapsed (09:00 in Tokyo, midday in Auckland), while users west could select
    // tomorrow — the very thing this guard exists to prevent.
    val maxDayUtcMs = remember(maxDateMs) {
        Instant.ofEpochMilli(maxDateMs).atZone(zone).toLocalDate()
            .atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
    }
    val dpState = rememberDatePickerState(
        initialSelectedDateMillis = seedUtcMs,
        selectableDates = remember(maxDayUtcMs, maxDateMs) {
            object : SelectableDates {
                override fun isSelectableDate(utcTimeMillis: Long) = utcTimeMillis <= maxDayUtcMs
                override fun isSelectableYear(year: Int) =
                    year <= Instant.ofEpochMilli(maxDateMs).atZone(ZoneId.systemDefault()).year
            }
        }
    )
    DatePickerDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(onClick = {
                dpState.selectedDateMillis?.let { picked -> onPicked(combineDay(picked, dateMs)) }
                onDismiss()
            }) { Text("OK") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    ) {
        DatePicker(state = dpState)
    }
}

/**
 * The date picker returns a UTC-midnight millis for the chosen day; keep the time-of-day from
 * [keepTimeFromMs] (the entry's original time, or "now" for a new entry) so backdating only moves
 * the calendar day, not the clock.
 */
private fun combineDay(pickedUtcMidnightMs: Long, keepTimeFromMs: Long): Long {
    val zone = ZoneId.systemDefault()
    val day = Instant.ofEpochMilli(pickedUtcMidnightMs).atZone(ZoneOffset.UTC).toLocalDate()
    val time = Instant.ofEpochMilli(keepTimeFromMs).atZone(zone).toLocalTime()
    return day.atTime(time).atZone(zone).toInstant().toEpochMilli()
}

/**
 * The cardio start-time picker (GYMAP-33) — sets the time-of-day of the entry's timestamp (there is
 * no separate start-time column; the entry's [dateMs] already carries the clock). [onPicked] returns
 * the same calendar day with the chosen time. Honors Settings → Format → Clock, which defaults to
 * the phone's own 12/24-hour setting; it read the phone's setting directly until the 2026-09-26
 * audit, so a user who picked 24h in Avex still got an AM/PM dial here.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CardioTimePickerDialog(dateMs: Long, onPicked: (Long) -> Unit, onDismiss: () -> Unit) {
    val time = remember(dateMs) { Instant.ofEpochMilli(dateMs).atZone(ZoneId.systemDefault()).toLocalTime() }
    val tpState = rememberTimePickerState(
        initialHour = time.hour,
        initialMinute = time.minute,
        is24Hour = com.forge.app.ui.theme.LocalForgeSettings.current.timeFormat24h
    )
    // A session can't have started later than now. The date picker already stops future days; this
    // stops a future time TODAY, which would end the entry (and its Health Connect mirror) ahead of the clock.
    val nowMs = remember { System.currentTimeMillis() }
    val pickedMs = combineTime(tpState.hour, tpState.minute, dateMs)
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(enabled = pickedMs <= nowMs, onClick = {
                onPicked(pickedMs)
                onDismiss()
            }) { Text("OK") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
        text = {
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                TimePicker(state = tpState)
            }
        }
    )
}

/**
 * Keep the calendar day from [keepDayFromMs] and replace only the time-of-day with the picked
 * [hour]:[minute] — the inverse of [combineDay], so setting the start time never shifts the date.
 */
private fun combineTime(hour: Int, minute: Int, keepDayFromMs: Long): Long {
    val zone = ZoneId.systemDefault()
    val day = Instant.ofEpochMilli(keepDayFromMs).atZone(zone).toLocalDate()
    return day.atTime(hour, minute).atZone(zone).toInstant().toEpochMilli()
}
