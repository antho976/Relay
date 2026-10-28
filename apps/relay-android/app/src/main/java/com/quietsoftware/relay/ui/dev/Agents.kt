package com.quietsoftware.relay.ui.dev

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Restorable
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.shell.ShellState
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Relay

/**
 * Every agent, wherever it works, as one list a person can search (design 14, "Agents"): a field
 * that finds an agent by what it is doing, its name, its task, its branch or its project; chips
 * that keep only the ones that need the person, are working or are ready; and one project or all
 * of them. Each agent is a card with its badge (its own colour and initials), what it is doing,
 * where, and when it last said anything. Stopped agents fold into one row at the end. Everything
 * shows from the phone's copy with the PC away; the lifecycle keys then say they need it.
 */
@Composable
fun AgentsScreen(nav: Nav) {
    SpaceFrame(nav) { Wall(nav) }
}

/** The project the Agents page shows, or null for all of them: a project's row in the drawer picks one. */
internal object AgentsScope {
    var project by mutableStateOf<Long?>(null)
}

private val STOPPED = setOf("restorable", "exited")

/** The state chips, in order, and which agents each keeps. */
private enum class Show(val label: String, val keeps: (Session) -> Boolean) {
    All("All", { true }),
    Needs("Needs you", { it.state == "blocked" }),
    Working("Working", { it.state == "running" || it.state == "spawning" }),
    Ready("Ready", { it.state == "idle" }),
    Other("Parked", { it.state !in setOf("blocked", "running", "spawning", "idle") }),
}

