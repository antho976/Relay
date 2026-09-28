package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDefaults
import com.forge.app.ui.common.window.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.window.ModalBottomSheet
import androidx.compose.material3.SelectableDates
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.ForgeChromeButton
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.ui.onboarding.MAX_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.MIN_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.parseSaneBodyweightLb
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale
import kotlin.math.roundToInt
import com.forge.app.domain.units.filterDecimalInput

/**
 * Quick-log sheet for bodyweight — lives on the Profile's BODYWEIGHT section (moved from the old
 * Stats Body tab 2026-07-01), and is the only place to record bodyweight after onboarding. Type a
 * value (validated against the same sane range as onboarding), optionally backdate to any past day
 * (GYMAP-54 — the day's existing weigh-in re-seeds the fields so a missed day round-trips), and add
 * an optional note. When Health Connect is connected AND you're on today, pull the latest reading
 * with one tap. Saving closes the sheet; importing keeps it open so the result line is visible.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun BodyweightLogSheet(
    entries: List<BodyweightEntry>,
    canImport: Boolean,
    message: String?,
    /** The stored weigh-in for any day; [entries] only covers the recent window. */
    lookupDay: suspend (LocalDate) -> BodyweightEntry?,
    /** [note] is null when the note field was never touched: the day keeps its stored note. */
    onSave: (weightLb: Double, date: LocalDate, note: String?) -> Unit,
    onImport: () -> Unit,
    onDismiss: () -> Unit
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    // Stones (GYMAP-72) enters as a stone + pounds pair (the British idiom); kg/lb keep one decimal field.
    val stones = weightUnit == WeightUnit.ST
    val cs = MaterialTheme.colorScheme
    val onBg = cs.onBackground
    val muted = cs.onSurfaceVariant
    val sheetState = rememberModalBottomSheetState()
    val focus = LocalFocusManager.current
    val poundsFocus = remember { FocusRequester() }
    val noteFocus = remember { FocusRequester() }

    val today = remember { LocalDate.now() }
    var date by remember { mutableStateOf(today) }
    var showDatePicker by remember { mutableStateOf(false) }

    // The weigh-in already on the selected day (if any) — editing a past day shows what's there.
    // [entries] is only the recent window and the date picker reaches any past day, so a day outside
    // it is read directly: it used to look empty, seed the latest weight and a blank note, and Save
    // overwrote that day and erased its note (audit 2026-09-26, 03).
    var lookedUp by remember(date) { mutableStateOf<BodyweightEntry?>(null) }
    LaunchedEffect(date) {
        if (entries.none { it.dateKey == date.toString() }) lookedUp = lookupDay(date)
    }
    val entryForDate = remember(entries, date, lookedUp) {
        entries.lastOrNull { it.dateKey == date.toString() } ?: lookedUp?.takeIf { it.dateKey == date.toString() }
    }
    // Seed the field from that day's entry, falling back to the latest weight (entries are oldest→newest).
    val seedLb = entryForDate?.weightLb ?: entries.lastOrNull()?.weightLb

    // Keyed on the day + seed + unit so switching date re-seeds from that day, a late flow emission
    // still lands the prefill, and flipping cm/kg re-seeds in the new unit instead of leaving a stale
    // value that fails validation and locks Save.
    var input by remember(seedLb, weightUnit, date) {
        mutableStateOf(if (stones) "" else seedLb?.let { weightInputValue(it, weightUnit) } ?: "")
    }
    // Stone + pounds fields, seeded by splitting the stored lb (whole-lb remainder). The seed text
    // is kept so an UNCHANGED pair can be told from a re-typed one — see [parsed].
    val stSeed = if (stones) seedLb?.let { (it.roundToInt() / 14).toString() } ?: "" else ""
    val lbSeed = if (stones) seedLb?.let { (it.roundToInt() % 14).toString() } ?: "" else ""
    var stInput by remember(seedLb, weightUnit, date) { mutableStateOf(stSeed) }
    var lbInput by remember(seedLb, weightUnit, date) { mutableStateOf(lbSeed) }
    // Keyed on the day's entry (not just the date) so a late flow emission — or a change while the
    // sheet is open — re-seeds the note like the weight field above, rather than leaving it blank and
    // then blanking the stored note on Save.
    var note by remember(entryForDate, date) { mutableStateOf(entryForDate?.note ?: "") }
    // Untouched, Save leaves the day's note alone rather than trusting the seed to have landed.
    var noteTouched by remember(entryForDate, date) { mutableStateOf(false) }

    // Parsed lb — stones sums the two fields, kg/lb parse the single field; both clamp to the sane range.
    val parsed: Double? = if (stones) {
        if (seedLb != null && stInput == stSeed && lbInput == lbSeed) {
            // Neither field was changed, so hand back the STORED weight at full precision rather
            // than the whole-pound split of it. The stone/lb pair can only carry whole pounds, so
            // re-opening a 180.4 lb weigh-in imported from Health Connect and tapping Save without
            // editing rewrote it as 180.0 — over an import history that quantises the whole
            // bodyweight trend and can flip the sign of a small week-over-week delta. Same rule
            // SetRow applies to an untouched kg/stones set weight.
            seedLb.takeIf { it in MIN_BODYWEIGHT_LB..MAX_BODYWEIGHT_LB }
        } else {
            val lb = (stInput.toIntOrNull() ?: 0) * 14.0 + (lbInput.toIntOrNull() ?: 0)
            if (stInput.isNotBlank() || lbInput.isNotBlank()) lb.takeIf { it in MIN_BODYWEIGHT_LB..MAX_BODYWEIGHT_LB } else null
        }
    } else parseSaneBodyweightLb(input, weightUnit == WeightUnit.KG)  // non-stones branch is kg or lb
    val anyInput = if (stones) stInput.isNotBlank() || lbInput.isNotBlank() else input.isNotBlank()
    val invalid = anyInput && parsed == null
    val minDisp = toDisplayWeight(MIN_BODYWEIGHT_LB, weightUnit).roundToInt()
    val maxDisp = toDisplayWeight(MAX_BODYWEIGHT_LB, weightUnit).roundToInt()
    val rangeText = if (stones)
        "Enter ${formatWeight(MIN_BODYWEIGHT_LB, weightUnit)}–${formatWeight(MAX_BODYWEIGHT_LB, weightUnit)}."
        else "Enter $minDisp–$maxDisp ${unitLabel(weightUnit)}."

    val isToday = date == today
    val dateLabel = remember(date, today) {
        val md = date.format(
            DateTimeFormatter.ofPattern(if (date.year == today.year) "MMM d" else "MMM d, yyyy", Locale.getDefault())
        ).uppercase()
        if (isToday) "TODAY · $md"
        else "${date.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase()} · $md"
    }
    val supportingLine = when {
        invalid -> rangeText
        isToday -> "One entry per day, saving replaces today's."
        else -> "One entry per day, saving replaces this day's."
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        // §3/§5: a modal is a `surface` fill (#15161B, #080808 on AMOLED). M3's own default is
        // `surfaceContainerLow` — a tone this theme never defines, so it fell through to the
        // baseline dark palette's #1D1B20: lighter and purple-leaning, which is exactly the pale
        // grey slab that read wrong on the near-black page.
        containerColor = cs.surface
    ) {
        Column(
            Modifier.fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .imePadding()
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp)
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text("Log bodyweight", style = MaterialTheme.typography.headlineSmall, color = onBg)
                // The day is a filled capsule, the tappable idiom: tap to backdate to any past day.
                BodyLogDateCapsule(dateLabel) { showDatePicker = true }
            }
            // ONE group of inline rows: the reading, then its note. The explainer (or the range, when
            // the typed value is out of it) sits under the group as its footnote.
            ForgeGroupSection(label = null, footer = { BodyLogSupportingLine(supportingLine, invalid) }) {
                val noteRow: @Composable () -> Unit = {
                    ForgeFieldRow(
                        label = "Note",
                        value = note,
                        onValueChange = { note = it.take(140); noteTouched = true },
                        placeholder = "Optional",
                        keyboardOptions = KeyboardOptions(
                            capitalization = KeyboardCapitalization.Sentences,
                            imeAction = ImeAction.Done
                        ),
                        keyboardActions = KeyboardActions(onDone = { focus.clearFocus() }),
                        focusRequester = noteFocus
                    )
                }
                if (stones) {
                    // Stone + pounds pair — the British compound entry. The two rows sum to lb on Save.
                    ForgeRowGroup(
                        {
                            ForgeFieldRow(
                                label = "Stone",
                                value = stInput,
                                onValueChange = { stInput = it.filter { ch -> ch.isDigit() }.take(2) },
                                placeholder = "0",
                                suffix = "st",
                                isError = invalid,
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Next),
                                keyboardActions = KeyboardActions(onNext = { poundsFocus.requestFocus() })
                            )
                        },
                        {
                            ForgeFieldRow(
                                label = "Pounds",
                                value = lbInput,
                                onValueChange = { lbInput = it.filter { ch -> ch.isDigit() }.take(2) },
                                placeholder = "0",
                                suffix = "lb",
                                isError = invalid,
                                focusRequester = poundsFocus,
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Next),
                                keyboardActions = KeyboardActions(onNext = { noteFocus.requestFocus() })
                            )
                        },
                        noteRow
                    )
                } else {
                    ForgeRowGroup(
                        {
                            ForgeFieldRow(
                                label = "Weight",
                                value = input,
                                onValueChange = { v -> input = filterDecimalInput(v) },
                                placeholder = "0",
                                suffix = unitLabel(weightUnit),
                                isError = invalid,
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Next),
                                keyboardActions = KeyboardActions(onNext = { noteFocus.requestFocus() })
                            )
                        },
                        noteRow
                    )
                }
            }
            // §8: the page's actions group at the END — one filled do-it-now capsule ① and its
            // filled sidekick ②, nothing else.
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                ForgePrimaryCapsule(
                    label = "Save",
                    onClick = { parsed?.let { onSave(it, date, note.takeIf { noteTouched }) } },
                    enabled = parsed != null,
                    modifier = Modifier.fillMaxWidth()
                )
                // Import pulls the newest HC reading (dated by HC) — only meaningful on today, hidden while backdating.
                if (canImport && isToday) {
                    ForgeSecondaryCapsule(
                        label = "Import latest from Health Connect",
                        onClick = onImport,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }
            message?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
            }
        }
    }

    if (showDatePicker) {
        BodyLogDatePickerDialog(
            date = date,
            onPicked = { date = it },
            onDismiss = { showDatePicker = false }
        )
    }
}

