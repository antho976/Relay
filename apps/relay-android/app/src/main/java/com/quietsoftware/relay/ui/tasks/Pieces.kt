package com.quietsoftware.relay.ui.tasks

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewModelScope
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.model.Lamp
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.sync.Refs
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

// The board's vocabulary (crates/relay-board, board_view.rs), shared by every task surface.

internal val TYPES = listOf("task" to "Task", "feature" to "Feature", "bug" to "Bug", "chore" to "Chore", "spike" to "Spike")
internal val PRIORITIES = listOf("low" to "Low", "medium" to "Medium", "high" to "High", "urgent" to "Urgent")
internal val SIZES = listOf("S", "M", "L")

/** Sessions a task can be dispatched to: the handler refuses exited and closed ones. */
internal val DISPATCHABLE = setOf("created", "parked", "restorable", "running", "idle", "blocked")

/** Sub-tasks nest this deep at most (TASK_DEPTH_MAX): a root and two levels of children. */
internal const val DEPTH_MAX = 3

/** The rollup bar and a completed module's pill (board.css `#8957e5`). */
internal val ROLLUP = Color(0xFF8957E5)

internal fun titled(s: String) = s.replaceFirstChar { it.uppercase() }

internal fun points(size: String?) = when (size) {
    "S" -> 1
    "M" -> 3
    "L" -> 5
    else -> 0
}

/** The task's id in a payload: its number, or a reference to the create the outbox still holds. */
internal fun Task.idJson(): JsonElement = num?.let { JsonPrimitive(it) } ?: Refs.ref(id.removePrefix(Optimistic.TEMP))

/** How labels and toasts name a task: `#42`, or its title while the PC has not numbered it. */
internal fun Task.named(): String = num?.let { "#$it" } ?: "“${title.take(40)}”"

/** The label's hue, as the PC picks it (board_view.rs `hue`: FNV-1a over the name, mod 8). */
internal fun labelHue(name: String): Color {
    var h = -2128831035 // 2166136261 as a u32
    for (b in name.toByteArray()) h = (h xor (b.toInt() and 0xFF)) * 16777619
    return Palette.LABELS[(h.toUInt() % 8u).toInt()]
}

/** Ids of tasks with edits the PC has not taken yet, read off the open outbox. */
internal fun pendingTasks(outbox: List<OutboxEntry>): Set<String> =
    outbox.mapNotNullTo(HashSet()) { e -> Optimistic.target(e.op, e.payload, e.id)?.takeIf { it.first == Kind.Task }?.second }

/** Ids of tasks with an edit the PC refused, parked in the outbox until the person decides. */
internal fun refusedTasks(outbox: List<OutboxEntry>): Set<String> = pendingTasks(outbox.filter { it.parked })

/** The first edit of [taskId] the PC refused, if one is parked in the outbox. */
internal fun refusedEdit(outbox: List<OutboxEntry>, taskId: String): OutboxEntry? =
    outbox.firstOrNull { e -> e.parked && Optimistic.target(e.op, e.payload, e.id) == (Kind.Task to taskId) }

/** Why a parked edit is not on the PC, in the outbox page's words. */
internal fun parkedWhy(e: OutboxEntry): String = when (e.state) {
    OutboxEntry.State.Held -> "Held by a guardrail on the PC"
    OutboxEntry.State.Conflict -> "Changed on the PC too"
    else -> "The PC refused: ${e.error?.message?.takeIf { it.isNotBlank() } ?: "no reason given"}"
}

/** The index a task holds in its column, which is what `task.move`'s `position` means. */
internal fun indexIn(all: List<Task>, task: Task): Int? =
    all.filter { it.column == task.column }.indexOfFirst { it.id == task.id }.takeIf { it >= 0 }

/** Columns a task may be moved to: Done only by approval, unless a commit is already linked. */
internal fun moveTargets(task: Task): List<String> =
    Task.COLUMNS.filter { it != task.column && (it != "done" || task.commits.isNotEmpty()) }

