package com.quietsoftware.relay.ui.tasks

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Module
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch

/** Until the person picks, the board opens on the first column with work in it, review first. */
private val OPEN_ORDER = listOf("in_review", "active", "ready", "backlog")

/**
 * The project board (board_view.rs) on a phone: one column at a time, swiped or picked from the
 * column strip, with search and filters. Everything reads from the phone's copy and every edit
 * goes through the outbox, so the board works with the PC away; only Dispatch needs it.
 */
@Composable
fun BoardScreen(projectId: Long?, nav: Nav) {
    SpaceFrame(nav) { Board(projectId, nav) }
}

@Composable
private fun Board(projectId: Long?, nav: Nav) {
    val c = Relay.colors
    val shell by nav.shell.state.collectAsStateWithLifecycle()
    var picked by rememberSaveable { mutableStateOf<Long?>(null) }
    val pid = picked ?: projectId ?: shell.project?.num
    val project = shell.projects.firstOrNull { it.num == pid }
    if (pid == null) {
        Empty("board", "No project yet", "Add a project on the PC and its board shows here.")
        return
    }

    val loaded by remember(pid) { nav.relay.tasks(pid) }.collectAsStateWithLifecycle(null as List<Task>?)
    val modules by remember(pid) { nav.relay.modules(pid) }.collectAsStateWithLifecycle(emptyList<Module>())
    val all = loaded.orEmpty()

    var search by rememberSaveable(pid) { mutableStateOf("") }
    var type by rememberSaveable(pid) { mutableStateOf<String?>(null) }
    var priority by rememberSaveable(pid) { mutableStateOf<String?>(null) }
    var label by rememberSaveable(pid) { mutableStateOf<String?>(null) }
    var module by rememberSaveable(pid) { mutableStateOf<Long?>(null) }
    var showFilters by rememberSaveable(pid) { mutableStateOf(false) }
    var chose by rememberSaveable(pid) { mutableStateOf(false) }
    var menuFor by remember { mutableStateOf<String?>(null) }
    var moving by remember { mutableStateOf(false) }
    var dispatchFor by remember { mutableStateOf<String?>(null) }
    var picker by remember { mutableStateOf<String?>(null) }
    val ops = remember(nav) { TaskOps(nav) }

    val sessions = remember(shell.sessions) { shell.sessions.associateBy { it.name } }
    val pending = remember(shell.outbox) { pendingTasks(shell.outbox) }
    val refused = remember(shell.outbox) { refusedTasks(shell.outbox) }
    val needle = search.trim().lowercase().removePrefix("#")
    val filtered = remember(all, needle, type, priority, label, module) {
        all.filter { t ->
            (needle.isEmpty() || t.num?.toString() == needle || t.title.lowercase().contains(needle) || t.body.lowercase().contains(needle)) &&
                (type == null || t.type == type) &&
                (priority == null || t.priority == priority) &&
                (label == null || label in t.labels) &&
                (module == null || t.moduleId == module)
        }
    }
    val cards = remember(filtered, all, sessions, pending, refused) {
        val info = cardInfos(all, sessions, pending, refused).associateBy { it.task.id }
        filtered.mapNotNull { info[it.id] }.groupBy { it.task.column }
    }
    val labels = remember(all) { all.flatMap { it.labels }.distinct().sortedBy { it.lowercase() } }
    val active = listOfNotNull(type, priority, label, module).size
    val blank = loaded != null && all.isEmpty()
    val open = all.count { it.column != "done" }
    val done = all.size - open

    val pager = rememberPagerState(initialPage = Task.COLUMNS.indexOf("backlog")) { Task.COLUMNS.size }
    val scope = rememberCoroutineScope()
    LaunchedEffect(pid, loaded != null) {
        if (loaded != null && !chose) {
            val first = OPEN_ORDER.firstOrNull { col -> all.any { it.column == col } } ?: "backlog"
            pager.scrollToPage(Task.COLUMNS.indexOf(first))
            chose = true
        }
    }
    val shown = Task.COLUMNS[pager.currentPage]
    // A new task starts in the column showing; Done is reached by approval, so it starts in Backlog there.
    val newTask = { nav.newTask(pid, column = shown.takeIf { it != "done" }) }
    val focus = LocalFocusManager.current

    Column(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp).column().align(Alignment.CenterHorizontally)) {
            Row(Modifier.fillMaxWidth().padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    T("Board", style = Relay.type.heading.copy(fontSize = 20.sp, lineHeight = 26.sp), color = c.ink)
                    T(
                        buildString {
                            append("$open open · $done done")
                            if (filtered.size != all.size) append(" · ${filtered.size} shown")
                        },
                        style = Relay.type.caption, color = c.ink3, maxLines = 1,
                    )
                }
                Key("New task", newTask, kind = KeyKind.Primary, glyph = "plus", compact = true)
            }
            Row(Modifier.fillMaxWidth().padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Segmented(listOf("board" to "Board", "modules" to "Modules"), "board", { if (it == "modules") nav.modules(pid) })
                Box(Modifier.weight(1f))
                if (shell.projects.size > 1) {
                    Pill(project?.name ?: "Project", Modifier.widthIn(max = 180.dp), glyph = "folder", onClick = { picker = "project" })
                } else {
                    project?.let { T(it.name, style = Relay.type.caption, color = c.ink3, maxLines = 1) }
                }
            }
            if (blank) return@Column
            Field(
                search, { search = it }, Modifier.fillMaxWidth().padding(top = 12.dp),
                placeholder = "Search title, description or #id",
                leading = "search",
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                // The board filters as it is typed; the keyboard's search key puts the keyboard away.
                keyboardActions = KeyboardActions(onSearch = { focus.clearFocus() }),
            ) {
                if (search.isNotEmpty()) FieldKey("close", "Clear the search", c.ink3) { search = "" }
                FieldKey("sliders", if (showFilters) "Hide the filters" else "Filters", if (showFilters || active > 0) c.ink else c.ink3) { showFilters = !showFilters }
            }
            if (showFilters || active > 0) {
                Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Pill(type?.let { "Type: ${titled(it)}" } ?: "Type", selected = type != null, glyph = "chevron-down", onClick = { picker = "type" })
                    Pill(priority?.let { "Priority: ${titled(it)}" } ?: "Priority", selected = priority != null, glyph = "chevron-down", onClick = { picker = "priority" })
                    if (labels.isNotEmpty() || label != null) Pill(label?.let { "Label: $it" } ?: "Label", selected = label != null, glyph = "chevron-down", onClick = { picker = "label" })
                    if (modules.isNotEmpty() || module != null) Pill(module?.let { m -> "Module: ${modules.firstOrNull { it.id == m.toString() }?.name ?: "#$m"}" } ?: "Module", selected = module != null, glyph = "chevron-down", onClick = { picker = "module" })
                    if (active > 0) Pill("Clear", glyph = "close", onClick = { type = null; priority = null; label = null; module = null })
                }
            }
        }
        if (blank) {
            Empty("board", "No tasks yet", "Capture the first piece of work for ${project?.name ?: "this project"}.") {
                Key("New task", newTask, kind = KeyKind.Primary, glyph = "plus")
            }
            return@Column
        }
        ColumnStrip(pager.currentPage, { i -> scope.launch { pager.animateScrollToPage(i) } }) { col -> cards[col]?.size ?: 0 }
        Hairline(color = c.edge)
        HorizontalPager(pager, Modifier.fillMaxWidth().weight(1f), key = { Task.COLUMNS[it] }, beyondViewportPageCount = 1) { page ->
            val col = Task.COLUMNS[page]
            val list = cards[col].orEmpty()
            if (list.isEmpty()) {
                if (loaded != null) LaneEmpty(col, filtered.size != all.size, if (col != "done") newTask else null)
            } else {
                LazyColumn(
                    Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    items(list, key = { it.task.id }) { info ->
                        TaskCard(
                            info,
                            onClick = { nav.task(info.task.id) },
                            onLongClick = { menuFor = info.task.id; moving = false },
                            modifier = Modifier.animateItem().column(),
                            showModule = module == null,
                        )
                    }
                    item(key = "hint") { T("Swipe for the next column · hold a card for more", Modifier.padding(top = 8.dp), Relay.type.caption, c.ink3) }
                }
            }
        }
    }

    // ---- Sheets ----
    val menuTask = menuFor?.let { id -> all.firstOrNull { it.id == id } }
    if (menuTask != null) {
        BoardSheet({ menuFor = null }, title = if (moving) "Move ${menuTask.named()} to" else menuTask.headline()) { close ->
            if (moving) {
                MoveChoices(menuTask, { close { ops.approve(menuTask) } }) { to -> close { ops.move(menuTask, to, indexIn(all, menuTask)) } }
            } else {
                MenuRow("Open", "external", { close { nav.task(menuTask.id) } })
                MenuRow("Move to…", "columns", { moving = true })
                if (menuTask.column == "in_review") MenuRow("Approve", "check", { close { ops.approve(menuTask) } }, detail = "moves it to Done")
                if (menuTask.column != "done") MenuRow("Dispatch…", "play", { close { dispatchFor = menuTask.id } }, enabled = menuTask.num != null, detail = if (menuTask.num == null) "once the PC has it" else null)
                menuTask.sessions.lastOrNull()?.let { s -> MenuRow("Open terminal", "terminal", { close { nav.terminal(s) } }, detail = s) }
                MenuRow("Delete", "trash", { close { ops.delete(menuTask) } }, danger = true)
            }
        }
    }
    val dispatchTask = dispatchFor?.let { id -> all.firstOrNull { it.id == id } }
    if (dispatchTask != null) DispatchSheet(dispatchTask, nav) { dispatchFor = null }

    when (picker) {
        "project" -> BoardSheet({ picker = null }, title = "Project") { close ->
            for (p in shell.projects) {
                MenuRow(p.name, "folder", { close { picked = p.num; nav.shell.pickProject(p.num) } }, detail = p.path.substringAfterLast('/').takeIf { it != p.name }, selected = p.num == pid)
            }
        }
        "type" -> ChoiceSheet("Type", TYPES, type, { picker = null }) { type = it }
        "priority" -> ChoiceSheet("Priority", PRIORITIES, priority, { picker = null }) { priority = it }
        "label" -> ChoiceSheet("Label", labels.map { it to it }, label, { picker = null }) { label = it }
        "module" -> BoardSheet({ picker = null }, title = "Module") { close ->
            MenuRow("Any module", "modules", { close { module = null } }, selected = module == null)
            for (m in modules) {
                val n = m.id.toLongOrNull() ?: continue
                MenuRow(m.name, "modules", { close { module = n } }, detail = if (m.completedAt != null) "done" else null, selected = module == n)
            }
        }
    }
}