@Composable
private fun Wall(nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val scope = AgentsScope.project?.takeIf { id -> s.projects.any { it.num == id } }
    val project = scope?.let { id -> s.projects.firstOrNull { it.num == id } }
    val offers by remember { nav.relay.restorable() }.collectAsStateWithLifecycle(emptyList())
    val tasks by remember { nav.relay.allTasks() }.collectAsStateWithLifecycle(emptyList())
    val life = rememberLifecycle(nav)
    var picking by remember { mutableStateOf(false) }
    var query by rememberSaveable { mutableStateOf("") }
    var show by rememberSaveable { mutableStateOf(Show.All) }

    val byTask = remember(tasks) { tasks.associateBy { it.num } }
    val byProject = remember(s.projects) { s.projects.associateBy { it.num } }
    val inScope = remember(s.sessions, scope) {
        s.sessions.filter { it.state != "closed" && it.state !in STOPPED && (scope == null || it.projectId == scope) }
    }
    val words = remember(query) { query.lowercase().split(' ').filter { it.isNotBlank() } }
    fun matches(a: Session): Boolean {
        if (words.isEmpty()) return true
        val task = a.taskId?.let(byTask::get)
        val hay = listOfNotNull(a.intent, a.name, task?.title, task?.num?.let { "#$it" }, a.branch, byProject[a.projectId]?.name, providerName(a.provider), a.model, a.role)
            .joinToString(" ").lowercase()
        return words.all { it in hay }
    }
    val found = remember(inScope, words, byTask, byProject) { inScope.filter(::matches) }
    val shown = remember(found, show) {
        // What needs the person first; the rest newest first, so the one just started is at the top.
        found.filter(show.keeps).sortedWith(compareBy<Session> { it.state != "blocked" }.thenByDescending { if (it.id > 0) it.id else Long.MAX_VALUE })
    }
    // One project's agents under its name when every project is shown and more than one has any.
    val groups = remember(shown, scope, s.projects) {
        val by = shown.groupBy { it.projectId }
        if (scope != null || by.size < 2) listOf(null to shown)
        else s.projects.mapNotNull { p -> by[p.num]?.let { p to it } } + by.filterKeys { k -> s.projects.none { it.num == k } }.values.map { null to it }
    }
    val stopped = remember(s.sessions, offers, scope, words) { stoppedOf(s.sessions, offers, scope).filter { matches(it.session) } }

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            if (s.projects.isEmpty()) {
                Empty("folder", "No projects yet", "Add a project on the PC. It shows here once the phone has synced.")
                return@Column
            }
            BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
                val columns = if (maxWidth >= 720.dp) 2 else 1
                val grid = rememberLazyGridState()
                LazyVerticalGrid(
                    GridCells.Fixed(columns),
                    Modifier.fillMaxSize(),
                    state = grid,
                    contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 88.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    item(key = "find", span = { GridItemSpan(maxLineSpan) }, contentType = "find") {
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            Field(
                                query, { query = it }, Modifier.fillMaxWidth(),
                                placeholder = "Find an agent: what it does, its name, a task",
                                leading = "search",
                                trailing = if (query.isNotEmpty()) { { IconKey("close", { query = "" }, size = 15.dp, label = "Clear") } } else null,
                            )
                            Row(
                                Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                                horizontalArrangement = Arrangement.spacedBy(6.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Pill(
                                    project?.name ?: "All projects",
                                    glyph = "folder",
                                    color = c.ink,
                                    onClick = { picking = true },
                                )
                                for (o in Show.entries) {
                                    val n = found.count(o.keeps)
                                    if (o != Show.All && n == 0 && show != o) continue
                                    Pill(
                                        "${o.label} $n",
                                        selected = show == o,
                                        dot = when (o) { Show.Needs -> c.waiting; Show.Working -> c.live; else -> null },
                                        onClick = { show = if (show == o) Show.All else o },
                                    )
                                }
                            }
                            Attention(s, nav)
                            if (project != null) {
                                Segmented(
                                    listOf("agents" to "Agents", "files" to "Files", "git" to "Changes", "design" to "Designs"),
                                    "agents",
                                    { v ->
                                        val id = project.num ?: return@Segmented
                                        when (v) {
                                            "files" -> nav.files(id)
                                            "git" -> nav.git(id)
                                            "design" -> nav.designs(id)
                                        }
                                    },
                                    Modifier.fillMaxWidth(),
                                    fill = true,
                                )
                            }
                        }
                    }
                    if (shown.isEmpty()) {
                        item(key = "empty", span = { GridItemSpan(maxLineSpan) }, contentType = "empty") {
                            when {
                                words.isNotEmpty() || show != Show.All -> Empty("search", "No agent matches", if (words.isNotEmpty()) "Nothing running is about “${query.trim()}”." else "None is ${show.label.lowercase()} now.") {
                                    Key("Show every agent", { query = ""; show = Show.All; AgentsScope.project = null }, kind = KeyKind.Plain, compact = true)
                                }
                                // The stopped ones fold in below: none is running, but there are agents.
                                else -> Empty("grid", if (stopped.isEmpty()) "No agents yet" else "No agents running", "Launch a solo agent in its own worktree, or a review group on one branch.") {
                                    Key("New agent", { nav.launch(scope ?: s.project?.num) }, kind = KeyKind.Primary, glyph = "plus")
                                }
                            }
                        }
                    }
                    for ((p, list) in groups) {
                        if (p != null) {
                            item(key = "head:${p.id}", span = { GridItemSpan(maxLineSpan) }, contentType = "head") {
                                Row(
                                    Modifier.fillMaxWidth().clip(Radii.key).clickable { AgentsScope.project = p.num }.padding(start = 4.dp, end = 4.dp, top = 12.dp, bottom = 2.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Eyebrow(p.name, Modifier.weight(1f))
                                    T(list.size.toString(), style = Relay.type.mono, color = c.ink3)
                                }
                            }
                        }
                        items(list, key = { PLATE_KEY + it.name }, contentType = { "plate" }) { a ->
                            val task = a.taskId?.let(byTask::get)
                            Plate(a, task, byProject[a.projectId].takeIf { scope == null && groups.size < 2 && s.projects.size > 1 }, nav, s.online, life)
                        }
                    }
                    if (stopped.isNotEmpty()) {
                        item(key = "stopped", span = { GridItemSpan(maxLineSpan) }, contentType = "stopped") {
                            StoppedFold(stopped, s.online, life, nav)
                        }
                    }
                }
            }
        }
        val launchIn = scope ?: s.project?.num
        if (launchIn != null && shown.isNotEmpty()) {
            Key(
                "New agent",
                { nav.launch(launchIn) },
                Modifier.align(Alignment.BottomEnd).padding(16.dp).shadow(10.dp, Radii.key),
                kind = KeyKind.Primary,
                glyph = "plus",
            )
        }
    }
    LifecycleSheets(life)
    if (picking) {
        ProjectSheet(
            s.projects, s.workspaces, scope,
            onPick = { p -> AgentsScope.project = p.num; nav.shell.pickProject(p.num) },
            onDismiss = { picking = false },
            onAll = { AgentsScope.project = null },
        )
    }
}

