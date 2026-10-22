@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.programbuilder

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.domain.schedule.WeeklySchedule
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.dragContainer
import com.forge.app.ui.common.parseAccentHex
import com.forge.app.ui.common.rememberDragDropState
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.HistoryColors
import com.forge.app.ui.theme.HistoryType
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

/**
 * Edit day (design `program_pd`): the day's name (tap to rename), its type, weekday and colour as
 * tight choices, then its lifts (tap for sets and reps, hold to reorder, X to remove), Add a lift,
 * and Duplicate / Remove day at the end. Pure: every change goes out through a callback.
 */
@Composable
fun DayEditorPage(
    day: BuilderDay,
    weekdayPlan: Boolean,
    onBack: () -> Unit,
    onRename: () -> Unit,
    onSetType: (String) -> Unit,
    onSetAccent: (String) -> Unit,
    onToggleWeekday: (Int) -> Unit,
    onClearWeekdays: () -> Unit,
    onOpenExercise: (String) -> Unit,
    onRemoveExercise: (String) -> Unit,
    onMoveExercise: (Int, Int) -> Unit,
    onAddExercise: () -> Unit,
    onDuplicateDay: () -> Unit,
    onRemoveDay: () -> Unit
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val listState = rememberLazyListState()
    val drag = rememberDragDropState(listState, firstDraggableIndex = 2, draggableItemCount = day.exercises.size) { from, to ->
        onMoveExercise(from, to)
    }

    LazyColumn(
        state = listState,
        modifier = Modifier.fillMaxSize().statusBarsPadding().dragContainer(drag),
        contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 56.dp)
    ) {
        item("top") {
            Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
                OpenTopBarIcon(OpenIcons.ArrowLeft, "Back to your program", onBack)
            }
        }
        item("head") {
            Column {
                Spacer(Modifier.height(20.dp))
                // The name itself renames; the pencil beside it says so.
                Row(
                    Modifier.bounceCombinedClick(onClickLabel = "Rename day", role = Role.Button, onClick = onRename),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(day.name, style = OpenType.titleM, color = ink, modifier = Modifier.weight(1f, fill = false).semantics { heading() })
                    Icon(OpenIcons.Edit, contentDescription = null, tint = muted, modifier = Modifier.size(18.dp))
                }

                Spacer(Modifier.height(32.dp))
                OpenLabel("Type")
                Spacer(Modifier.height(12.dp))
                val current = baseArchetype(day.archetype)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    DAY_TYPES.forEach { (key, label, _) ->
                        TightChoice(label, selected = current == key, onClick = { onSetType(key) })
                    }
                }

                Spacer(Modifier.height(32.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    OpenLabel("Day of the week")
                    OpenLabel(day.weekdayLabel() ?: "Any")
                }
                Spacer(Modifier.height(12.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                    (0 until WeeklySchedule.SLOTS).forEach { wd ->
                        WeekdayChoice(
                            label = WeeklySchedule.WEEKDAY_SHORT[wd].take(1),
                            readout = WeeklySchedule.WEEKDAY_SHORT[wd],
                            selected = wd in day.weekdays,
                            onClick = { onToggleWeekday(wd) },
                            modifier = Modifier.weight(1f)
                        )
                    }
                    WeekdayChoice("Any", "Any day", day.weekdays.isEmpty(), onClearWeekdays, Modifier.width(52.dp))
                }
                Spacer(Modifier.height(8.dp))
                // Once any day has a weekday the plan runs by weekday, and an unpinned day never
                // comes up on its own, so the line says that instead of "next in line".
                OpenLabel(
                    when {
                        day.weekdays.isNotEmpty() -> "Home shows this day on these weekdays. A weekday holds one day"
                        weekdayPlan -> "Any: not on your week. Start it from Your program"
                        else -> "Any: no fixed day, it comes up next in line"
                    },
                    style = OpenType.monoXS
                )

                Spacer(Modifier.height(32.dp))
                OpenLabel("Colour")
                Spacer(Modifier.height(12.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    DAY_ACCENTS.forEachIndexed { i, hex ->
                        val chosen = hex.equals(day.accentHex, ignoreCase = true)
                        Box(
                            Modifier
                                .touchSlop(vertical = 6.dp, horizontal = 6.dp)
                                .selectable(selected = chosen, role = Role.RadioButton) { onSetAccent(hex) }
                                .semantics { contentDescription = "Day colour ${i + 1}" }
                                .padding(6.dp)
                                .size(36.dp)
                                .then(if (chosen) Modifier.border(2.dp, ink, CircleShape) else Modifier),
                            contentAlignment = Alignment.Center
                        ) {
                            Box(Modifier.size(26.dp).clip(CircleShape).background(parseAccentHex(hex)))
                        }
                    }
                }

                Spacer(Modifier.height(36.dp))
                Row(Modifier.fillMaxWidth().semantics { heading() }, horizontalArrangement = Arrangement.SpaceBetween) {
                    OpenLabel("Lifts · hold to reorder", Modifier.weight(1f, fill = false))
                    Spacer(Modifier.width(12.dp))
                    OpenLabel("${day.totalSets} sets")
                }
                Spacer(Modifier.height(6.dp))
                if (day.exercises.isEmpty()) {
                    Spacer(Modifier.height(10.dp))
                    OpenLabel("0 lifts · add one below", style = OpenType.monoXS)
                }
            }
        }
        itemsIndexed(day.exercises, key = { _, e -> e.uid }) { i, e ->
            DraggableItem(drag, i) { dragging ->
                PlannedLiftRow(
                    name = e.name,
                    sub = e.muscle,
                    figure = setsTimesReps(e.sets, e.reps),
                    load = null,
                    tag = null,
                    editing = true,
                    dragging = dragging,
                    onOpen = { onOpenExercise(e.uid) },
                    onRemove = { onRemoveExercise(e.uid) },
                    moveActions = buildList {
                        if (i > 0) add(CustomAccessibilityAction("Move up") { onMoveExercise(i, i - 1); true })
                        if (i < day.exercises.size - 1) add(CustomAccessibilityAction("Move down") { onMoveExercise(i, i + 1); true })
                    },
                    figureStyle = HistoryType.liftFigure,
                    modifier = Modifier.padding(vertical = 10.dp)
                )
            }
        }
        item("end") {
            Column {
                Spacer(Modifier.height(10.dp))
                OutlinedAction(OpenIcons.Plus, "Add a lift", onAddExercise, Modifier.fillMaxWidth())
                Spacer(Modifier.height(28.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    OutlinedAction(OpenIcons.Copy, "Duplicate day", onDuplicateDay, Modifier.weight(1f))
                    OutlinedAction(
                        OpenIcons.Trash, "Remove day", onRemoveDay, Modifier.weight(1f),
                        color = HistoryColors.Remove, edge = HistoryColors.RemoveEdge
                    )
                }
            }
        }
    }
}

private val ChoiceShape = RoundedCornerShape(4.dp)

/** A tight pick-one choice: 38dp, 4dp corners, the chosen one washed red inside a red line. */
@Composable
private fun TightChoice(label: String, selected: Boolean, onClick: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        Modifier
            .touchSlop(vertical = 5.dp)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .padding(vertical = 5.dp)
            .heightIn(min = 38.dp)
            .then(
                if (selected) Modifier.background(accent.copy(alpha = 0.15f), ChoiceShape).border(1.dp, accent, ChoiceShape)
                else Modifier.border(1.dp, OpenColors.Rule, ChoiceShape)
            )
            .padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = HistoryType.choice, color = MaterialTheme.colorScheme.onBackground)
    }
}

/** One weekday (or Any) in the week row: 40dp, mono, the chosen one bold in a red line. */
@Composable
private fun WeekdayChoice(label: String, readout: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        modifier
            .touchSlop(vertical = 4.dp)
            .selectable(selected = selected, role = Role.Checkbox, onClick = onClick)
            .semantics { contentDescription = readout }
            .padding(vertical = 4.dp)
            .heightIn(min = 40.dp)
            .then(
                if (selected) Modifier.background(accent.copy(alpha = 0.15f), ChoiceShape).border(1.dp, accent, ChoiceShape)
                else Modifier.border(1.dp, OpenColors.Rule, ChoiceShape)
            ),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = if (selected) HistoryType.weekdayOn else HistoryType.weekday, color = MaterialTheme.colorScheme.onBackground)
    }
}