internal val tiny @Composable get() = Relay.type.caption.copy(fontSize = 11.sp, lineHeight = 14.sp)

// ---- Board edits with Undo ----

/**
 * The board's edits, through the outbox, each with one toast and an Undo. An Undo of an edit
 * still waiting on the phone drops it from the outbox, which puts the row back exactly; one the
 * PC has taken is answered by the inverse edit.
 */
internal class TaskOps(private val nav: Nav) {
    fun move(task: Task, to: String, index: Int?) {
        val from = task.column
        val name = task.named()
        send("task.move", buildJsonObject { put("task_id", task.idJson()); put("column", to) }, "Move $name to ${Task.columnLabel(to)}", "Moved $name to ${Task.columnLabel(to)}") {
            nav.relay.change("task.move", buildJsonObject { put("task_id", task.idJson()); put("column", from); index?.let { put("position", it) } }, "Move $name back to ${Task.columnLabel(from)}")
        }
    }

    /**
     * Approval is the person's alone. Its Undo is the approval's own inverse while the PC is in
     * reach (column, slot and state back, the commit link it added dropped, as the desktop's
     * Ctrl+Z); a move back to review is what the phone can queue without it.
     */
    fun approve(task: Task) {
        val name = task.named()
        send("task.approve", buildJsonObject { put("task_id", task.idJson()) }, "Approve $name", "Approved $name") {
            val undo = task.num?.let { n ->
                try {
                    approvalUndo(nav, n)
                } catch (e: BusException) {
                    null
                }
            }
            if (undo != null) nav.relay.call("task.unapprove", undo)
            else nav.relay.change("task.move", buildJsonObject { put("task_id", task.idJson()); put("column", "in_review") }, "Send $name back to review")
        }
    }

    /**
     * Reopen a done task (`task.unapprove`): back where its approval found it, or to In review
     * when no approval of it is on record (moved to Done with a commit already linked). Needs the PC.
     */
    fun reopen(task: Task) {
        val num = task.num ?: return
        val name = task.named()
        nav.shell.viewModelScope.launch {
            val undo = try {
                approvalUndo(nav, num)
            } catch (e: BusException) {
                nav.shell.toast(if (e.error.code == "link.down") "Needs the PC, which is out of reach" else e.error.message.ifBlank { e.error.code })
                return@launch
            }
            // The end of In review: the PC clamps a slot past the last to it.
            val payload = undo ?: buildJsonObject { put("task_id", num); put("column", "in_review"); put("position", Int.MAX_VALUE); put("state", "none") }
            nav.shell.act("task.unapprove", payload, done = "Reopened $name into ${Task.columnLabel(payload.s("column") ?: "in_review")}")
        }
    }

    fun delete(task: Task) {
        val name = task.named()
        send("task.delete", buildJsonObject { put("task_id", task.idJson()) }, "Delete $name", "Deleted $name") {
            nav.relay.change("task.restore", buildJsonObject { put("task_id", task.idJson()) }, "Restore $name")
        }
    }

    private fun send(op: String, payload: JsonObject, label: String, done: String, inverse: suspend () -> Unit) = undoable(nav, op, payload, label, done, inverse)
}

/**
 * What undoes task [num]'s approval: the inverse the PC recorded with its newest one (the column,
 * slot and state it left, and the commit link it made), read off the task's history. Null when
 * that approval is not on record or was undone already. Throws with the PC out of reach.
 */
internal suspend fun approvalUndo(nav: Nav, num: Long): JsonObject? {
    val got = nav.relay.call("task.activity", buildJsonObject { put("task_id", num); put("limit", 100) }) as? JsonObject ?: return null
    // Newest first: the first approval is the one that put the task in Done.
    val row = got.arr("history").firstNotNullOfOrNull { h -> (h as? JsonObject)?.takeIf { it.s("op") == "task.approve" && it.s("kind") == "ok" } } ?: return null
    if (row.l("undone_by") != null) return null
    return row.o("undo_op")?.takeIf { it.s("op") == "task.unapprove" }?.o("payload")
}

