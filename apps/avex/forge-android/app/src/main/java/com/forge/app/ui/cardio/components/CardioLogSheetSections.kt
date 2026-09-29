package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
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
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioCondition
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioField
import com.forge.app.domain.units.elevationUnitLabel
import com.forge.app.ui.common.ForgeChoiceRow
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.ui.common.ForgeTileGrid
import java.time.Instant
import java.time.ZoneId
import java.time.ZoneOffset

/**
 * The optional cardio details (effort / HR zone / intervals / per-type fields / conditions), tucked
 * behind a "More" expander so the common case (just time + distance) stays short. Added as LazyColumn
 * items by [CardioLogSheet].
 *
 * Grouped-surface form (2026-09-27): effort and HR zone are label + segmented-control rows, and the
 * per-type numbers are inline field rows, all in ONE group; the weather tags are a connected tile
 * grid under it. A tapped-again segment clears its answer, as the pills did.
 */
internal fun LazyListScope.cardioMoreItems(
    moreOpen: Boolean,
    onToggleMore: () -> Unit,
    activity: CardioActivity,
    effort: CardioEffort?,
    onEffort: (CardioEffort?) -> Unit,
    hrZone: String?,
    onHrZone: (String?) -> Unit,
    intervalText: String,
    onIntervalChange: (String) -> Unit,
    inclineText: String,
    onInclineChange: (String) -> Unit,
    lapsText: String,
    onLapsChange: (String) -> Unit,
    elevationText: String,
    onElevationChange: (String) -> Unit,
    /** Weather / environment tags (GYMAP-39), multi-select — the currently-selected set + a per-tag toggle. */
    conditions: Set<CardioCondition>,
    onToggleCondition: (CardioCondition) -> Unit,
    /** Distance/elevation unit — true = miles + feet, false = km + metres. */
    useMiles: Boolean
) {
    item("more") {
        ExpanderHeader(label = "More", expanded = moreOpen, onToggle = onToggleMore)
    }
    if (!moreOpen) return

    item("details") {
        FormSection(label = "Details", optional = true) {
            val rows = buildList<@Composable () -> Unit> {
                add {
                    val efforts = CardioEffort.entries
                    ForgeChoiceRow("Effort", efforts.map { it.displayName }, efforts.indexOf(effort)) { i ->
                        onEffort(if (effort == efforts[i]) null else efforts[i])
                    }
                }
                add {
                    val zones = (1..5).map { it.toString() }
                    ForgeChoiceRow("HR zone", zones.map { "Z$it" }, zones.indexOf(hrZone)) { i ->
                        onHrZone(if (hrZone == zones[i]) null else zones[i])
                    }
                }
                // Interval count — only meaningful for HIIT / interval work.
                if (activity.isHiit) {
                    add { NumberRow("Intervals", intervalText, onIntervalChange, "8", "intervals", KeyboardType.Number) }
                }
                // Per-type fields (GYMAP-38) — each shows only for the activities it fits (belt
                // grade, pool laps, outdoor climb), so the form never carries a field the activity
                // can't use.
                if (CardioField.INCLINE in activity.optionalFields) {
                    add { NumberRow("Incline", inclineText, onInclineChange, "6", "%", KeyboardType.Decimal) }
                }
                if (CardioField.LAPS in activity.optionalFields) {
                    add { NumberRow("Laps", lapsText, onLapsChange, "20", "laps", KeyboardType.Number) }
                }
                if (CardioField.ELEVATION in activity.optionalFields) {
                    add {
                        NumberRow(
                            "Elevation gain", elevationText, onElevationChange, "120",
                            elevationUnitLabel(useMiles), KeyboardType.Number
                        )
                    }
                }
            }
            ForgeRowGroup(*rows.toTypedArray())
        }
    }

    // Conditions (GYMAP-39) — the weather the session was done in, multi-select. Applies to any active
    // session (the whole More block is hidden on a rest day), so it's not gated by activity type.
    item("conditions") {
        FormSection(label = "Conditions", optional = true) {
            ForgeTileGrid(CardioCondition.entries, cols = 4) { c, corners, modifier ->
                CardioTextTile(
                    label = c.displayName,
                    selected = c in conditions,
                    corners = corners,
                    onClick = { onToggleCondition(c) },
                    modifier = modifier
                )
            }
        }
    }
}

/** One optional number, typed inline at its row's end. */
@Composable
private fun NumberRow(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    unit: String,
    keyboardType: KeyboardType
) {
    val focus = LocalFocusManager.current
    ForgeFieldRow(
        label = label,
        value = value,
        onValueChange = onValueChange,
        placeholder = placeholder,
        suffix = unit,
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { focus.clearFocus() })
    )
}

/** The save / cancel action row at the foot of the cardio log sheet. */
internal fun LazyListScope.cardioSaveActionsItem(
    editing: Boolean,
    activity: CardioActivity,
    canSubmit: Boolean,
    onSubmit: () -> Unit,
    onCancel: () -> Unit
) {
    item("actions") {
        Row(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(top = 28.dp, bottom = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            // The one do-it-now action — a filled light capsule (§8); disabled = dimmed, no border swap.
            ForgePrimaryCapsule(
                label = when {
                    editing -> "Save changes"
                    activity.isRest -> "Save rest day"
                    else -> "Save entry"
                },
                onClick = onSubmit,
                enabled = canSubmit,
                modifier = Modifier.weight(1f)
            )
            ForgeSecondaryCapsule(label = "Cancel", onClick = onCancel, modifier = Modifier.weight(1f))
        }
    }
}

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