/** A key inside the search field: its glyph in a 32dp target, named for a screen reader. */
@Composable
private fun FieldKey(glyph: String, label: String, tint: Color, onClick: () -> Unit) {
    Box(
        Modifier.size(32.dp).clip(Radii.icon).clickable(role = Role.Button, onClick = onClick).semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Glyph(glyph, 16.dp, tint)
    }
}

/** A filter's choices, "Any" first. */
@Composable
private fun ChoiceSheet(title: String, options: List<Pair<String, String>>, current: String?, onDismiss: () -> Unit, onPick: (String?) -> Unit) {
    BoardSheet(onDismiss, title = title) { close ->
        MenuRow("Any", "sliders", { close { onPick(null) } }, selected = current == null)
        Column(Modifier.weight(1f, fill = false)) {
            LazyColumn {
                items(options, key = { it.first }) { (value, label) ->
                    MenuRow(label, "sliders", { close { onPick(value) } }, selected = current == value, lead = when (title) {
                        "Priority" -> ({ PriorityMark(value, 13.dp) })
                        "Type" -> ({ TypeMark(value, 10.dp) })
                        "Label" -> ({ Dot(labelHue(value), 8.dp) })
                        else -> null
                    })
                }
            }
        }
    }
}

/** The columns as a strip of tabs, each with its ring and count; the one showing lit. */
@Composable
private fun ColumnStrip(selected: Int, onSelect: (Int) -> Unit, count: (String) -> Int) {
    val c = Relay.colors
    val state = rememberLazyListState()
    LaunchedEffect(selected) { state.animateScrollToItem((selected - 1).coerceAtLeast(0)) }
    LazyRow(
        Modifier.fillMaxWidth().padding(top = 12.dp),
        state = state,
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        itemsIndexed(Task.COLUMNS, key = { _, col -> col }) { i, col ->
            val on = i == selected
            Row(
                Modifier.heightIn(min = 34.dp).clip(Radii.pill)
                    .background(if (on) c.track else Color.Transparent)
                    .border(1.dp, if (on) c.lineFocus else c.edge, Radii.pill)
                    .clickable { onSelect(i) }
                    .padding(horizontal = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(7.dp),
            ) {
                StatusRing(col, 12.dp)
                T(Task.columnLabel(col), style = Relay.type.uiMedium, color = if (on) c.ink else c.ink2, weight = if (on) FontWeight.SemiBold else FontWeight.Medium)
                T(count(col).toString(), style = Relay.type.mono, color = if (on) c.ink2 else c.ink3)
            }
        }
    }
}

/** An empty lane, in the PC's words (board_view.rs `empty_lane`). */
@Composable
private fun LaneEmpty(column: String, filtered: Boolean, onNew: (() -> Unit)?) {
    val c = Relay.colors
    val (title, hint) = if (filtered) "No matches" to "Nothing here fits the search or filters." else when (column) {
        "backlog" -> "All quiet" to "Capture a task and it waits here."
        "ready" -> "All quiet" to "Move a task here when it is ready to dispatch."
        "active" -> "No work in flight" to "Dispatched tasks land here."
        "in_review" -> "Nothing to review" to "Agents move finished work here."
        else -> "Nothing approved yet" to "Approve a task in review and it lands here."
    }
    Box(Modifier.fillMaxSize().padding(16.dp), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.column().fillMaxWidth().clip(Radii.board).border(1.dp, c.edge, Radii.board).padding(horizontal = 16.dp, vertical = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            StatusRing(column, 18.dp)
            T(title, Modifier.padding(top = 6.dp), Relay.type.uiMedium, c.ink2)
            T(hint, style = Relay.type.caption.copy(textAlign = androidx.compose.ui.text.style.TextAlign.Center), color = c.ink3)
            if (onNew != null && !filtered) Key("New task", onNew, Modifier.padding(top = 10.dp), glyph = "plus", compact = true)
        }
    }
}
