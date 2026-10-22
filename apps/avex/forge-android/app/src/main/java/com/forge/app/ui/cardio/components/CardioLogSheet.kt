package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.health.HcExerciseTypes
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioCondition
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioField
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CustomCardioType
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.health.WatchWorkout
import com.forge.app.domain.units.distanceInputValue
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.elevationInputValue
import com.forge.app.domain.units.elevationUnitLabel
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.parseToKm
import com.forge.app.domain.units.parseToMeters
import com.forge.app.domain.units.storedUnlessEdited
import com.forge.app.ui.cardio.CardioChange
import com.forge.app.ui.cardio.CardioCorner
import com.forge.app.ui.cardio.CardioOutlineChip
import com.forge.app.ui.cardio.LocalCardioTypes
import com.forge.app.ui.cardio.cardioFigure
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.common.window.DropdownMenu
import com.forge.app.ui.common.window.ModalBottomSheet
import com.forge.app.ui.theme.CardioOpenColors
import com.forge.app.ui.theme.CardioOpenType
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.openOnAccentLarge
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/** What the sheet hands back on Save. */
typealias CardioSaveHandler = (
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
) -> Unit

/**
 * The log as a sheet (redesign 2026-10-21, "Log cardio") over whatever opened it: the hub, or a
 * session's page for an edit. Back and a swipe down dismiss it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CardioLogSheetHost(
    onDismiss: () -> Unit,
    onSave: CardioSaveHandler,
    onCreateCustom: (CustomCardioType) -> Unit = {},
    editing: CardioEntry? = null,
    useMiles: Boolean = false,
    lastUsedType: String? = null,
    recentTypes: List<String> = emptyList(),
    watchSessions: List<WatchWorkout> = emptyList(),
    onHideWatchSessions: () -> Unit = {},
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(topStart = 20.dp, topEnd = 20.dp),
        dragHandle = {
            Box(Modifier.padding(top = 12.dp).width(36.dp).height(4.dp).clip(RoundedCornerShape(2.dp)).background(OpenColors.Rule))
        }
    ) {
        CardioLogSheet(
            onDismiss = onDismiss,
            onSave = onSave,
            onCreateCustom = onCreateCustom,
            editing = editing,
            useMiles = useMiles,
            lastUsedType = lastUsedType,
            recentTypes = recentTypes,
            watchSessions = watchSessions,
            onHideWatchSessions = onHideWatchSessions,
            modifier = Modifier.navigationBarsPadding()
        )
    }
}

/**
 * The log sheet's body: the serif title; what the watch picked up, each with Use this; Today and the
 * start time; the activity as tiles (the recent ones, then More for every activity and a custom
 * one); the duration as a typed serif value with 20 / 30 / 45 / 60 quick picks; the distance; a
 * collapsed row for effort, heart rate, conditions and the note; and the red Save named for the
 * activity. A rest day trades the duration and distance for its reason.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CardioLogSheet(
    onDismiss: () -> Unit,
    onSave: CardioSaveHandler,
    /** Persist a just-created custom activity (GYMAP-37). */
    onCreateCustom: (CustomCardioType) -> Unit = {},
    editing: CardioEntry? = null,
    /** Distance entry/pace unit — true = miles, false = km. The field stores km regardless. */
    useMiles: Boolean = false,
    /** The last-logged activity code (GYMAP-40) — seeds a NEW entry's activity. */
    lastUsedType: String? = null,
    /** The most recently logged activities, newest first — the tiles before More. */
    recentTypes: List<String> = emptyList(),
    /** Watch workouts with no entry yet, offered on top with Use this (W5). */
    watchSessions: List<WatchWorkout> = emptyList(),
    onHideWatchSessions: () -> Unit = {},
    modifier: Modifier = Modifier,
    /** The clock; tests pin it so a golden never shows the time it was recorded at. */
    now: () -> Long = System::currentTimeMillis,
) {
    val editKey = editing?.id
    val customTypes = LocalCardioTypes.current
    // EVERY field is rememberSaveable (M-12): a rotation recreates the Activity, and a half-filled
    // log must come back as it was. Each is a saveable primitive; the domain objects derive from them.
    var typeCode by rememberSaveable(editKey) {
        mutableStateOf(editing?.type ?: lastUsedType?.takeIf { it.isNotBlank() } ?: CardioActivity.RUN.code)
    }
    var justCreated by remember(editKey) { mutableStateOf<CustomCardioType?>(null) }
    val type = justCreated?.takeIf { it.code == typeCode }?.let { CardioActivity.Custom(it) }
        ?: CardioActivity.resolve(typeCode, customTypes)
    // The last-used activity can land from DataStore just after the sheet composes; apply it once,
    // but never over a pick the user already made.
    var typePicked by rememberSaveable(editKey) { mutableStateOf(false) }
    LaunchedEffect(editKey, lastUsedType) {
        if (editing == null && !typePicked && !lastUsedType.isNullOrBlank()) typeCode = lastUsedType
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
    var inclineText by rememberSaveable(editKey) { mutableStateOf(editing?.inclinePct?.takeIf { it > 0 }?.let { plainDecimalInput(it) } ?: "") }
    var lapsText by rememberSaveable(editKey) { mutableStateOf(editing?.laps?.takeIf { it > 0 }?.toString() ?: "") }
    val elevationSeed = editing?.elevationM?.takeIf { it > 0 }?.let { elevationInputValue(it, useMiles) }
    var elevationText by rememberSaveable(editKey) { mutableStateOf(elevationSeed ?: "") }
    var conditionsCode by rememberSaveable(editKey) { mutableStateOf(editing?.conditions) }
    val conditions = CardioCondition.decode(conditionsCode)
    // A NEW entry's start defaults to when the sheet opened MINUS its duration, so a session logged
    // right after finishing does not end in the future.
    val openedAtMs = rememberSaveable(editKey) { editing?.date ?: now() }
    var dateMs by rememberSaveable(editKey) { mutableStateOf(openedAtMs) }
    var timePicked by rememberSaveable(editKey) { mutableStateOf(false) }
    var datePicked by rememberSaveable(editKey) { mutableStateOf(false) }
    var showDatePicker by rememberSaveable { mutableStateOf(false) }
    var showTimePicker by rememberSaveable { mutableStateOf(false) }
    // The watch session this log was filled from, if any.
    var usedWatchId by rememberSaveable(editKey) { mutableStateOf<String?>(null) }
    // The optional details start tucked away, open only when an edited entry already has one.
    var moreOpen by rememberSaveable(editKey) {
        mutableStateOf(
            editing != null && (
                editing.effort != null || editing.hrZone != null || (editing.intervalCount ?: 0) > 0 ||
                    editing.inclinePct != null || editing.laps != null || editing.elevationM != null ||
                    !editing.conditions.isNullOrBlank() || !editing.note.isNullOrBlank()
                )
        )
    }

    // Plain minutes ("90") or an H:MM clock value ("1:30" -> 90) — GYMAP-41.
    val durationInt = parseDurationMin(durationText).coerceAtMost(com.forge.app.data.importer.ImportBounds.MAX_CARDIO_MINUTES)
    // An untouched field keeps the stored km, so a note-only edit never re-rounds a distance.
    val distanceKm = storedUnlessEdited(distanceText, distanceSeed, editing?.distanceKm) { parseToKm(it, useMiles) }
        ?.coerceAtMost(com.forge.app.data.importer.ImportBounds.MAX_CARDIO_DISTANCE_KM)
    val elevationValue = storedUnlessEdited(elevationText, elevationSeed, editing?.elevationM) { parseToMeters(it, useMiles) }
    val canSubmit = if (type.isRest) restReason != null else durationInt > 0
    val distanceFocus = remember { FocusRequester() }
    val focus = LocalFocusManager.current

    // The start actually saved and shown: a never-picked time on a new entry counts back from when
    // the sheet opened, held to a day the user picked.
    val startMs = if (editing == null && !timePicked) {
        val counted = minOf(dateMs, openedAtMs - (if (type.isRest) 0 else durationInt) * 60_000L)
        if (datePicked) {
            val zone = ZoneId.systemDefault()
            counted.coerceAtLeast(Instant.ofEpochMilli(dateMs).atZone(zone).toLocalDate().atStartOfDay(zone).toInstant().toEpochMilli())
        } else counted
    } else dateMs

    val locale = currentLocale()
    val use24h = LocalForgeSettings.current.timeFormat24h
    val dayChip = remember(startMs, locale) { dayChipLabel(startMs, now(), locale) }
    val timeChip = remember(startMs, use24h, locale) { clockLabel(startMs, use24h, locale) }

    fun useWatch(w: WatchWorkout) {
        typeCode = HcExerciseTypes.toCardioCode(w.exerciseType)
        typePicked = true
        durationText = w.durationMin.takeIf { it > 0 }?.toString() ?: durationText
        distanceText = w.distanceKm?.let { distanceInputValue(it, useMiles) } ?: ""
        dateMs = w.startMs
        timePicked = true
        datePicked = true
        usedWatchId = w.recordId
    }

    Column(
        modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(start = 24.dp, end = 24.dp, top = 18.dp, bottom = 28.dp)
    ) {
        Text(
            if (editing != null && editing.id != 0L) "Edit ${type.displayName.lowercase()}" else "Log cardio",
            style = OpenType.figureMS,
            color = MaterialTheme.colorScheme.onBackground
        )
        if (watchSessions.isNotEmpty()) {
            Spacer(Modifier.height(16.dp))
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                watchSessions.forEach { w ->
                    WatchSessionRow(w, used = usedWatchId == w.recordId, useMiles = useMiles, locale = locale, use24h = use24h, nowMs = now()) { useWatch(w) }
                }
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                Box(
                    Modifier
                        .touchSlop(vertical = 12.dp)
                        .clickableLabeled("Hide your watch's sessions", onClick = onHideWatchSessions)
                        .padding(vertical = 12.dp)
                ) { OpenLabel("Hide these", style = OpenType.monoXS) }
            }
        } else {
            Spacer(Modifier.height(16.dp))
        }

        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(0.dp)) {
            CardioOutlineChip(dayChip, onClick = { showDatePicker = true }, label = "Day: $dayChip. Change the day")
            if (!type.isRest) CardioOutlineChip(timeChip, onClick = { showTimePicker = true }, label = "Start: $timeChip. Change the start time")
        }

        Spacer(Modifier.height(20.dp))
        OpenLabel("Activity")
        Spacer(Modifier.height(4.dp))
        ActivityTiles(
            selected = type,
            recentTypes = recentTypes,
            onSelect = { typeCode = it.code; typePicked = true },
            onAddCustom = { showCreateCustom = true }
        )

        if (!type.isRest) {
            Spacer(Modifier.height(20.dp))
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                OpenLabel("Duration", Modifier.weight(1f))
                DurationField(
                    value = durationText,
                    onValueChange = { durationText = capDurationText(sanitizeDuration(it)) },
                    onNext = { distanceFocus.requestFocus() }
                )
            }
            Spacer(Modifier.height(4.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf(20, 30, 45, 60).forEach { m ->
                    CardioOutlineChip(
                        "$m",
                        onClick = { durationText = "$m" },
                        selected = durationText == "$m",
                        height = 40.dp,
                        style = CardioOpenType.pick,
                        label = "$m minutes",
                        role = Role.RadioButton,
                        centered = true,
                        modifier = Modifier.weight(1f)
                    )
                }
            }
            Spacer(Modifier.height(12.dp))
            FieldRow(
                label = "Distance",
                value = distanceText,
                onValueChange = { distanceText = capDistanceText(sanitizeDecimal(it), useMiles) },
                unit = distanceUnitLabel(useMiles),
                keyboardType = KeyboardType.Decimal,
                focusRequester = distanceFocus,
                onDone = { focus.clearFocus() }
            )
            pacePerUnit(durationInt, distanceKm, useMiles)?.let { p ->
                Spacer(Modifier.height(6.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                    CardioChange("Pace $p /${distanceUnitLabel(useMiles)}", moved = false)
                }
            }
        } else {
            Spacer(Modifier.height(20.dp))
            OpenLabel("Reason")
            Spacer(Modifier.height(4.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                CardioRestReason.entries.forEach { r ->
                    CardioOutlineChip(
                        r.displayName, onClick = { restReasonCode = r.code }, selected = restReason == r,
                        height = 40.dp, role = Role.RadioButton
                    )
                }
            }
        }

        Spacer(Modifier.height(8.dp))
        MoreRow(
            text = if (type.isRest) "Note" else "Effort, heart rate, conditions, note",
            open = moreOpen,
            onToggle = { moreOpen = !moreOpen }
        )
        if (moreOpen) {
            if (!type.isRest) {
                ChipGroup("Effort", CardioEffort.entries.map { it.displayName }, CardioEffort.entries.indexOf(effort)) { i ->
                    val e = CardioEffort.entries[i]
                    effortCode = if (effort == e) null else e.code
                }
                val zones = (1..5).map { it.toString() }
                ChipGroup("Heart rate zone", zones.map { "Z$it" }, zones.indexOf(hrZone)) { i ->
                    hrZone = if (hrZone == zones[i]) null else zones[i]
                }
                if (type.isHiit) {
                    Spacer(Modifier.height(12.dp))
                    FieldRow("Intervals", intervalText, { intervalText = it.filter(Char::isDigit).take(3) }, "", KeyboardType.Number, onDone = { focus.clearFocus() })
                }
                if (CardioField.INCLINE in type.optionalFields) {
                    Spacer(Modifier.height(12.dp))
                    FieldRow("Incline", inclineText, { inclineText = sanitizeDecimal(it) }, "%", KeyboardType.Decimal, onDone = { focus.clearFocus() })
                }
                if (CardioField.LAPS in type.optionalFields) {
                    Spacer(Modifier.height(12.dp))
                    FieldRow("Laps", lapsText, { lapsText = it.filter(Char::isDigit).take(4) }, "laps", KeyboardType.Number, onDone = { focus.clearFocus() })
                }
                if (CardioField.ELEVATION in type.optionalFields) {
                    Spacer(Modifier.height(12.dp))
                    FieldRow("Elevation gain", elevationText, { elevationText = it.filter(Char::isDigit).take(5) }, elevationUnitLabel(useMiles), KeyboardType.Number, onDone = { focus.clearFocus() })
                }
                ChipGroup(
                    "Conditions",
                    CardioCondition.entries.map { it.displayName },
                    selected = -1,
                    isSelected = { CardioCondition.entries[it] in conditions },
                    role = Role.Checkbox
                ) { i ->
                    val c = CardioCondition.entries[i]
                    conditionsCode = CardioCondition.encode(if (c in conditions) conditions - c else conditions + c)
                }
            }
            Spacer(Modifier.height(16.dp))
            OpenLabel("Note")
            Spacer(Modifier.height(10.dp))
            NoteField(note) { note = it.take(300) }
        }

        Spacer(Modifier.height(24.dp))
        SaveBar(
            text = when {
                editing != null && editing.id != 0L -> "Save changes"
                type.isRest -> "Save rest day"
                else -> "Save ${type.displayName.lowercase()}"
            },
            enabled = canSubmit,
            onClick = {
                onSave(
                    type,
                    if (type.isRest) 0 else durationInt,
                    if (type.isRest) null else distanceKm,
                    if (type.isRest) null else effort,
                    if (type.isRest) restReason else null,
                    note.ifBlank { null },
                    startMs,
                    if (type.isHiit) intervalText.toIntOrNull() else null,
                    if (type.isRest) null else hrZone,
                    inclineText.toDoubleOrNull(),
                    lapsText.toIntOrNull(),
                    elevationValue,
                    if (type.isRest) emptySet() else conditions
                )
            }
        )
    }

    if (showDatePicker) {
        CardioDatePickerDialog(
            dateMs = startMs,
            onPicked = { dateMs = it; datePicked = true },
            onDismiss = { showDatePicker = false }
        )
    }
    if (showTimePicker) {
        CardioTimePickerDialog(
            dateMs = startMs,
            onPicked = { dateMs = it; timePicked = true },
            onDismiss = { showTimePicker = false }
        )
    }
    if (showCreateCustom) {
        CustomActivityDialog(
            initial = null,
            onDismiss = { showCreateCustom = false },
            onConfirm = { created ->
                onCreateCustom(created)
                justCreated = created
                typeCode = created.code
                typePicked = true
                showCreateCustom = false
            }
        )
    }
}

/** "Today", "Yesterday", or "Mon Oct 19" for the day chip. */
internal fun dayChipLabel(startMs: Long, nowMs: Long, locale: Locale, zone: ZoneId = ZoneId.systemDefault()): String {
    val d = Instant.ofEpochMilli(startMs).atZone(zone).toLocalDate()
    val today = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
    return when (d) {
        today -> "Today"
        today.minusDays(1) -> "Yesterday"
        else -> d.format(DateTimeFormatter.ofPattern(if (d.year == today.year) "EEE MMM d" else "MMM d, yyyy", locale))
    }
}

/** "7:40 am" (or "07:40" on a 24-hour clock). */
internal fun clockLabel(ms: Long, use24h: Boolean, locale: Locale, zone: ZoneId = ZoneId.systemDefault()): String =
    Instant.ofEpochMilli(ms).atZone(zone)
        .format(DateTimeFormatter.ofPattern(com.forge.app.domain.units.clockPattern(use24h), locale))
        .lowercase(locale)

/** The glyph a tile draws: the redesign's own for the three common activities, else the activity's. */
private fun tileIcon(a: CardioActivity): ImageVector = when (a.code) {
    CardioType.RUN.code -> OpenIcons.Navigation
    CardioType.WALK.code -> OpenIcons.MapPin
    CardioType.CYCLE.code -> OpenIcons.Compass
    else -> a.icon
}

/**
 * The activity tiles: the chosen one and the most recent (Run, Walk, Cycle on a fresh install),
 * then More, which lists every activity, your custom ones, and Add custom activity.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ActivityTiles(
    selected: CardioActivity,
    recentTypes: List<String>,
    onSelect: (CardioActivity) -> Unit,
    onAddCustom: () -> Unit,
) {
    val customs = LocalCardioTypes.current
    val base = (recentTypes + listOf(CardioType.RUN.code, CardioType.WALK.code, CardioType.CYCLE.code)).distinct().take(3)
    val codes = if (selected.code in base) base else listOf(selected.code) + base.take(2)
    var more by remember { mutableStateOf(false) }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        codes.forEach { code ->
            val a = if (code == selected.code) selected else CardioActivity.resolve(code, customs)
            CardioOutlineChip(
                a.displayName,
                onClick = { onSelect(a) },
                selected = a.code == selected.code,
                icon = tileIcon(a),
                height = 40.dp,
                role = Role.RadioButton
            )
        }
        Box {
            CardioOutlineChip("More", onClick = { more = true }, icon = OpenIcons.ChevronDown, height = 40.dp, label = "More activities")
            DropdownMenu(expanded = more, onDismissRequest = { more = false }) {
                val all = CardioType.entries.map { CardioActivity.Builtin(it) } + customs.map { CardioActivity.Custom(it) }
                all.forEach { a ->
                    DropdownMenuItem(
                        text = {
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                                Icon(a.icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp))
                                Text(a.displayName, style = OpenType.body, color = MaterialTheme.colorScheme.onBackground)
                            }
                        },
                        onClick = { onSelect(a); more = false }
                    )
                }
                DropdownMenuItem(
                    text = {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Icon(OpenIcons.Plus, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(18.dp))
                            Text("Add custom activity", style = OpenType.body, color = MaterialTheme.colorScheme.onBackground)
                        }
                    },
                    onClick = { more = false; onAddCustom() }
                )
            }
        }
    }
}

/** One watch session on top of the sheet: washed red, with Use this. */
@Composable
private fun WatchSessionRow(w: WatchWorkout, used: Boolean, useMiles: Boolean, locale: Locale, use24h: Boolean, nowMs: Long, onUse: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    val ink = MaterialTheme.colorScheme.onBackground
    val name = CardioType.fromCode(HcExerciseTypes.toCardioCode(w.exerciseType)).displayName
    val whenText = remember(w.startMs, locale, use24h) {
        "${dayChipLabel(w.startMs, nowMs, locale).lowercase(locale)} ${clockLabel(w.startMs, use24h, locale)}"
    }
    val title = buildList {
        add(name)
        add("${w.durationMin} min")
        w.distanceKm?.takeIf { it > 0 }?.let { add(formatDistance(it, useMiles)) }
    }.joinToString(" · ")
    Row(
        Modifier
            .fillMaxWidth()
            .clip(CardioCorner)
            .background(accent.copy(alpha = 0.15f))
            .border(1.dp, if (used) accent else accent.copy(alpha = 0.35f), CardioCorner)
            .padding(14.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(OpenIcons.Navigation, contentDescription = null, tint = ink, modifier = Modifier.size(20.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            OpenLabel("Your watch · $whenText", style = OpenType.monoXS, color = ink)
            Text(title, style = OpenType.rowTitle, color = ink)
        }
        Box(
            Modifier
                .touchSlop(vertical = 7.dp)
                .bounceClick(role = Role.Button, onClick = onUse)
                .padding(vertical = 7.dp)
                .semantics { contentDescription = if (used) "Using this watch session" else "Use this watch session: $title" }
                .heightIn(min = 34.dp)
                .clip(CardioCorner)
                .background(accent)
                .padding(horizontal = 12.dp),
            contentAlignment = Alignment.Center
        ) {
            Text(if (used) "Using" else "Use this", style = OpenType.buttonS, color = MaterialTheme.colorScheme.onPrimary)
        }
    }
}

/** The typed duration: a serif value with its unit, right-aligned, numeric. */
@Composable
private fun DurationField(value: String, onValueChange: (String) -> Unit, onNext: () -> Unit) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(verticalAlignment = Alignment.Bottom) {
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            textStyle = CardioOpenType.sheetFigure.copy(color = ink, textAlign = TextAlign.End),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Next),
            keyboardActions = KeyboardActions(onNext = { onNext() }),
            modifier = Modifier.width(IntrinsicSize.Min).widthIn(min = 20.dp).semantics { contentDescription = "Duration in minutes" },
            decorationBox = { inner ->
                Box(contentAlignment = Alignment.CenterEnd) {
                    if (value.isEmpty()) Text("0", style = CardioOpenType.sheetFigure, color = muted.copy(alpha = 0.6f))
                    inner()
                }
            }
        )
        Text(
            cardioFigure("", if (value.contains(':')) " h:mm" else " min", CardioOpenType.sheetFigureUnit),
            style = CardioOpenType.sheetFigure,
            color = ink
        )
    }
}

