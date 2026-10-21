package com.forge.app.ui.programbuilder

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.common.DraggableItem
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.dragContainer
import com.forge.app.ui.common.rememberDragDropState
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.HistoryColors
import com.forge.app.ui.theme.HistoryType
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.openOnAccentLarge
import java.time.LocalDate

/** What the Program page shows: the plan, which day is open, and whether it is being edited. */
class ProgramPageModel(
    val days: List<BuilderDay>,
    val selected: Int,
    val editing: Boolean,
    /** Read-only and loaded: the pencil can open the editor. */
    val canEdit: Boolean,
    val loaded: Boolean,
    val meta: String,
    val deloadNote: Boolean = false,
    /** "dayKey|libId" to the coach's change on that lift ("↑ 1 rep"). */
    val coachChanges: Map<String, String> = emptyMap(),
    /** Library id (of the lift actually trained) to today's load. */
    val loads: Map<String, PlannedLoad?> = emptyMap(),
    /** Base library id to (name, id) of the lift swapped in "every week". */
    val swaps: Map<String, Pair<String, String?>> = emptyMap(),
    /** Start is drawn: viewing a saved day that has lifts. */
    val canStart: Boolean = false,
    val saving: Boolean = false,
    val unit: WeightUnit = WeightUnit.LB,
    val today: LocalDate = LocalDate.now()
)

/** Everything the Program page can do. */
class ProgramPageActions(
    val back: () -> Unit = {},
    val edit: () -> Unit = {},
    val select: (Int) -> Unit = {},
    val moveDay: (Int, Int) -> Unit = { _, _ -> },
    val moveLift: (Int, Int) -> Unit = { _, _ -> },
    val removeLift: (String) -> Unit = {},
    val openLift: (String) -> Unit = {},
    val editDay: () -> Unit = {},
    val addLift: () -> Unit = {},
    val addDay: () -> Unit = {},
    val save: () -> Unit = {},
    val start: () -> Unit = {}
)

/**
 * Your program (designs `program_pr` and `program_pe`): the title and the plan's split, the week as
 * one bar per day carrying its sets, the day you tap as its lifts with sets × reps and today's load,
 * and Start. The pencil turns the same page into the editor: bars and lifts get handles (hold to
 * drag), lifts get a remove, and Edit day / Add a lift / Add day / Save close it.
 */