/**
 * An edit through the outbox, then one toast with Undo. Undo drops the edit from the outbox while
 * it has not left the phone (the row goes back exactly as it was); once sent, [inverse] runs.
 */
internal fun undoable(nav: Nav, op: String, payload: JsonObject, label: String, done: String, inverse: suspend () -> Unit) = nav.shell.viewModelScope.launch {
    try {
        val queued = (nav.relay.change(op, payload, label) as? Hub.Change.Queued)?.entry
        val away = !nav.shell.state.value.online
        nav.shell.toast(if (away) "$done · saved on this phone" else done, undo = {
            val still = queued?.let { nav.relay.hub.outbox.get(it.id) }
            if (still != null && still.state == OutboxEntry.State.Pending && still.attempts == 0 && !nav.shell.state.value.online) nav.relay.discard(still.id)
            else inverse()
        })
    } catch (e: BusException) {
        nav.shell.toast(e.error.message.ifBlank { e.error.code })
    }
}

// ---- Marks ----

/**
 * The status glyphs (board_view.rs `status_icon`) in the column's colour: a dashed ring for
 * backlog, an open ring for ready, half full for active, three quarters for review, and a
 * checked disc for done.
 */
@Composable
internal fun StatusRing(column: String, size: Dp = 12.dp, modifier: Modifier = Modifier) {
    val color = Palette.column(column, Relay.colors)
    val cut = Relay.colors.wall
    Canvas(modifier.size(size)) {
        val s = this.size.minDimension
        val u = s / 12f
        val r = s / 2f - u
        val stroke = (1.4f * u).coerceAtLeast(1f)
        when (column) {
            "backlog" -> drawCircle(color, r - .3f * u, style = Stroke(stroke, pathEffect = PathEffect.dashPathEffect(floatArrayOf(1.6f * u, 1.9f * u))))
            "done" -> {
                drawCircle(color, r)
                val p = Path().apply {
                    moveTo(center.x - r * .45f, center.y + r * .02f)
                    lineTo(center.x - r * .1f, center.y + r * .36f)
                    lineTo(center.x + r * .48f, center.y - r * .32f)
                }
                drawPath(p, cut, style = Stroke(1.6f * u, cap = StrokeCap.Round))
            }
            else -> {
                drawCircle(color, r - .3f * u, style = Stroke(stroke))
                val pie = when (column) {
                    "active" -> 180f
                    "in_review" -> 270f
                    else -> 0f
                }
                if (pie > 0f) {
                    val pr = r - 2.6f * u
                    drawArc(color, -90f, pie, true, Offset(center.x - pr, center.y - pr), Size(pr * 2, pr * 2))
                }
            }
        }
    }
}

/** Signal bars for low, medium and high; a red square with a cut-out `!` for urgent. */
@Composable
internal fun PriorityMark(priority: String, size: Dp = 12.dp) {
    val c = Relay.colors
    val color = when (priority) {
        "urgent" -> c.held
        "high" -> c.waiting
        else -> c.ink2
    }
    Canvas(Modifier.size(size)) {
        val u = this.size.minDimension / 13f
        if (priority == "urgent") {
            drawRect(color, Offset(.5f * u, .5f * u), Size(12f * u, 12f * u))
            drawRect(c.wall, Offset(5.6f * u, 2.8f * u), Size(1.8f * u, 5.2f * u))
            drawRect(c.wall, Offset(5.6f * u, 9.2f * u), Size(1.8f * u, 1.8f * u))
            return@Canvas
        }
        val lit = when (priority) {
            "high" -> 3
            "medium" -> 2
            else -> 1
        }
        for (i in 0 until 3) {
            val h = listOf(4.5f, 8f, 11.5f)[i] * u
            drawRect(color.copy(alpha = if (i < lit) 1f else .25f), Offset((1f + i * 4f) * u, 12.5f * u - h), Size(2.6f * u, h))
        }
    }
}

