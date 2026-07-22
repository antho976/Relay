package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SelectableDates
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.bounceClick
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
    onSave: (weightLb: Double, date: LocalDate, note: String?) -> Unit,
    onImport: () -> Unit,
    onDismiss: () -> Unit
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    // Stones (GYMAP-72) enters as a stone + pounds pair (the British idiom); kg/lb keep one decimal field.
    val stones = weightUnit == WeightUnit.ST
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val sheetState = rememberModalBottomSheetState()

    val today = remember { LocalDate.now() }
    var date by remember { mutableStateOf(today) }
    var showDatePicker by remember { mutableStateOf(false) }

    // The weigh-in already on the selected day (if any) — editing a past day shows what's there.
    val entryForDate = remember(entries, date) { entries.lastOrNull { it.dateKey == date.toString() } }
    // Seed the field from that day's entry, falling back to the latest weight (entries are oldest→newest).
    val seedLb = entryForDate?.weightLb ?: entries.lastOrNull()?.weightLb

    // Keyed on the day + seed + unit so switching date re-seeds from that day, a late flow emission
    // still lands the prefill, and flipping cm/kg re-seeds in the new unit instead of leaving a stale
    // value that fails validation and locks Save.
    var input by remember(seedLb, weightUnit, date) {
        mutableStateOf(if (stones) "" else seedLb?.let { weightInputValue(it, weightUnit) } ?: "")
    }
    // Stone + pounds fields, seeded by splitting the stored lb (whole-lb remainder).
    var stInput by remember(seedLb, weightUnit, date) {
        mutableStateOf(if (stones) seedLb?.let { (it.roundToInt() / 14).toString() } ?: "" else "")
    }
    var lbInput by remember(seedLb, weightUnit, date) {
        mutableStateOf(if (stones) seedLb?.let { (it.roundToInt() % 14).toString() } ?: "" else "")
    }
    // Keyed on the day's entry (not just the date) so a late flow emission — or a change while the
    // sheet is open — re-seeds the note like the weight field above, rather than leaving it blank and
    // then blanking the stored note on Save.
    var note by remember(entryForDate, date) { mutableStateOf(entryForDate?.note ?: "") }

    // Parsed lb — stones sums the two fields, kg/lb parse the single field; both clamp to the sane range.
    val parsed: Double? = if (stones) {
        val lb = (stInput.toIntOrNull() ?: 0) * 14.0 + (lbInput.toIntOrNull() ?: 0)
        if (stInput.isNotBlank() || lbInput.isNotBlank()) lb.takeIf { it in MIN_BODYWEIGHT_LB..MAX_BODYWEIGHT_LB } else null
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

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState) {
        Column(
            Modifier.fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .imePadding()
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text("Log bodyweight", style = MaterialTheme.typography.headlineSmall, color = onBg)
            // Accent mono = the tappable idiom (§5): tap to backdate to any past day.
            Text(
                dateLabel,
                style = MaterialTheme.typography.labelMedium,
                color = accent,
                modifier = Modifier.bounceClick { showDatePicker = true }.padding(vertical = 6.dp)
            )
            if (stones) {
                // Stone + pounds pair — the British compound entry. The two fields sum to lb on Save.
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.fillMaxWidth()) {
                    OutlinedTextField(
                        value = stInput,
                        onValueChange = { stInput = it.filter { ch -> ch.isDigit() }.take(2) },
                        label = { Text("Stone") },
                        singleLine = true,
                        isError = invalid,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        modifier = Modifier.weight(1f)
                    )
                    OutlinedTextField(
                        value = lbInput,
                        onValueChange = { lbInput = it.filter { ch -> ch.isDigit() }.take(2) },
                        label = { Text("Pounds") },
                        singleLine = true,
                        isError = invalid,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        modifier = Modifier.weight(1f)
                    )
                }
                Text(
                    supportingLine,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (invalid) MaterialTheme.colorScheme.error else muted
                )
            } else {
                OutlinedTextField(
                    value = input,
                    onValueChange = { v ->
                        // Digits + at most ONE decimal point: collapse any extra dots so '7.5.2' can't slip
                        // through and surface as a misleading out-of-range error.
                        val f = v.filter { ch -> ch.isDigit() || ch == '.' }
                        val dot = f.indexOf('.')
                        input = if (dot < 0) f else f.substring(0, dot + 1) + f.substring(dot + 1).replace(".", "")
                    },
                    label = { Text("Weight (${unitLabel(weightUnit)})") },
                    singleLine = true,
                    isError = invalid,
                    supportingText = { Text(supportingLine) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    modifier = Modifier.fillMaxWidth()
                )
            }
            OutlinedTextField(
                value = note,
                onValueChange = { note = it.take(140) },
                label = { Text("Note (optional)") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth()
            )
            ForgePrimaryCapsule(
                label = "Save",
                onClick = { parsed?.let { onSave(it, date, note) } },
                enabled = parsed != null,
                modifier = Modifier.fillMaxWidth()
            )
            // Import pulls the newest HC reading (dated by HC) — only meaningful on today, hidden while backdating.
            if (canImport && isToday) {
                ForgeOutlineCapsule(
                    label = "Import latest from Health Connect",
                    onClick = onImport,
                    modifier = Modifier.fillMaxWidth()
                )
            }
            message?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
            }
        }
    }

    if (showDatePicker) {
        // No future days — a weigh-in can't be recorded ahead, and a future date would distort the trend.
        val maxDateMs = remember { System.currentTimeMillis() }
        val dpState = rememberDatePickerState(
            initialSelectedDateMillis = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(),
            selectableDates = remember(maxDateMs) {
                object : SelectableDates {
                    override fun isSelectableDate(utcTimeMillis: Long) = utcTimeMillis <= maxDateMs
                    override fun isSelectableYear(year: Int) =
                        year <= Instant.ofEpochMilli(maxDateMs).atZone(ZoneId.systemDefault()).year
                }
            }
        )
        DatePickerDialog(
            onDismissRequest = { showDatePicker = false },
            confirmButton = {
                TextButton(onClick = {
                    // DatePicker returns UTC midnight — map that calendar day to a LocalDate.
                    dpState.selectedDateMillis?.let { picked ->
                        date = Instant.ofEpochMilli(picked).atZone(ZoneOffset.UTC).toLocalDate()
                    }
                    showDatePicker = false
                }) { Text("OK") }
            },
            dismissButton = { TextButton(onClick = { showDatePicker = false }) { Text("Cancel") } }
        ) {
            DatePicker(state = dpState)
        }
    }
}