@Composable
fun ProgramPage(model: ProgramPageModel, actions: ProgramPageActions) {
    val ink = MaterialTheme.colorScheme.onBackground
    val days = model.days
    val open = days.getOrNull(model.selected)
    val lifts = open?.exercises.orEmpty()
    val listState = rememberLazyListState()
    val drag = rememberDragDropState(listState, firstDraggableIndex = LEADING_ITEMS, draggableItemCount = lifts.size) { from, to ->
        actions.moveLift(from, to)
    }

    LazyColumn(
        state = listState,
        modifier = Modifier
            .fillMaxSize()
            .statusBarsPadding()
            .then(if (model.editing) Modifier.dragContainer(drag) else Modifier),
        contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 56.dp)
    ) {
        item("top") {
            Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
                OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", actions.back)
                Spacer(Modifier.weight(1f))
                if (model.canEdit) OpenTopBarIcon(OpenIcons.Edit, "Edit program", actions.edit)
            }
        }
        item("head") {
            Column {
                Spacer(Modifier.height(20.dp))
                Text("Your program", style = OpenType.titleM, color = ink, modifier = Modifier.semantics { heading() })
                Spacer(Modifier.height(8.dp))
                if (model.editing) OpenLabel("Editing · nothing changes until you save", style = OpenType.monoXS, color = ink)
                else if (model.meta.isNotEmpty()) OpenLabel(model.meta, style = OpenType.monoXS)
                if (model.editing && model.deloadNote && days.isNotEmpty()) {
                    Spacer(Modifier.height(8.dp))
                    OpenLabel("Deload week: these sets are reduced, and saving keeps them as your plan", style = OpenType.monoXS)
                }
                Spacer(Modifier.height(36.dp))
                if (days.isEmpty()) {
                    if (model.loaded) OpenLabel(
                        if (model.editing) "0 days · add a day to start your plan" else "0 days · no plan yet",
                        style = OpenType.monoXS
                    )
                } else {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        OpenLabel(if (days.size == 1) "1 day" else "${days.size} days")
                        Spacer(Modifier.width(12.dp))
                        OpenLabel("${days.sumOf { it.totalSets }} sets a week")
                    }
                    Spacer(Modifier.height(14.dp))
                    ProgramWeekBars(
                        days = days,
                        selected = model.selected,
                        today = model.today,
                        editing = model.editing,
                        onSelect = actions.select,
                        onMove = actions.moveDay
                    )
                }
            }
        }
        item("lifts-head") {
            if (open != null) Column {
                Spacer(Modifier.height(40.dp))
                Row(Modifier.fillMaxWidth().semantics { heading() }, horizontalArrangement = Arrangement.SpaceBetween) {
                    val wd = open.weekdays.minOrNull()?.let { com.forge.app.domain.schedule.WeeklySchedule.WEEKDAY_SHORT[it] }
                    OpenLabel(listOfNotNull(open.name, wd).joinToString(" · "), Modifier.weight(1f, fill = false))
                    Spacer(Modifier.width(12.dp))
                    OpenLabel("${lifts.size} ${if (lifts.size == 1) "lift" else "lifts"} · ${open.totalSets} sets")
                }
                Spacer(Modifier.height(6.dp))
                if (lifts.isEmpty()) {
                    Spacer(Modifier.height(10.dp))
                    OpenLabel("0 lifts · add one and it shows here", style = OpenType.monoXS)
                }
            }
        }
        if (open != null) itemsIndexed(lifts, key = { _, e -> e.uid }) { i, ex ->
            DraggableItem(drag, i) { dragging ->
                val swap = if (model.editing) null else model.swaps[ex.libId]
                PlannedLiftRow(
                    name = swap?.first ?: ex.name,
                    sub = if (swap != null) "In place of ${ex.name}" else ex.muscle,
                    figure = setsTimesReps(ex.sets, ex.reps),
                    load = loadLabel(model.loads[swap?.second ?: ex.libId], model.unit),
                    tag = model.coachChanges["${open.key}|${ex.libId}"],
                    editing = model.editing,
                    dragging = dragging,
                    onOpen = if (model.editing) ({ actions.openLift(ex.uid) }) else null,
                    onRemove = { actions.removeLift(ex.uid) },
                    moveActions = if (!model.editing) emptyList() else buildList {
                        if (i > 0) add(CustomAccessibilityAction("Move up") { actions.moveLift(i, i - 1); true })
                        if (i < lifts.size - 1) add(CustomAccessibilityAction("Move down") { actions.moveLift(i, i + 1); true })
                    },
                    modifier = Modifier.padding(vertical = 10.dp)
                )
            }
        }
        item("end") {
            Column {
                if (model.editing) {
                    if (open != null) {
                        Spacer(Modifier.height(10.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            OutlinedAction(OpenIcons.Edit, "Edit day", actions.editDay)
                            OutlinedAction(OpenIcons.Plus, "Add a lift", actions.addLift)
                        }
                    }
                    Spacer(Modifier.height(32.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        OutlinedAction(OpenIcons.Plus, "Add day", actions.addDay, Modifier.weight(1f), tall = true)
                        FilledAction("Save", actions.save, Modifier.weight(1f), enabled = model.loaded && !model.saving)
                    }
                } else if (open != null && model.canStart) {
                    Spacer(Modifier.height(26.dp))
                    FilledAction("Start ${open.name}", actions.start, Modifier.fillMaxWidth())
                    if (lifts.any { model.coachChanges.containsKey("${open.key}|${it.libId}") }) {
                        Spacer(Modifier.height(8.dp))
                        OpenLabel("Red marks this week's change from the coach", style = OpenType.monoXS)
                    }
                }
            }
        }
    }
}

/** Leading LazyColumn items above the lift rows: top bar, head, lifts head. */
private const val LEADING_ITEMS = 3

/**
 * The week as bars: each day's sets against the heaviest day, in a 40dp track, its name and its
 * place in the week under it. The open day is red. Editing adds a handle under each, and holding a
 * bar drags it along the week.
 */