/** The type's drawn mark (pages.rs `task_mark`): a filled square for a feature, lines for a chore. */
@Composable
internal fun TypeMark(type: String, size: Dp = 9.dp) {
    val color = Relay.colors.ink2
    Canvas(Modifier.size(size)) {
        val w = this.size.width
        val h = this.size.height
        val line = Stroke(1.dp.toPx())
        when (type) {
            "feature" -> drawRect(color)
            "chore" -> for (y in listOf(1f, 4f, 7f)) drawRect(color, Offset(0f, y / 9f * h), Size(w, 1.dp.toPx()))
            "spike" -> rotate(45f) { drawRect(color, Offset(1.5f / 9f * w, 1.5f / 9f * h), Size(w - 3f / 9f * w, h - 3f / 9f * h), style = line) }
            else -> {
                drawRect(color, style = line)
                if (type == "bug") drawRect(color, Offset(3f / 9f * w, 3f / 9f * h), Size(w - 6f / 9f * w, h - 6f / 9f * h))
            }
        }
    }
}

/** The execution-state badge (board_view.rs `state_badge`); nothing for a task at rest. */
@Composable
internal fun StateBadge(state: String) {
    val c = Relay.colors
    val caption = when (state) {
        "dispatched" -> "DISPATCHED"
        "running" -> "RUNNING"
        "blocked" -> "BLOCKED"
        "failed" -> "FAILED"
        "awaiting_review" -> "REVIEW"
        else -> return
    }
    val tone = when (state) {
        "running" -> c.live
        "awaiting_review", "dispatched" -> c.waiting
        else -> c.held
    }
    Row(
        Modifier.heightIn(min = 18.dp).clip(Radii.board).border(1.dp, c.edge, Radii.board).padding(horizontal = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Dot(tone, 5.dp)
        T(caption, style = Relay.type.caption.copy(fontSize = 9.5.sp, letterSpacing = .06.em), color = if (state == "dispatched" || state == "awaiting_review") c.ink2 else tone, weight = FontWeight.SemiBold)
    }
}

/** A field as a pill, the way a project card shows its custom fields. */
@Composable
internal fun FieldPill(text: String, color: Color = Relay.colors.ink2, edge: Color = Relay.colors.edge, fill: Color = Color.Transparent, mono: Boolean = false, onClick: (() -> Unit)? = null, mark: (@Composable () -> Unit)? = null) {
    Row(
        Modifier.heightIn(min = 22.dp).clip(Radii.board).background(fill).border(1.dp, edge, Radii.board)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 7.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        mark?.invoke()
        T(text, style = if (mono) Relay.type.mono.copy(fontSize = 11.sp) else tiny, color = color, maxLines = 1)
    }
}

@Composable
internal fun PriorityPill(priority: String, onClick: (() -> Unit)? = null) {
    val c = Relay.colors
    val (color, edge, fill) = when (priority) {
        "urgent" -> Triple(c.held, c.held.copy(alpha = .5f), c.held.copy(alpha = .08f))
        "high" -> Triple(c.waiting, c.waiting.copy(alpha = .45f), Color.Transparent)
        else -> Triple(c.ink2, c.edge, Color.Transparent)
    }
    FieldPill(titled(priority), color, edge, fill, onClick = onClick) { PriorityMark(priority, 11.dp) }
}

/** A label: an edge-outlined chip with a muted swatch, so tags read without shouting. */
@Composable
internal fun LabelChip(name: String, onClick: (() -> Unit)? = null, trailing: String? = null) {
    val c = Relay.colors
    Row(
        Modifier.heightIn(min = 22.dp).clip(Radii.board).border(1.dp, c.edge, Radii.board)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(start = 7.dp, end = 8.dp, top = 2.dp, bottom = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        Dot(labelHue(name), 6.dp)
        T(name, style = tiny, color = c.ink2, maxLines = 1)
        trailing?.let { Glyph(it, 11.dp, c.ink3) }
    }
}

/** A column's name in its ring, as the task page's status pill. */
@Composable
internal fun ColumnPill(column: String, onClick: (() -> Unit)? = null) {
    val tone = Palette.column(column, Relay.colors)
    Row(
        Modifier.heightIn(min = 26.dp).clip(Radii.pill).background(tone.copy(alpha = .12f)).border(1.dp, tone.copy(alpha = .45f), Radii.pill)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 10.dp, vertical = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        StatusRing(column, 12.dp)
        T(Task.columnLabel(column), style = Relay.type.caption, color = Relay.colors.ink, weight = FontWeight.Medium)
    }
}

/** "Saved on this phone": a row the PC has not taken yet; in held red, one with an edit it did not apply. */
@Composable
internal fun WaitingMark(text: String = "saved on phone", tone: Color = Relay.colors.waiting) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
        Dot(tone, 6.dp)
        T(text, style = tiny, color = Relay.colors.ink3, maxLines = 1)
    }
}

