package com.tally.app.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDefaults
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import com.tally.app.ui.theme.Tally
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset

/**
 * A question that must be answered before an act that cannot be undone: Relay's popover, a title,
 * the reading in a sentence, and the two keys. Almost nothing in Tally asks: deletes come with Undo.
 */
@Composable
fun ConfirmDialog(
    title: String,
    body: String,
    confirm: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    danger: Boolean = false,
    dismiss: String = "Cancel",
    /** A third way out, between the two: "Archive instead". */
    alternate: Pair<String, () -> Unit>? = null,
) {
    val c = Tally.colors
    Dialog(onDismissRequest = onDismiss) {
        Column(
            Modifier.widthIn(max = 420.dp).clip(Radii.popover).background(c.slab).border(1.dp, c.strong, Radii.popover).padding(18.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            T(title, style = Tally.type.title, color = c.ink)
            T(body, style = Tally.type.ui, color = c.ink2)
            Gap(4.dp)
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                Key(dismiss, onDismiss, kind = KeyKind.Quiet)
                alternate?.let { (label, act) -> Key(label, act) }
                Key(confirm, onConfirm, kind = if (danger) KeyKind.Danger else KeyKind.Primary)
            }
        }
    }
}

/**
 * A sheet from the bottom in Relay's slab: a picker, a short form. Scrolls when it holds more than
 * the screen; the handle is Relay's track pill.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Sheet(onDismiss: () -> Unit, title: String? = null, content: @Composable ColumnScope.() -> Unit) {
    val c = Tally.colors
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = state,
        containerColor = c.slab,
        contentColor = c.ink,
        scrimColor = Color.Black.copy(alpha = .5f),
        shape = Radii.sheet,
        dragHandle = {
            Box(Modifier.padding(top = 10.dp, bottom = 6.dp).size(width = 36.dp, height = 4.dp).clip(Radii.pill).background(c.track))
        },
    ) {
        Column(
            Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 20.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            title?.let { T(it, Modifier.padding(start = 4.dp, bottom = 4.dp), Tally.type.title, c.ink) }
            content()
        }
    }
}

/** A row in a sheet that picks one thing: a glyph or badge, its name, a check when it is the one. */
@Composable
fun PickRow(text: String, selected: Boolean, onClick: () -> Unit, sub: String? = null, leading: (@Composable () -> Unit)? = null) {
    val c = Tally.colors
    ListRow(selected = selected, onClick = onClick) {
        leading?.invoke()
        Column(Modifier.weight(1f)) {
            T(text, style = Tally.type.ui, color = c.ink, maxLines = 1)
            sub?.let { T(it, style = Tally.type.caption, color = c.ink3, maxLines = 1) }
        }
        if (selected) Glyph("check", 16.dp, c.ink)
    }
}

/** Material's date picker in Relay's colours; dates are local days, never instants on screen. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DateSheet(date: LocalDate, onPick: (LocalDate) -> Unit, onDismiss: () -> Unit, latest: LocalDate? = null) {
    val c = Tally.colors
    val state = rememberDatePickerState(
        initialSelectedDateMillis = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(),
        selectableDates = object : androidx.compose.material3.SelectableDates {
            override fun isSelectableDate(utcTimeMillis: Long): Boolean =
                latest == null || !Instant.ofEpochMilli(utcTimeMillis).atZone(ZoneOffset.UTC).toLocalDate().isAfter(latest)
        },
    )
    val colors = DatePickerDefaults.colors(
        containerColor = c.slab,
        titleContentColor = c.ink3,
        headlineContentColor = c.ink,
        weekdayContentColor = c.ink3,
        dayContentColor = c.ink,
        selectedDayContainerColor = c.ink,
        selectedDayContentColor = c.wall,
        todayContentColor = c.ink,
        todayDateBorderColor = c.ink2,
    )
    DatePickerDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            Key("Done", {
                state.selectedDateMillis?.let { onPick(Instant.ofEpochMilli(it).atZone(ZoneOffset.UTC).toLocalDate()) }
                onDismiss()
            }, kind = KeyKind.Primary, modifier = Modifier.padding(end = 8.dp, bottom = 8.dp))
        },
        dismissButton = { Key("Cancel", onDismiss, kind = KeyKind.Quiet, modifier = Modifier.padding(bottom = 8.dp)) },
        colors = colors,
    ) {
        DatePicker(state = state, colors = colors, showModeToggle = false)
    }
}

/** Spacing for a sheet's actions. */
@Composable
fun SheetActions(content: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
        content()
    }
}