// ── Shared body-log sheet furniture ───────────────────────────────────────────
// Used by BodyweightLogSheet + BodyFatLogSheet. Both are the same modal (§3): one dated reading,
// one group of inline fields, one Save. If a third sheet needs these, promote them to `ui/common` (§8).

/**
 * The sheet's day as a small filled capsule — the tap target that opens [BodyLogDatePickerDialog]
 * to backdate the reading. It was accent mono text, which read as a link rather than a control.
 */
@Composable
internal fun BodyLogDateCapsule(dateLabel: String, onClick: () -> Unit) {
    ForgeChromeButton(onClick = onClick, label = "Pick date") {
        Row(
            Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(dateLabel, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onBackground)
            Icon(
                Icons.Filled.KeyboardArrowDown,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(18.dp)
            )
        }
    }
}

/** The one-line explainer under a log sheet's group (§13) — caption rung (muted 0.7), full `error`
 *  when the typed value is out of range. */
@Composable
internal fun BodyLogSupportingLine(text: String, invalid: Boolean) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = if (invalid) MaterialTheme.colorScheme.error
        else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
    )
}

/**
 * The §5 tones for any `DatePickerDialog` in this app — pass to BOTH the dialog and the nested
 * `DatePicker`, since the dialog itself only consumes `containerColor` from them.
 *
 * M3's date-picker palette is authored for its baseline dark scheme, which this theme replaces: the
 * dialog lands on `surfaceContainerHigh` (#2B2930) with an `outlineVariant` (#49454F) rule under the
 * header, both markedly paler than the sheet that opened it. Pinned to the ladder instead.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun forgeDatePickerColors() = DatePickerDefaults.colors(
    containerColor = MaterialTheme.colorScheme.surface,
    headlineContentColor = MaterialTheme.colorScheme.onBackground,
    weekdayContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
    dividerColor = MaterialTheme.colorScheme.outline.copy(alpha = 0.25f)
)

/**
 * The backdating calendar shared by both body-log sheets. No future days — a reading can't be
 * recorded ahead, and a future date would distort the trend. The confirm/dismiss labels drop M3's
 * accent default (a muted navy at ~2:1 on this ground) for onBackground and muted.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun BodyLogDatePickerDialog(
    date: LocalDate,
    onPicked: (LocalDate) -> Unit,
    onDismiss: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val maxDateMs = remember { System.currentTimeMillis() }
    val maxDayUtcMs = remember(maxDateMs) {
        Instant.ofEpochMilli(maxDateMs).atZone(ZoneId.systemDefault()).toLocalDate()
            .atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
    }
    val dpState = rememberDatePickerState(
        initialSelectedDateMillis = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(),
        selectableDates = remember(maxDateMs) {
            object : SelectableDates {
                // Compare calendar DAYS in the user's zone. A UTC-midnight candidate against a
                // local `now` locked users east of UTC out of today until their offset elapsed,
                // and let users west of it pick tomorrow.
                override fun isSelectableDate(utcTimeMillis: Long) = utcTimeMillis <= maxDayUtcMs
                override fun isSelectableYear(year: Int) =
                    year <= Instant.ofEpochMilli(maxDateMs).atZone(ZoneId.systemDefault()).year
            }
        }
    )
    val pickerColors = forgeDatePickerColors()
    DatePickerDialog(
        onDismissRequest = onDismiss,
        colors = pickerColors,
        confirmButton = {
            TextButton(
                onClick = {
                    // DatePicker returns UTC midnight — map that calendar day to a LocalDate.
                    dpState.selectedDateMillis?.let { picked ->
                        onPicked(Instant.ofEpochMilli(picked).atZone(ZoneOffset.UTC).toLocalDate())
                    }
                    onDismiss()
                },
                colors = ButtonDefaults.textButtonColors(contentColor = cs.onBackground)
            ) { Text("OK") }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                colors = ButtonDefaults.textButtonColors(contentColor = cs.onSurfaceVariant)
            ) { Text("Cancel") }
        }
    ) {
        DatePicker(state = dpState, colors = pickerColors)
    }
}