@Composable
private fun ProgramWeekBars(
    days: List<BuilderDay>,
    selected: Int,
    today: LocalDate,
    editing: Boolean,
    onSelect: (Int) -> Unit,
    onMove: (Int, Int) -> Unit
) {
    val peak = days.maxOf { it.totalSets }.coerceAtLeast(1)
    val reorderable = editing && days.size > 1
    val gapPx = with(LocalDensity.current) { BAR_GAP.toPx() }
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    var rowWidth by remember { mutableIntStateOf(0) }
    var dragKey by remember { mutableStateOf<String?>(null) }
    var dragOffset by remember { mutableFloatStateOf(0f) }
    val keys by rememberUpdatedState(days.map { it.uid })
    val move by rememberUpdatedState(onMove)
    val select by rememberUpdatedState(onSelect)

    Row(
        Modifier.fillMaxWidth().onSizeChanged { rowWidth = it.width },
        horizontalArrangement = Arrangement.spacedBy(BAR_GAP),
        verticalAlignment = Alignment.Top
    ) {
        days.forEachIndexed { i, d ->
            key(d.uid) {
                val dragging = dragKey == d.uid
                val on = i == selected || dragging
                val fraction = if (d.totalSets <= 0) 0f else (d.totalSets.toFloat() / peak).coerceAtLeast(0.12f)
                val animated by animateFloatAsState(fraction, ForgeMotion.standardTween(), label = "day_sets")
                Column(
                    Modifier
                        .weight(1f)
                        .then(
                            if (dragging) Modifier.zIndex(1f).graphicsLayer { translationX = dragOffset; scaleX = 1.06f; scaleY = 1.06f }
                            else Modifier
                        )
                        .bounceClick(role = Role.Tab) { onSelect(i) }
                        .semantics {
                            contentDescription = "${d.name}, ${d.totalSets} sets"
                            this.selected = i == selected
                            if (reorderable) customActions = buildList {
                                if (i > 0) add(CustomAccessibilityAction("Move earlier") { onMove(i, i - 1); true })
                                if (i < days.size - 1) add(CustomAccessibilityAction("Move later") { onMove(i, i + 1); true })
                            }
                        }
                        .then(
                            if (!reorderable) Modifier
                            else Modifier.pointerInput(d.uid) {
                                var at = -1
                                fun slot(): Float = (rowWidth + gapPx) / keys.size.coerceAtLeast(1)
                                detectDragGesturesAfterLongPress(
                                    onDragStart = {
                                        at = keys.indexOf(d.uid)
                                        dragKey = d.uid
                                        dragOffset = 0f
                                        if (at >= 0) select(at)
                                    },
                                    onDrag = { change, amount ->
                                        change.consume()
                                        dragOffset += amount.x
                                        val s = slot()
                                        val logical = if (rtl) -dragOffset else dragOffset
                                        val step = when {
                                            logical > s / 2 && at < keys.size - 1 -> 1
                                            logical < -s / 2 && at > 0 -> -1
                                            else -> 0
                                        }
                                        if (step != 0 && at >= 0) {
                                            move(at, at + step)
                                            at += step
                                            dragOffset -= s * step * (if (rtl) -1 else 1)
                                        }
                                    },
                                    onDragEnd = { dragKey = null; dragOffset = 0f },
                                    onDragCancel = { dragKey = null; dragOffset = 0f }
                                )
                            }
                        ),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Box(
                        Modifier.widthIn(max = 40.dp).fillMaxWidth().height(110.dp)
                            .clip(RoundedCornerShape(4.dp)).background(HistoryColors.DayTrack),
                        contentAlignment = Alignment.BottomCenter
                    ) {
                        Box(
                            Modifier.fillMaxWidth().fillMaxHeight(animated.coerceIn(0f, 1f))
                                .clip(RoundedCornerShape(4.dp))
                                .background(if (on) MaterialTheme.colorScheme.primary else OpenColors.Bar)
                        )
                    }
                    OpenLabel(
                        d.name,
                        style = OpenType.monoXS.copy(textAlign = TextAlign.Center),
                        color = if (on) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    OpenLabel(weekdayDateLabel(d.weekdays, today), style = OpenType.monoXS.copy(textAlign = TextAlign.Center))
                    if (editing) Icon(OpenIcons.Menu, contentDescription = null, tint = OpenColors.Glyph, modifier = Modifier.size(16.dp))
                }
            }
        }
    }
}

private val BAR_GAP = 10.dp

/**
 * One planned lift: (handle) · its name with the coach's change tag and the muscle under it · sets ×
 * reps over today's load · (remove). Editing, the row opens its sets and reps.
 */
@Composable
internal fun PlannedLiftRow(
    name: String,
    sub: String,
    figure: String,
    load: String?,
    tag: String?,
    editing: Boolean,
    dragging: Boolean,
    onOpen: (() -> Unit)?,
    onRemove: () -> Unit,
    moveActions: List<CustomAccessibilityAction>,
    modifier: Modifier = Modifier,
    figureStyle: androidx.compose.ui.text.TextStyle = HistoryType.planFigure
) {
    val ink = MaterialTheme.colorScheme.onBackground
    Row(
        modifier
            .fillMaxWidth()
            .then(if (dragging) Modifier.background(MaterialTheme.colorScheme.surface, RoundedCornerShape(4.dp)) else Modifier)
            .then(
                if (onOpen != null) Modifier.bounceCombinedClick(onClickLabel = "Sets and reps for $name", role = Role.Button, onClick = onOpen)
                else Modifier
            )
            .semantics { if (moveActions.isNotEmpty()) customActions = moveActions },
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (editing) Icon(OpenIcons.Menu, contentDescription = null, tint = OpenColors.Rule, modifier = Modifier.size(18.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(name, style = HistoryType.planLift, color = ink, modifier = Modifier.weight(1f, fill = false))
                if (tag != null) CoachTag(tag)
            }
            if (sub.isNotBlank()) OpenLabel(sub, style = OpenType.monoXS)
        }
        Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(figure, style = figureStyle, color = ink)
            if (load != null) OpenLabel(load, style = OpenType.monoXS, color = ink)
        }
        if (editing) {
            // 32dp drawn so the row keeps the design's spacing; 48dp to the finger vertically.
            Box(
                Modifier
                    .touchSlop(vertical = 8.dp, horizontal = 8.dp)
                    .bounceCombinedClick(onClickLabel = "Remove $name", role = Role.Button, onClick = onRemove)
                    .padding(8.dp)
                    .size(16.dp)
            ) {
                Icon(OpenIcons.X, contentDescription = "Remove $name", tint = OpenColors.Glyph, modifier = Modifier.size(16.dp))
            }
        }
    }
}