// ---- Cards ----

/** What a card shows besides the task itself, worked out once per change of the board. */
@Immutable
internal data class CardInfo(
    val task: Task,
    val parent: String?,
    val blocked: String?,
    val duplicate: String?,
    val agents: List<Pair<String, Lamp>>,
    val pending: Boolean,
    val refused: Boolean = false,
)

internal fun cardInfos(tasks: List<Task>, sessions: Map<String, Session>, pending: Set<String>, refused: Set<String> = emptySet()): List<CardInfo> {
    val byNum = HashMap<Long, Task>(tasks.size)
    for (t in tasks) t.num?.let { byNum[it] = t }
    return tasks.map { t ->
        val open = t.blockedBy.mapNotNull { byNum[it] }.filter { it.column != "done" }
        CardInfo(
            task = t,
            parent = t.parentId?.let { byNum[it]?.title },
            blocked = when {
                t.column == "done" || open.isEmpty() -> null
                open.size == 1 -> "Blocked by #${open[0].num} ${open[0].title}"
                else -> "Blocked by ${open.size} tasks"
            },
            duplicate = t.duplicateOf?.let { d -> byNum[d]?.let { "Duplicate of #$d ${it.title}" } ?: "Duplicate of #$d" },
            agents = t.sessions.map { it to (sessions[it]?.lamp ?: Lamp.Off) },
            pending = Optimistic.isTemp(t.id) || t.id in pending,
            refused = t.id in refused,
        )
    }
}

