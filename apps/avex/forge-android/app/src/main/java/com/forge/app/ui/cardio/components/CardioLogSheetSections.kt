package com.forge.app.ui.cardio.components

import android.text.format.DateFormat
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioCondition
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioField
import com.forge.app.domain.units.elevationUnitLabel
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import java.time.Instant
import java.time.ZoneId
import java.time.ZoneOffset

/**
 * The optional cardio details (effort / HR zone / intervals / per-type fields / conditions), tucked
 * behind a "More" expander so the common case (just time + distance) stays short. Added as LazyColumn
 * items by [CardioLogSheet].
 */
@OptIn(ExperimentalLayoutApi::class)
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
    useMiles: Boolean,
    onBg: Color,
    bg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    item("more") {
        ExpanderHeader(
            label = "More",
            expanded = moreOpen,
            muted = muted, onBg = onBg, outline = outline,
            onToggle = onToggleMore
        )
    }
    if (!moreOpen) return

    item("effort") {
        FormSection(label = "Effort", optional = true, muted = muted, onBg = onBg, outline = outline) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                CardioEffort.entries.forEach { e ->
                    PillChip(
                        label = e.displayName.uppercase(),
                        selected = effort == e,
                        onClick = { onEffort(if (effort == e) null else e) },
                        onBg = onBg, bg = bg, muted = muted, outline = outline
                    )
                }
            }
        }
    }

    item("hr-zone") {
        FormSection(label = "HR zone", optional = true, muted = muted, onBg = onBg, outline = outline) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                (1..5).forEach { z ->
                    val code = z.toString()
                    PillChip(
                        label = "Z$z",
                        selected = hrZone == code,
                        onClick = { onHrZone(if (hrZone == code) null else code) },
                        onBg = onBg, bg = bg, muted = muted, outline = outline
                    )
                }
            }
        }
    }

    // Conditions (GYMAP-39) — the weather the session was done in, multi-select. Applies to any active
    // session (the whole More block is hidden on a rest day), so it's not gated by activity type.
    item("conditions") {
        FormSection(label = "Conditions", optional = true, muted = muted, onBg = onBg, outline = outline) {
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                CardioCondition.entries.forEach { c ->
                    PillChip(
                        label = c.displayName.uppercase(),
                        selected = c in conditions,
                        onClick = { onToggleCondition(c) },
                        onBg = onBg, bg = bg, muted = muted, outline = outline
                    )
                }
            }
        }
    }

    // Interval count — only meaningful for HIIT / interval work.
    if (activity.isHiit) {
        item("intervals") {
            FormSection(label = "Intervals", optional = true, muted = muted, onBg = onBg, outline = outline) {
                NumberInputRow(
                    value = intervalText,
                    onValueChange = onIntervalChange,
                    placeholder = "8",
                    unit = "intervals",
                    keyboardType = KeyboardType.Number,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
            }
        }
    }

    // Per-type fields (GYMAP-38) — each shows only for the activities it fits (belt grade, pool
    // laps, outdoor climb), so the form never carries a field the activity can't use.
    if (CardioField.INCLINE in activity.optionalFields) {
        item("incline") {
            FormSection(label = "Incline", optional = true, muted = muted, onBg = onBg, outline = outline) {
                NumberInputRow(
                    value = inclineText,
                    onValueChange = onInclineChange,
                    placeholder = "6",
                    unit = "%",
                    keyboardType = KeyboardType.Decimal,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
            }
        }
    }
    if (CardioField.LAPS in activity.optionalFields) {
        item("laps") {
            FormSection(label = "Laps", optional = true, muted = muted, onBg = onBg, outline = outline) {
                NumberInputRow(
                    value = lapsText,
                    onValueChange = onLapsChange,
                    placeholder = "20",
                    unit = "laps",
                    keyboardType = KeyboardType.Number,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
            }
        }
    }
    if (CardioField.ELEVATION in activity.optionalFields) {
        item("elevation") {
            FormSection(label = "Elevation gain", optional = true, muted = muted, onBg = onBg, outline = outline) {
                NumberInputRow(
                    value = elevationText,
                    onValueChange = onElevationChange,
                    placeholder = "120",
                    unit = elevationUnitLabel(useMiles),
                    keyboardType = KeyboardType.Number,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
            }
        }
    }
}

/** The save / cancel action row at the foot of the cardio log sheet. */
internal fun LazyListScope.cardioSaveActionsItem(
    editing: Boolean,
    activity: CardioActivity,
    canSubmit: Boolean,
    onSubmit: () -> Unit,
    onCancel: () -> Unit,
    onBg: Color,
    bg: Color,
    muted: Color
) {
    item("actions") {
        Spacer(Modifier.height(16.dp))
        Row(
            modifier = Modifier.padding(horizontal = 24.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(20.dp)
        ) {
            // The one do-it-now action — a filled light capsule (§8); disabled = dimmed, no border swap.
            ForgePrimaryCapsule(
                label = when {
                    editing -> "Save changes"
                    activity.isRest -> "Save rest day"
                    else -> "Save entry"
                },
                onClick = onSubmit,
                enabled = canSubmit
            )
            ForgeOutlineCapsule(label = "Cancel", onClick = onCancel)
        }
        Spacer(Modifier.height(16.dp))
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
 * the same calendar day with the chosen time. Honors the device's 12/24-hour format.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CardioTimePickerDialog(dateMs: Long, onPicked: (Long) -> Unit, onDismiss: () -> Unit) {
    val time = remember(dateMs) { Instant.ofEpochMilli(dateMs).atZone(ZoneId.systemDefault()).toLocalTime() }
    val tpState = rememberTimePickerState(
        initialHour = time.hour,
        initialMinute = time.minute,
        is24Hour = DateFormat.is24HourFormat(LocalContext.current)
    )
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(onClick = {
                onPicked(combineTime(tpState.hour, tpState.minute, dateMs))
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