/** The coach's change on a lift: red, 2dp corners, dark mono. */
@Composable
private fun CoachTag(text: String) {
    Box(
        Modifier.clip(RoundedCornerShape(2.dp)).background(MaterialTheme.colorScheme.primary)
            .padding(horizontal = 5.dp, vertical = 3.dp)
            .semantics { contentDescription = "Coach changed this week: $text" },
        contentAlignment = Alignment.Center
    ) {
        Text(text.uppercase(), style = HistoryType.changeTag, color = OpenColors.OnAccentDark)
    }
}

private val ActionShape = RoundedCornerShape(4.dp)

/** An outlined action with its glyph: Edit day, Add a lift (44dp), Add day (52dp, [tall]). */
@Composable
internal fun OutlinedAction(
    icon: ImageVector?,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tall: Boolean = false,
    color: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.onBackground,
    edge: androidx.compose.ui.graphics.Color = OpenColors.Rule
) {
    Row(
        modifier
            .touchSlop(vertical = if (tall) 0.dp else 2.dp)
            .bounceCombinedClick(onClickLabel = label, role = Role.Button, onClick = onClick)
            .padding(vertical = if (tall) 0.dp else 2.dp)
            .heightIn(min = if (tall) 52.dp else 44.dp)
            .border(1.dp, edge, ActionShape)
            .padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (icon != null) Icon(icon, contentDescription = null, tint = color, modifier = Modifier.size(16.dp))
        Text(label, style = HistoryType.choice, color = color)
    }
}

/** The page's red action: 52dp, 4dp corners, its label centred in white. */
@Composable
internal fun FilledAction(label: String, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        modifier
            .bounceCombinedClick(onClickLabel = label, role = Role.Button, enabled = enabled, onClick = onClick)
            .heightIn(min = 52.dp)
            .clip(ActionShape)
            .background(if (enabled) accent else accent.copy(alpha = 0.35f))
            .padding(horizontal = 16.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = OpenType.button, color = openOnAccentLarge(), textAlign = TextAlign.Center)
    }
}