/** A label on the left and a 140dp outlined field with its unit on the right (Distance, Laps...). */
@Composable
private fun FieldRow(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    unit: String,
    keyboardType: KeyboardType,
    focusRequester: FocusRequester? = null,
    onDone: () -> Unit = {},
) {
    val ink = MaterialTheme.colorScheme.onBackground
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        OpenLabel(label, Modifier.weight(1f).padding(end = 12.dp))
        Row(
            Modifier
                .width(140.dp)
                .heightIn(min = 40.dp)
                .border(1.dp, CardioOpenColors.FieldEdge, CardioCorner)
                .padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            BasicTextField(
                value = value,
                onValueChange = onValueChange,
                textStyle = CardioOpenType.fieldValue.copy(color = ink),
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = keyboardType, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { onDone() }),
                modifier = Modifier
                    .weight(1f)
                    .then(if (focusRequester != null) Modifier.focusRequester(focusRequester) else Modifier)
                    .semantics { contentDescription = label }
            )
            if (unit.isNotEmpty()) OpenLabel(unit, style = OpenType.monoXS)
        }
    }
}

/** The collapsed row that opens the optional details. */
@Composable
private fun MoreRow(text: String, open: Boolean, onToggle: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier
            .fillMaxWidth()
            .clickableLabeled(if (open) "Hide the details" else "Show the details", onClick = onToggle)
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(text, style = OpenType.body, color = muted, modifier = Modifier.weight(1f))
        Icon(if (open) OpenIcons.ChevronUp else OpenIcons.ChevronDown, contentDescription = null, tint = muted, modifier = Modifier.size(18.dp))
    }
}

