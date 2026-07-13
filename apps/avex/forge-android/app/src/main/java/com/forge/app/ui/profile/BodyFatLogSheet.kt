package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.bounceClick
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale

/** Sane manual-entry bounds for a body-fat % (essential-fat floor to severe-obesity ceiling). */
internal const val MIN_BODY_FAT_PCT = 3.0
internal const val MAX_BODY_FAT_PCT = 65.0

/** Parse a typed body-fat % to a value inside the sane range, or null if blank / out of range. */
internal fun parseSaneBodyFat(input: String): Double? =
    input.trim().toDoubleOrNull()?.takeIf { it in MIN_BODY_FAT_PCT..MAX_BODY_FAT_PCT }

/**
 * Quick-log sheet for body fat % (GYMAP-62) — the sibling of [BodyweightLogSheet], lives on the
 * Profile's BODY FAT section. Type a percentage (validated against a sane range), optionally
 * backdate to any past day (the day's existing reading re-seeds the field so a missed day
 * round-trips), and when Health Connect is connected AND you're on today, pull the latest reading
 * with one tap. Saving closes the sheet; importing keeps it open so the result line is visible.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun BodyFatLogSheet(
    entries: List<BodyFatEntry>,
    canImport: Boolean,
    message: String?,
    onSave: (percent: Double, date: LocalDate) -> Unit,
    onImport: () -> Unit,
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val sheetState = rememberModalBottomSheetState()

    val today = remember { LocalDate.now() }
    var date by remember { mutableStateOf(today) }
    var showDatePicker by remember { mutableStateOf(false) }

    // The reading already on the selected day (if any) — editing a past day shows what's there.
    val entryForDate = remember(entries, date) { entries.lastOrNull { it.dateKey == date.toString() } }
    // Seed the field from that day's entry, falling back to the latest reading (entries are oldest→newest).
    val seed = entryForDate?.percent ?: entries.lastOrNull()?.percent

    // Keyed on the day + seed so switching date re-seeds from that day, and a late flow emission still
    // lands the prefill instead of leaving a stale value that fails validation and locks Save.
    var input by remember(seed, date) {
        mutableStateOf(seed?.let { "%.1f".format(it) } ?: "")
    }

    val parsed = parseSaneBodyFat(input)
    val invalid = input.isNotBlank() && parsed == null

    val isToday = date == today
    val dateLabel = remember(date, today) {
        val md = date.format(
            DateTimeFormatter.ofPattern(if (date.year == today.year) "MMM d" else "MMM d, yyyy", Locale.getDefault())
        ).uppercase()
        if (isToday) "TODAY · $md"
        else "${date.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase()} · $md"
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
            Text("Log body fat", style = MaterialTheme.typography.headlineSmall, color = onBg)
            // Accent mono = the tappable idiom (§5): tap to backdate to any past day.
            Text(
                dateLabel,
                style = MaterialTheme.typography.labelMedium,
                color = accent,
                modifier = Modifier.bounceClick { showDatePicker = true }.padding(vertical = 6.dp)
            )
            OutlinedTextField(
                value = input,
                onValueChange = { v ->
                    // Digits + at most ONE decimal point.
                    val f = v.filter { ch -> ch.isDigit() || ch == '.' }
                    val dot = f.indexOf('.')
                    input = if (dot < 0) f else f.substring(0, dot + 1) + f.substring(dot + 1).replace(".", "")
                },
                label = { Text("Body fat (%)") },
                singleLine = true,
                isError = invalid,
                supportingText = {
                    Text(
                        when {
                            invalid -> "Enter ${MIN_BODY_FAT_PCT.toInt()}–${MAX_BODY_FAT_PCT.toInt()}%."
                            isToday -> "One entry per day, saving replaces today's."
                            else -> "One entry per day, saving replaces this day's."
                        }
                    )
                },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                modifier = Modifier.fillMaxWidth()
            )
            ForgePrimaryCapsule(
                label = "Save",
                onClick = { parsed?.let { onSave(it, date) } },
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
        // No future days — a reading can't be recorded ahead, and a future date would distort the trend.
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