private const val PLATE_KEY = "plate:"

private fun stoppedOf(sessions: List<Session>, offers: List<Restorable>, pid: Long?): List<Stopped> {
    val offered = offers.associateBy { it.session.name }
    val known = sessions.mapTo(HashSet()) { it.name }
    val listed = sessions.filter { (pid == null || it.projectId == pid) && it.state in STOPPED }.map {
        val offer = offered[it.name]
        Stopped(it, offered = offer != null || it.state == "restorable", dirty = offer?.worktreeDirty == true)
    }
    // An offer the replica has no row for (after a restart) still belongs to its project.
    val extra = offers.filter { (pid == null || it.session.projectId == pid) && it.session.name !in known }.map {
        Stopped(it.session, offered = true, dirty = it.worktreeDirty)
    }
    return listed + extra
}

// ---- What needs the person ----

/** Held commands, besides the agents' own chips; nothing when there is none. */
@Composable
private fun Attention(s: ShellState, nav: Nav) {
    val c = Relay.colors
    if (s.holds.isEmpty()) return
    Pill(if (s.holds.size == 1) "1 command held for you" else "${s.holds.size} commands held for you", Modifier.heightIn(min = 36.dp), color = c.heldText, dot = c.held, onClick = nav::inbox)
}

// ---- An agent ----

/**
 * One agent as a card: its badge, what it is doing in words, who and where it is, its state and
 * when it last said anything, and the one key its state offers. No terminal: a tap opens its
 * conversation, a long press the session's details. [project] is named when the list mixes them.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun Plate(session: Session, task: Task?, project: Project?, nav: Nav, online: Boolean, life: Lifecycle) {
    val c = Relay.colors
    val pending = session.pending
    val held = session.state == "blocked"
    val title = agentTitle(session, task)
    Row(
        Modifier
            .fillMaxWidth()
            .clip(Radii.card)
            .background(c.slab)
            .border(1.dp, if (held) c.waiting.copy(alpha = .45f) else c.edge, Radii.card)
            .combinedClickable(
                onClickLabel = "Open conversation",
                onLongClickLabel = "Session details",
                onLongClick = { if (!pending) nav.session(session.name) },
                onClick = { if (pending) nav.shell.toast(slateHint(session, online)) else nav.chat(session.name) },
            )
            .padding(start = 12.dp, end = 10.dp, top = 12.dp, bottom = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        AgentBadge(session, 40.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            T(title, style = Relay.type.uiMedium, color = c.ink, maxLines = 2, weight = FontWeight.SemiBold)
            val where = agentWhere(session, title, project, task)
            if (where.isNotEmpty()) T(where, style = Relay.type.caption, color = c.ink2, maxLines = 1)
            Row(Modifier.padding(top = 2.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val heard = lastHeard(session)
                T(
                    stateWord(session) + if (heard.isNotEmpty()) " · $heard" else "",
                    Modifier.weight(1f),
                    Relay.type.caption,
                    when (session.state) { "blocked" -> c.waiting; "running", "spawning" -> c.live; else -> c.ink3 },
                    maxLines = 1,
                    weight = if (held) FontWeight.Medium else null,
                )
                val act = if (pending) null else leadAct(session.state) ?: reviveAct(session.state)
                if (act != null) {
                    Key(
                        if (act == Act.Start) "Start" else act.label,
                        { life.run(act, session) },
                        Modifier.alpha(if (online) 1f else .5f),
                        kind = KeyKind.Quiet,
                        glyph = act.glyph,
                        enabled = life.busy == null,
                        compact = true,
                    )
                }
            }
        }
    }
}

// ---- Stopped agents ----

/**
 * Every stopped agent of the project folded into one row (the old app's StoppedAgents): Resume
 * all, or open it for each agent's Resume, Start fresh and Discard. While "Resume all" runs, the
 * row counts and a tap stops it after the current agent.
 */
