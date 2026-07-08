package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioCalorieEstimator
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.parseToKm
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun CardioLogSheet(
    onDismiss: () -> Unit,
    onSave: (
        type: CardioType,
        durationMin: Int,
        distanceKm: Double?,
        effort: CardioEffort?,
        restReason: CardioRestReason?,
        note: String?,
        dateMs: Long,
        intervalCount: Int?,
        hrZone: String?
    ) -> Unit,
    editing: CardioEntry? = null,
    /** Latest logged bodyweight (lb) — scales the live calorie estimate; null hides it. */
    bodyweightLb: Double? = null,
    /** Distance entry/pace unit — true = miles, false = km. The field stores km regardless. */
    useMiles: Boolean = false,
    /** Tapping the Avex wordmark — defaults to "go Home"; the cardio tab overrides it to close first. */
    onHome: () -> Unit = com.forge.app.ui.common.LocalGoHome.current
) {
    // Keyed on the edited entry's id so the form re-seeds if the sheet is ever reused for a different
    // entry without leaving composition — fields can't carry over from the previously-opened entry.
    val editKey = editing?.id
    var type by remember(editKey) { mutableStateOf(editing?.let { CardioType.fromCode(it.type) } ?: CardioType.RUN) }
    var durationText by remember(editKey) { mutableStateOf(editing?.durationMin?.takeIf { it > 0 }?.toString() ?: "") }
    var distanceText by remember(editKey) { mutableStateOf(editing?.distanceKm?.let { distanceInputValue(it, useMiles) } ?: "") }
    var effort by remember(editKey) { mutableStateOf(editing?.let { CardioEffort.fromCode(it.effort) }) }
    var restReason by remember(editKey) { mutableStateOf(editing?.let { CardioRestReason.fromCode(it.restReason) }) }
    var note by remember(editKey) { mutableStateOf(editing?.note ?: "") }
    var intervalText by remember(editKey) { mutableStateOf(editing?.intervalCount?.takeIf { it > 0 }?.toString() ?: "") }
    var hrZone by remember(editKey) { mutableStateOf(editing?.hrZone) }
    var dateMs by remember(editKey) { mutableStateOf(editing?.date ?: System.currentTimeMillis()) }
    var showDatePicker by remember { mutableStateOf(false) }
    // Optional details (effort / HR zone / intervals) start tucked away — opened by default only when
    // editing an entry that already has one of them, so they're never silently hidden.
    var moreOpen by remember(editKey) {
        mutableStateOf(editing != null && (editing.effort != null || editing.hrZone != null || (editing.intervalCount ?: 0) > 0))
    }

    val durationInt = durationText.toIntOrNull() ?: 0
    // The field holds a number in the display unit; convert to the canonical km we store + pass to onSave.
    val distanceKm = parseToKm(distanceText, useMiles)
    val intervalInt = intervalText.toIntOrNull()
    val canSubmit = if (type.isRest) restReason != null else durationInt > 0

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background
    val accent = MaterialTheme.colorScheme.primary

    val dateHeader = run {
        val d = Date(dateMs)
        val day = SimpleDateFormat("EEE", Locale.getDefault()).format(d).uppercase().take(3)
        val date = SimpleDateFormat("MMM d, yyyy", Locale.getDefault()).format(d).uppercase()
        "$day · $date"
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { com.forge.app.ui.common.ForgeWordmark(onClick = onHome) },
                navigationIcon = {
                    IconButton(onClick = onDismiss) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                actions = {
                    Text(
                        "NEW ENTRY",
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 2.sp,
                        color = muted,
                        modifier = Modifier.padding(end = 16.dp)
                    )
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(bottom = 48.dp)
        ) {
            item("hero") {
                CardioLogHeroItem(
                    dateHeader = dateHeader,
                    muted = muted, onBg = onBg, outline = outline,
                    onPickDate = { showDatePicker = true }
                )
            }

            item("type") {
                FormSection(label = "Activity", optional = false, muted = muted, onBg = onBg, outline = outline) {
                    ActivityDropdown(
                        selected = type,
                        onSelect = { type = it },
                        onBg = onBg, muted = muted, outline = outline
                    )
                }
            }

            if (!type.isRest) {
                // Duration (required) + distance (optional) sit side-by-side on one compact row, with
                // the live calorie + pace readouts underneath — far shorter than two full sections.
                item("metrics") {
                    Column(Modifier.padding(horizontal = 24.dp)) {
                        Spacer(Modifier.height(12.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                            CompactNumberField(
                                caption = "Duration",
                                value = durationText,
                                onValueChange = { durationText = it.filter(Char::isDigit).take(4) },
                                placeholder = "30",
                                unit = "min",
                                keyboardType = KeyboardType.Number,
                                onBg = onBg, muted = muted, accent = accent, outline = outline,
                                modifier = Modifier.weight(1f)
                            )
                            CompactNumberField(
                                caption = "Distance",
                                value = distanceText,
                                onValueChange = { distanceText = sanitizeDecimal(it) },
                                placeholder = "0",
                                unit = distanceUnitLabel(useMiles),
                                keyboardType = KeyboardType.Decimal,
                                onBg = onBg, muted = muted, accent = accent, outline = outline,
                                modifier = Modifier.weight(1f)
                            )
                        }
                        // Live readouts: calorie estimate (needs bodyweight) and pace (needs both fields).
                        val kcal = CardioCalorieEstimator.estimate(type, durationInt, effort, bodyweightLb)
                        val pace = pacePerUnit(durationInt, distanceKm, useMiles)
                        if (kcal != null || pace != null) {
                            Spacer(Modifier.height(8.dp))
                            val parts = listOfNotNull(kcal?.let { "≈ $it kcal" }, pace?.let { "$it /${distanceUnitLabel(useMiles)}" })
                            Text(parts.joinToString("  ·  "), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
                        }
                        Spacer(Modifier.height(10.dp))
                        HorizontalDivider(color = outline.copy(alpha = 0.15f))
                    }
                }

                // Optional details (effort / HR zone / intervals), collapsed behind a "More" expander.
                cardioMoreItems(
                    moreOpen = moreOpen,
                    onToggleMore = { moreOpen = !moreOpen },
                    type = type,
                    effort = effort, onEffort = { effort = it },
                    hrZone = hrZone, onHrZone = { hrZone = it },
                    intervalText = intervalText,
                    onIntervalChange = { intervalText = it.filter(Char::isDigit).take(3) },
                    onBg = onBg, bg = bg, muted = muted, accent = accent, outline = outline
                )
            } else {
                item("rest-reason") {
                    FormSection(label = "Rest", optional = false, muted = muted, onBg = onBg, outline = outline) {
                        FlowRow(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            CardioRestReason.entries.forEach { r ->
                                PillChip(
                                    label = r.displayName.uppercase(),
                                    selected = restReason == r,
                                    onClick = { restReason = r },
                                    onBg = onBg, bg = bg, muted = muted, outline = outline
                                )
                            }
                        }
                    }
                }
            }

            item("note") {
                FormSection(label = "Note", optional = true, muted = muted, onBg = onBg, outline = outline) {
                    BasicTextField(
                        value = note,
                        onValueChange = { note = it.take(300) },
                        textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
                        cursorBrush = SolidColor(accent),
                        minLines = 2,
                        maxLines = 5,
                        decorationBox = { inner ->
                            Box {
                                if (note.isEmpty()) {
                                    Text(
                                        "jot a few words...",
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = muted.copy(alpha = 0.45f),
                                        fontStyle = FontStyle.Italic
                                    )
                                }
                                inner()
                            }
                        },
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }

            cardioSaveActionsItem(
                editing = editing != null,
                type = type,
                canSubmit = canSubmit,
                onSubmit = {
                    onSave(
                        type,
                        if (type.isRest) 0 else durationInt,
                        if (type.isRest) null else distanceKm,
                        if (type.isRest) null else effort,
                        if (type.isRest) restReason else null,
                        note.ifBlank { null },
                        dateMs,
                        if (type == CardioType.HIIT) intervalInt else null,
                        if (type.isRest) null else hrZone
                    )
                },
                onCancel = onDismiss,
                onBg = onBg, muted = muted
            )
        }
    }

    if (showDatePicker) {
        CardioDatePickerDialog(
            dateMs = dateMs,
            onPicked = { dateMs = it },
            onDismiss = { showDatePicker = false }
        )
    }
}