/**
 * A board card (board_view.rs `card`): the status ring, `#id`, the execution state; the title in
 * three lines; the rollup, blockers and duplicate; fields as pills; the agents on it.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun TaskCard(info: CardInfo, onClick: () -> Unit, onLongClick: () -> Unit, modifier: Modifier = Modifier, showModule: Boolean = true) {
    val c = Relay.colors
    val t = info.task
    val done = t.column == "done"
    val strong = c.strong
    Column(
        modifier
            .fillMaxWidth()
            .clip(Radii.board)
            .background(c.ink.copy(alpha = if (done) .03f else .045f))
            .border(1.dp, c.edge, Radii.board)
            .then(if (t.parentId != null) Modifier.drawBehind { drawRect(strong, size = Size(2.dp.toPx(), size.height)) } else Modifier)
            .combinedClickable(onLongClick = onLongClick, onClick = onClick)
            .padding(start = 12.dp, end = 12.dp, top = 10.dp, bottom = 11.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            StatusRing(t.column, 12.dp)
            T(t.num?.let { "#$it" } ?: "new", style = tiny, color = c.ink3, maxLines = 1)
            StateBadge(t.state)
            Box(Modifier.weight(1f))
            if (info.refused) WaitingMark("not applied", c.held) else if (info.pending) WaitingMark()
        }
        T(t.title.ifBlank { "Untitled" }, style = Relay.type.ui, color = if (done) c.ink2 else c.ink, maxLines = 3)
        info.parent?.let { p ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                Glyph("chevron-right", 10.dp, c.ink3)
                T(p, style = tiny, color = c.ink2, maxLines = 1)
            }
        }
        if (t.rollup.total > 0) Rollup(t.rollup.done, t.rollup.total)
        info.blocked?.let { b ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                Dot(c.held, 6.dp)
                T(b, style = tiny, color = c.heldText, maxLines = 1)
            }
        }
        info.duplicate?.let { T(it, style = tiny, color = c.ink2, maxLines = 1) }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(5.dp), verticalArrangement = Arrangement.spacedBy(5.dp)) {
            PriorityPill(t.priority)
            t.size?.let { FieldPill("$it · ${points(it)} pt", mono = true) }
            if (t.type != "task") FieldPill(titled(t.type)) { TypeMark(t.type) }
            if (showModule) t.moduleName?.let { m -> FieldPill(m) { Glyph("modules", 11.dp, c.ink2) } }
            t.labels.take(3).forEach { LabelChip(it) }
            if (t.labels.size > 3) T("+${t.labels.size - 3}", Modifier.padding(top = 3.dp), tiny, c.ink3)
        }
        if (info.agents.isNotEmpty()) {
            Column {
                Hairline(color = c.edge)
                Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    info.agents.distinctBy { it.first }.takeLast(3).forEach { (_, lamp) -> LampDot(lamp, 6.dp) }
                    T(info.agents.joinToString(" · ") { it.first }, style = tiny, color = c.ink2, maxLines = 1)
                }
            }
        }
    }
}

/** "n / total", a 6dp bar in the rollup violet, and the share done. */
@Composable
internal fun Rollup(done: Int, total: Int, modifier: Modifier = Modifier) {
    val c = Relay.colors
    val f = if (total == 0) 0f else (done.toFloat() / total).coerceIn(0f, 1f)
    Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        T("$done / $total", style = Relay.type.mono.copy(fontSize = 10.5.sp), color = c.ink2)
        Bar(f, Modifier.weight(1f))
        T("${(f * 100).toInt()}%", style = Relay.type.mono.copy(fontSize = 10.5.sp), color = c.ink2)
    }
}

/** A 6dp progress bar with the board's 2dp corners. */
@Composable
internal fun Bar(fraction: Float, modifier: Modifier = Modifier, color: Color = ROLLUP) {
    val track = Relay.colors.ink.copy(alpha = .1f)
    Canvas(modifier.fillMaxWidth().heightIn(min = 6.dp, max = 6.dp)) {
        val r = androidx.compose.ui.geometry.CornerRadius(2.dp.toPx())
        drawRoundRect(track, cornerRadius = r)
        if (fraction > 0f) drawRoundRect(color, size = Size(size.width * fraction.coerceIn(0f, 1f), size.height), cornerRadius = r)
    }
}

// ---- Sheets and menus ----

/**
 * A sheet from the bottom: the phone's form of the PC's popovers and panels. [content] gets a
 * `close` that slides the sheet away and then runs what was chosen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun BoardSheet(onDismiss: () -> Unit, title: String? = null, subtitle: String? = null, content: @Composable ColumnScope.(close: (() -> Unit) -> Unit) -> Unit) {
    val c = Relay.colors
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    val close: (() -> Unit) -> Unit = { then ->
        scope.launch { state.hide() }.invokeOnCompletion {
            onDismiss()
            then()
        }
    }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = state,
        shape = Radii.sheet,
        containerColor = c.slab,
        contentColor = c.ink,
        scrimColor = Color.Black.copy(alpha = .45f),
        dragHandle = { Box(Modifier.padding(top = 8.dp, bottom = 6.dp).size(width = 36.dp, height = 4.dp).clip(Radii.pill).background(c.track)) },
    ) {
        Column(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, bottom = 16.dp)) {
            if (title != null) {
                Column(Modifier.padding(start = 6.dp, end = 6.dp, bottom = 8.dp)) {
                    T(title, style = Relay.type.title, color = c.ink, maxLines = 2)
                    subtitle?.let { T(it, style = Relay.type.caption, color = c.ink3, maxLines = 2) }
                }
            }
            content(close)
        }
    }
}

/**
 * The focus for a sheet's one field, called inside the sheet: it takes the keyboard once the
 * sheet's window has focus, which comes a few frames after the sheet is shown.
 */