/** A labelled set of outlined chips: pick one (tap again to clear) or, with [isSelected], several. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ChipGroup(
    label: String,
    options: List<String>,
    selected: Int,
    isSelected: (Int) -> Boolean = { it == selected },
    role: Role = Role.RadioButton,
    onPick: (Int) -> Unit,
) {
    Spacer(Modifier.height(12.dp))
    OpenLabel(label)
    Spacer(Modifier.height(2.dp))
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        options.forEachIndexed { i, o ->
            CardioOutlineChip(o, onClick = { onPick(i) }, selected = isSelected(i), role = role)
        }
    }
}

/** The note: the one multi-line field, outlined as the other fields are. */
@Composable
private fun NoteField(value: String, onValueChange: (String) -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        textStyle = OpenType.body.copy(color = MaterialTheme.colorScheme.onBackground),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        minLines = 3,
        maxLines = 6,
        modifier = Modifier.fillMaxWidth().semantics { contentDescription = "Note" },
        decorationBox = { inner ->
            Box(
                Modifier
                    .fillMaxWidth()
                    .border(1.dp, CardioOpenColors.FieldEdge, CardioCorner)
                    .padding(12.dp)
            ) {
                if (value.isEmpty()) Text("A few words, if you like", style = OpenType.body, color = muted.copy(alpha = 0.6f))
                inner()
            }
        }
    )
}

/** The red Save, its label centred and named for the activity. Dim and passive until it can save. */
@Composable
private fun SaveBar(text: String, enabled: Boolean, onClick: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .bounceClick(enabled = enabled, role = Role.Button, onClick = onClick)
            .clip(CardioCorner)
            .background(if (enabled) accent else accent.copy(alpha = 0.35f)),
        contentAlignment = Alignment.Center
    ) {
        Text(text, style = OpenType.button, color = openOnAccentLarge(), modifier = Modifier.padding(vertical = 14.dp))
    }
}