@Composable
private fun StoppedFold(list: List<Stopped>, online: Boolean, life: Lifecycle, nav: Nav) {
    val c = Relay.colors
    var open by rememberSaveable { mutableStateOf(false) }
    val progress = life.progress
    Slab(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp), padding = PaddingValues(horizontal = 6.dp, vertical = 4.dp)) {
        ListRow(onClick = { if (progress != null) life.stopAfterCurrent() else open = !open }) {
            Glyph("pause", 16.dp, c.ink3)
            T(
                progress?.let { (done, total) ->
                    if (life.halt) "Stopping after ${life.busy ?: "this one"}…" else "Resuming ${minOf(done + 1, total)} of $total · tap to stop after this one"
                }
                    ?: if (list.size == 1) "1 stopped agent" else "${list.size} stopped agents",
                Modifier.weight(1f),
                Relay.type.ui,
                c.ink,
                maxLines = 1,
            )
            if (progress == null) {
                Key(
                    if (list.size == 1) "Resume" else "Resume all",
                    {
                        // One agent resumes as its row's key does, without a question; several are asked about.
                        val one = list.singleOrNull()
                        if (one != null) life.resume(one) { nav.chat(one.name) } else life.resumeAll(list.map { it.name })
                    },
                    Modifier.alpha(if (online) 1f else .5f),
                    kind = KeyKind.Quiet,
                    enabled = life.busy == null,
                    compact = true,
                )
                Glyph(if (open) "chevron-up" else "chevron-down", 13.dp, c.ink3)
            }
        }
        if (open) {
            for (st in list) StoppedRow(st, online, life, nav)
        }
    }
}

@Composable
private fun StoppedRow(st: Stopped, online: Boolean, life: Lifecycle, nav: Nav) {
    val c = Relay.colors
    // With the PC away the keys stay tappable and say they need it.
    val free = life.busy == null && life.progress == null
    val dim = Modifier.alpha(if (online) 1f else .5f)
    ListRow(onClick = { nav.session(st.name) }, padding = PaddingValues(start = 12.dp, end = 0.dp, top = 2.dp, bottom = 2.dp)) {
        AgentBadge(st.session, 28.dp, lamp = false)
        Column(Modifier.weight(1f)) {
            val title = agentTitle(st.session)
            T(title, style = Relay.type.uiMedium, color = if (life.busy == st.name) c.waiting else c.ink, maxLines = 1)
            T(
                listOfNotNull(st.name.takeIf { it != title }, providerName(st.provider), st.role.takeIf { it != "builder" }).joinToString(" · ") +
                    if (st.dirty) " · uncommitted changes" else "",
                style = Relay.type.caption,
                color = if (st.dirty) c.waiting else c.ink3,
                maxLines = 1,
                weight = if (st.dirty) FontWeight.Medium else null,
            )
        }
        IconKey("resume", { life.resume(st) { nav.chat(st.name) } }, dim, size = 16.dp, tint = c.ink2, enabled = free, label = "Resume ${st.name}")
        IconKey("refresh", { life.startFresh(st) { nav.chat(st.name) } }, dim, size = 16.dp, enabled = free, label = "Start ${st.name} fresh")
        IconKey("trash", { life.discard(st) }, dim, size = 16.dp, tint = c.heldText, enabled = free, label = "Discard ${st.name}")
    }
}