@Composable
internal fun rememberSheetFocus(): FocusRequester {
    val focus = remember { FocusRequester() }
    val window = LocalWindowInfo.current
    LaunchedEffect(Unit) {
        snapshotFlow { window.isWindowFocused }.first { it }
        runCatching { focus.requestFocus() }
    }
    return focus
}

/** One choice in a sheet's menu. */
@Composable
internal fun MenuRow(label: String, glyph: String, onClick: () -> Unit, detail: String? = null, danger: Boolean = false, enabled: Boolean = true, selected: Boolean = false, lead: (@Composable () -> Unit)? = null) {
    val c = Relay.colors
    ListRow(Modifier.alpha(if (enabled) 1f else .4f), selected = selected, onClick = if (enabled) onClick else null, padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp)) {
        if (lead != null) Box(Modifier.size(18.dp), contentAlignment = Alignment.Center) { lead() } else Glyph(glyph, 17.dp, if (danger) c.heldText else c.ink2)
        T(label, Modifier.weight(1f), Relay.type.ui, if (danger) c.heldText else c.ink, maxLines = 1)
        detail?.let { T(it, style = Relay.type.caption, color = c.ink3, maxLines = 1) }
        if (selected) Glyph("check", 15.dp, c.ink)
    }
}

/**
 * Where a task can go, each with its ring. Done is reached by approval, which links the branch
 * head as the task's commit; a plain move there is only for a task that already has one.
 */
@Composable
internal fun MoveChoices(task: Task, onApprove: () -> Unit, onPick: (String) -> Unit) {
    val targets = moveTargets(task)
    for (col in targets) MenuRow(Task.columnLabel(col), "columns", { onPick(col) }, lead = { StatusRing(col, 14.dp) })
    when {
        "done" in targets || task.column == "done" -> Unit
        task.column == "in_review" -> MenuRow("Done, by approval", "check", onApprove, detail = "links the branch head", lead = { StatusRing("done", 14.dp) })
        else -> T("Done comes by approval, once the work is in review.", Modifier.padding(horizontal = 10.dp, vertical = 8.dp), Relay.type.caption, Relay.colors.ink3)
    }
}

/** A sheet's title for a task: `#42 Title`, or the title alone while it has no number. */
internal fun Task.headline(): String = (num?.let { "#$it " } ?: "") + title

/** A heading inside a page, with an optional key at its end. */
@Composable
internal fun Heading(text: String, modifier: Modifier = Modifier, count: Int? = null, trailing: @Composable () -> Unit = {}) {
    val c = Relay.colors
    Row(modifier.fillMaxWidth().padding(top = 18.dp, bottom = 6.dp).heightIn(min = 32.dp), verticalAlignment = Alignment.CenterVertically) {
        T(text, style = Relay.type.section, color = c.ink2)
        count?.let { T("  $it", style = Relay.type.mono, color = c.ink3) }
        Box(Modifier.weight(1f))
        trailing()
    }
}

/** A quiet text key for a heading's end: "Add", "Edit". */
@Composable
internal fun TextKey(text: String, onClick: () -> Unit, glyph: String? = null, enabled: Boolean = true) {
    val c = Relay.colors
    Row(
        Modifier.alpha(if (enabled) 1f else .4f).heightIn(min = 32.dp).clip(Radii.key).clickable(enabled = enabled, onClick = onClick).padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        glyph?.let { Glyph(it, 13.dp, c.ink2) }
        T(text, style = Relay.type.caption, color = c.ink2, weight = FontWeight.Medium)
    }
}
