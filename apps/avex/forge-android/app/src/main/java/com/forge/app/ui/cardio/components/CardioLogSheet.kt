package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGroupCaption
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSlidingSegments
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioCondition
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CustomCardioType
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.elevationInputValue
import com.forge.app.domain.units.parseToKm
import com.forge.app.domain.units.parseToMeters
import com.forge.app.domain.units.storedUnlessEdited
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun CardioLogSheet(
    onDismiss: () -> Unit,
    onSave: (
        activity: CardioActivity,
        durationMin: Int,
        distanceKm: Double?,
        effort: CardioEffort?,
        restReason: CardioRestReason?,
        note: String?,
        dateMs: Long,
        intervalCount: Int?,
        hrZone: String?,
        inclinePct: Double?,
        laps: Int?,
        elevationM: Double?,
        conditions: Set<CardioCondition>
    ) -> Unit,
    /** Persist a just-created custom activity (GYMAP-37) — wired to the VM so it lands in DataStore
     *  and the picker/rows pick it up reactively. */
    onCreateCustom: (CustomCardioType) -> Unit = {},
    editing: CardioEntry? = null,
    /** Distance entry/pace unit — true = miles, false = km. The field stores km regardless. */
    useMiles: Boolean = false,
    /** The last-logged activity code (GYMAP-40) — seeds a NEW entry's activity instead of always
     *  defaulting to Run. Null (and ignored while editing) falls back to Run. */
    lastUsedType: String? = null
) {
    // Keyed on the edited entry's id so the form re-seeds if the sheet is ever reused for a different
    // entry without leaving composition — fields can't carry over from the previously-opened entry.
    val editKey = editing?.id
    // The selected activity — a built-in type or a user's custom one (GYMAP-37). Seeded by resolving
    // the edited entry's stored code against the custom list; a NEW entry seeds to the last-logged
    // activity (GYMAP-40), falling back to Run on a fresh install.
    val customTypes = com.forge.app.ui.cardio.LocalCardioTypes.current
    // EVERY field below is rememberSaveable, not remember (M-12). This form is a primary logging
    // flow whose Activity has no `configChanges`, so rotating or resizing recreates it: a plain
    // `remember` meant a half-filled new log came back blank and an edit silently reverted to the
    // stored row. Each is held as a saveable PRIMITIVE (a code, a text field, a flag) and the
    // domain object is derived from it, because a bundle can carry a String but not a sealed
    // CardioActivity or a Set<CardioCondition>.
    //
    // The activity: seeded from the edited entry's stored code, or for a NEW entry the last-logged
    // activity (GYMAP-40), falling back to Run on a fresh install.
    var typeCode by rememberSaveable(editKey) {
        mutableStateOf(editing?.type ?: lastUsedType?.takeIf { it.isNotBlank() } ?: CardioActivity.RUN.code)
    }
    // A custom activity created in this sheet is selected the instant it is minted, before the
    // DataStore write has flowed back into [customTypes] — without this it would resolve to Other
    // for that moment. Not saveable: by the time a recreation could happen the write has landed.
    var justCreated by remember(editKey) { mutableStateOf<CustomCardioType?>(null) }
    val type = justCreated?.takeIf { it.code == typeCode }?.let { CardioActivity.Custom(it) }
        ?: CardioActivity.resolve(typeCode, customTypes)
    // For a NEW entry, the last-used activity (GYMAP-40) can arrive from DataStore just AFTER the sheet
    // composes (lastUsedType null at first read → seeded to Run). Apply it once it lands — but never
    // after the user has picked, so a late emission can't clobber a manual choice. Editing ignores it.
    var typePicked by rememberSaveable(editKey) { mutableStateOf(false) }
    LaunchedEffect(editKey, lastUsedType) {
        if (editing == null && !typePicked && !lastUsedType.isNullOrBlank()) {
            typeCode = lastUsedType
        }
    }
    var showCreateCustom by rememberSaveable { mutableStateOf(false) }
    var durationText by rememberSaveable(editKey) { mutableStateOf(editing?.durationMin?.takeIf { it > 0 }?.toString() ?: "") }
    // Kept so save can tell an untouched field from an edit: the seed is rounded to one decimal.
    val distanceSeed = editing?.distanceKm?.let { distanceInputValue(it, useMiles) }
    var distanceText by rememberSaveable(editKey) { mutableStateOf(distanceSeed ?: "") }
    var effortCode by rememberSaveable(editKey) { mutableStateOf(editing?.effort) }
    val effort = CardioEffort.fromCode(effortCode)
    var restReasonCode by rememberSaveable(editKey) { mutableStateOf(editing?.restReason) }
    val restReason = CardioRestReason.fromCode(restReasonCode)
    var note by rememberSaveable(editKey) { mutableStateOf(editing?.note ?: "") }
    var intervalText by rememberSaveable(editKey) { mutableStateOf(editing?.intervalCount?.takeIf { it > 0 }?.toString() ?: "") }
    var hrZone by rememberSaveable(editKey) { mutableStateOf(editing?.hrZone) }
    // Per-type optional fields (GYMAP-38): incline % (treadmill/elliptical), laps (swim), elevation
    // gain (outdoor). Elevation seeds in the display unit; the field stores metres regardless.
    var inclineText by rememberSaveable(editKey) { mutableStateOf(editing?.inclinePct?.takeIf { it > 0 }?.let { plainDecimalInput(it) } ?: "") }
    var lapsText by rememberSaveable(editKey) { mutableStateOf(editing?.laps?.takeIf { it > 0 }?.toString() ?: "") }
    val elevationSeed = editing?.elevationM?.takeIf { it > 0 }?.let { elevationInputValue(it, useMiles) }
    var elevationText by rememberSaveable(editKey) { mutableStateOf(elevationSeed ?: "") }
    // Weather / environment tags (GYMAP-39), multi-select — held in the same comma-joined form the
    // row stores, so the bundle carries a String and the Set is decoded from it.
    var conditionsCode by rememberSaveable(editKey) { mutableStateOf(editing?.conditions) }
    val conditions = CardioCondition.decode(conditionsCode)
    var dateMs by rememberSaveable(editKey) { mutableStateOf(editing?.date ?: System.currentTimeMillis()) }
    var showDatePicker by rememberSaveable { mutableStateOf(false) }
    // The entry's start time (GYMAP-33) is the time-of-day of the same [dateMs] — no separate column.
    var showTimePicker by rememberSaveable { mutableStateOf(false) }
    // Optional details (effort / HR zone / intervals / per-type fields / conditions) start tucked away —
    // opened by default only when editing an entry that already has one, so they're never silently hidden.
    var moreOpen by rememberSaveable(editKey) {
        mutableStateOf(
            editing != null && (
                editing.effort != null || editing.hrZone != null || (editing.intervalCount ?: 0) > 0 ||
                    editing.inclinePct != null || editing.laps != null || editing.elevationM != null ||
                    !editing.conditions.isNullOrBlank()
                )
        )
    }

    // Accepts plain minutes ("90") or an H:MM clock value ("1:30" -> 90) — GYMAP-41.
    val durationInt = parseDurationMin(durationText)
    // The field holds a number in the display unit; convert to the canonical km we store + pass to onSave.
    // An untouched field keeps the stored km: re-parsing its one-decimal seed turned an adopted
    // 10.047 km into 10.0 on a note-only edit, and 5 km edited in miles mode into 4.989 (audit
    // 2026-09-26, 05). Elevation, seeded as a whole number, gets the same rule.
    val distanceKm = storedUnlessEdited(distanceText, distanceSeed, editing?.distanceKm) { parseToKm(it, useMiles) }
    val intervalInt = intervalText.toIntOrNull()
    // Per-type fields — raw parsed values; the VM keeps only the ones the chosen activity surfaces.
    val inclineValue = inclineText.toDoubleOrNull()
    val lapsValue = lapsText.toIntOrNull()
    val elevationValue = storedUnlessEdited(elevationText, elevationSeed, editing?.elevationM) { parseToMeters(it, useMiles) }
    val canSubmit = if (type.isRest) restReason != null else durationInt > 0
    val distanceFocus = remember { FocusRequester() }

    // "MON · JUL 6" — the year only appears once it differs from today's (backdating that far is rare).
    // Keyed on dateMs so the formatters aren't rebuilt on every recomposition (e.g. each keystroke).
    val dateHeader = remember(dateMs) {
        val d = Date(dateMs)
        val sameYear = SimpleDateFormat("yyyy", Locale.getDefault()).format(d) ==
            SimpleDateFormat("yyyy", Locale.getDefault()).format(Date())
        val day = SimpleDateFormat("EEE", Locale.getDefault()).format(d).uppercase().take(3)
        val date = SimpleDateFormat(if (sameYear) "MMM d" else "MMM d, yyyy", Locale.getDefault()).format(d).uppercase()
        "$day · $date"
    }
    // "7:24 AM" (GYMAP-33): the app's own 12/24-hour setting, matching the time picker below. It
    // followed the phone's setting, so with "Clock" set in Settings the header and picker disagreed.
    val use24h = com.forge.app.ui.theme.LocalForgeSettings.current.timeFormat24h
    val timeHeader = remember(dateMs, use24h) {
        SimpleDateFormat(com.forge.app.domain.units.clockPattern(use24h), Locale.getDefault())
            .format(Date(dateMs)).uppercase()
    }

    val focus = LocalFocusManager.current
    Scaffold(
        topBar = { ForgeTopBar(onBack = onDismiss) },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(bottom = 48.dp)
        ) {
            item("hero") {
                CardioLogHeroItem(
                    dateHeader = dateHeader,
                    timeHeader = timeHeader,
                    showTime = !type.isRest,
                    onPickDate = { showDatePicker = true },
                    onPickTime = { showTimePicker = true }
                )
            }

            item("type") {
                FormSection(label = "Activity", optional = false) {
                    ActivityDropdown(
                        selected = type,
                        onSelect = { typeCode = it.code; typePicked = true },
                        onAddCustom = { showCreateCustom = true }
                    )
                }
            }

            if (!type.isRest) {
                // Duration (required) + distance (optional) as one group of inline rows, with the
                // duration quick-picks as a member between them and the live pace under the group.
                item("metrics") {
                    val pace = pacePerUnit(durationInt, distanceKm, useMiles)
                    FormSection(
                        label = "Session",
                        optional = false,
                        footer = pace?.let { p ->
                            { ForgeGroupCaption("$p /${distanceUnitLabel(useMiles)}") }
                        }
                    ) {
                        val quick = listOf(20, 30, 45, 60)
                        ForgeRowGroup(
                            {
                                ForgeFieldRow(
                                    label = "Duration",
                                    value = durationText,
                                    onValueChange = { durationText = sanitizeDuration(it) },
                                    placeholder = "30",
                                    // Reflects how the typed value reads: plain minutes, or an H:MM clock (GYMAP-41).
                                    suffix = if (durationText.contains(':')) "h:mm" else "min",
                                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Next),
                                    keyboardActions = KeyboardActions(onNext = { distanceFocus.requestFocus() })
                                )
                            },
                            {
                                // §13 — the hot-path number gets its quick-picks; the field stays typeable.
                                Box(Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) {
                                    ForgeSlidingSegments(
                                        options = quick.map { "$it" },
                                        selectedIndex = quick.indexOfFirst { durationText == it.toString() },
                                        onSelect = { durationText = quick[it].toString() },
                                        modifier = Modifier.fillMaxWidth()
                                    )
                                }
                            },
                            {
                                ForgeFieldRow(
                                    label = "Distance",
                                    value = distanceText,
                                    onValueChange = { distanceText = sanitizeDecimal(it) },
                                    placeholder = "0",
                                    suffix = distanceUnitLabel(useMiles),
                                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
                                    keyboardActions = KeyboardActions(onDone = { focus.clearFocus() }),
                                    focusRequester = distanceFocus
                                )
                            }
                        )
                    }
                }

                // Optional details (effort / HR zone / intervals / per-type fields), collapsed behind "More".
                cardioMoreItems(
                    moreOpen = moreOpen,
                    onToggleMore = { moreOpen = !moreOpen },
                    activity = type,
                    effort = effort, onEffort = { effortCode = it?.code },
                    hrZone = hrZone, onHrZone = { hrZone = it },
                    intervalText = intervalText,
                    onIntervalChange = { intervalText = it.filter(Char::isDigit).take(3) },
                    inclineText = inclineText,
                    onInclineChange = { inclineText = sanitizeDecimal(it) },
                    lapsText = lapsText,
                    onLapsChange = { lapsText = it.filter(Char::isDigit).take(4) },
                    elevationText = elevationText,
                    onElevationChange = { elevationText = it.filter(Char::isDigit).take(5) },
                    conditions = conditions,
                    onToggleCondition = { c ->
                        conditionsCode = CardioCondition.encode(
                            if (c in conditions) conditions - c else conditions + c
                        )
                    },
                    useMiles = useMiles
                )
            } else {
                item("rest-reason") {
                    FormSection(label = "Rest", optional = false) {
                        val reasons = CardioRestReason.entries
                        ForgeSlidingSegments(
                            options = reasons.map { it.displayName },
                            selectedIndex = reasons.indexOf(restReason),
                            onSelect = { restReasonCode = reasons[it].code },
                            modifier = Modifier.fillMaxWidth()
                        )
                    }
                }
            }

            item("note") {
                FormSection(label = "Note", optional = true) {
                    FilledNoteField(note, { note = it.take(300) }, "jot a few words...")
                }
            }

            cardioSaveActionsItem(
                editing = editing != null,
                activity = type,
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
                        if (type.isHiit) intervalInt else null,
                        if (type.isRest) null else hrZone,
                        inclineValue,
                        lapsValue,
                        elevationValue,
                        if (type.isRest) emptySet() else conditions
                    )
                },
                onCancel = onDismiss
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

    if (showTimePicker) {
        CardioTimePickerDialog(
            dateMs = dateMs,
            onPicked = { dateMs = it },
            onDismiss = { showTimePicker = false }
        )
    }

    if (showCreateCustom) {
        CustomActivityDialog(
            initial = null,
            onDismiss = { showCreateCustom = false },
            onConfirm = { created ->
                onCreateCustom(created)      // persist so it lands in the list for next time
                // And select it now. The code is what survives a recreation; `justCreated` only
                // covers the moment before the persisted list flows back with it in.
                justCreated = created
                typeCode = created.code
                showCreateCustom = false
            }
        )
    }
}

