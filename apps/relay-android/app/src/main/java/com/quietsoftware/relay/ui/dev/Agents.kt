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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
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
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Restorable
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
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
import kotlinx.coroutines.flow.flowOf

/**
 * The Dev space's home, as the PC's agents view: the workspace strip (project crumb and the
 * Agents | Files | Git switch), a line for whatever needs the person, then the wall: the current
 * project's agents as terminal plates, live ones drawing their screen in miniature. Stopped agents
 * fold into one row at the end. Everything shows from the phone's copy with the PC away; the
 * lifecycle keys then say they need it.
 */
@Composable
fun AgentsScreen(nav: Nav) {
    SpaceFrame(nav) { Wall(nav) }
}

private val STOPPED = setOf("restorable", "exited")

/** Live miniatures streaming at once; the rest of the visible plates show their saved text. */
private const val MAX_MINIATURES = 6
private val PLATE_H = 180.dp

@Composable
private fun Wall(nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val project = s.project
    val pid = project?.num
    val offers by remember { nav.relay.restorable() }.collectAsStateWithLifecycle(emptyList())
    val tasks by remember(pid) { if (pid != null) nav.relay.tasks(pid) else flowOf(emptyList()) }.collectAsStateWithLifecycle(emptyList())
    val life = rememberLifecycle(nav)
    var picking by remember { mutableStateOf(false) }

    // Creation order, as the PC lays the wall out: a plate stays where it is when its state flips.
    val plates = remember(s.sessions, pid) {
        s.sessions.filter { it.projectId == pid && it.state != "closed" && it.state !in STOPPED }
            .sortedBy { if (it.id > 0) it.id else Long.MAX_VALUE }
    }
    val stopped = remember(s.sessions, offers, pid) { stoppedOf(s.sessions, offers, pid) }

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            WorkspaceStrip(s, project, onCrumb = { picking = true }, nav)
            if (project == null || pid == null) {
                Empty("folder", "No projects yet", "Add a project on the PC. It shows here once the phone has synced.")
                return@Column
            }
            BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
                val columns = if (maxWidth >= 720.dp) 2 else 1
                val grid = rememberLazyGridState()
                // The live plates in view get a stream, the first few of them; the rest show their
                // saved text. With the PC away there is no stream: a slate says so, where a
                // miniature of nothing saved was an empty black plate.
                val online = s.online
                val streaming by remember(grid, plates, online) {
                    derivedStateOf {
                        val byName = plates.associateBy { it.name }
                        grid.layoutInfo.visibleItemsInfo.asSequence()
                            .mapNotNull { (it.key as? String)?.removePrefix(PLATE_KEY)?.let(byName::get) }
                            .filter { online && it.attachable && !it.pending }
                            .map { it.name }
                            .take(MAX_MINIATURES)
                            .toSet()
                    }
                }
                LazyVerticalGrid(
                    GridCells.Fixed(columns),
                    Modifier.fillMaxSize(),
                    state = grid,
                    contentPadding = PaddingValues(bottom = 88.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                    horizontalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    item(key = "attention", span = { GridItemSpan(maxLineSpan) }, contentType = "attention") {
                        Attention(s, plates, tasks, nav)
                    }
                    if (plates.isEmpty()) {
                        item(key = "empty", span = { GridItemSpan(maxLineSpan) }, contentType = "empty") {
                            // The stopped ones fold in below: none is running, but the project has agents.
                            Empty("grid", if (stopped.isEmpty()) "No agents in ${project.name}" else "No agents running in ${project.name}", "Launch a solo agent in its own worktree, or a review group on one branch.") {
                                Key("New agent", { nav.launch(pid) }, kind = KeyKind.Primary, glyph = "plus")
                            }
                        }
                    }
                    items(plates, key = { PLATE_KEY + it.name }, contentType = { "plate" }) { p ->
                        Plate(p, nav, s.online, streaming = p.name in streaming, life)
                    }
                    if (stopped.isNotEmpty()) {
                        item(key = "stopped", span = { GridItemSpan(maxLineSpan) }, contentType = "stopped") {
                            StoppedFold(stopped, s.online, life, nav)
                        }
                    }
                }
            }
        }
        if (pid != null && plates.isNotEmpty()) {
            Key(
                "New agent",
                { nav.launch(pid) },
                Modifier.align(Alignment.BottomEnd).padding(16.dp).shadow(10.dp, Radii.key),
                kind = KeyKind.Primary,
                glyph = "plus",
            )
        }
    }
    LifecycleSheets(life)
    if (picking) {
        ProjectSheet(s.projects, s.workspaces, pid, onPick = { p -> nav.shell.pickProject(p.num) }, onDismiss = { picking = false })
    }
}

private const val PLATE_KEY = "plate:"

private fun stoppedOf(sessions: List<Session>, offers: List<Restorable>, pid: Long?): List<Stopped> {
    val offered = offers.associateBy { it.session.name }
    val known = sessions.mapTo(HashSet()) { it.name }
    val listed = sessions.filter { it.projectId == pid && it.state in STOPPED }.map {
        val offer = offered[it.name]
        Stopped(it.name, it.provider, it.role, offered = offer != null || it.state == "restorable", dirty = offer?.worktreeDirty == true)
    }
    // An offer the replica has no row for (after a restart) still belongs to its project.
    val extra = offers.filter { it.session.projectId == pid && it.session.name !in known }.map {
        Stopped(it.session.name, it.session.provider, it.session.role, offered = true, dirty = it.worktreeDirty)
    }
    return listed + extra
}

// ---- The workspace strip ----

/**
 * The strip above the wall (editor.rs): the project crumb, which picks the project, and the view
 * switch. On a narrow phone the switch takes a row of its own.
 */
@Composable
private fun WorkspaceStrip(s: ShellState, project: Project?, onCrumb: () -> Unit, nav: Nav) {
    val c = Relay.colors
    BoxWithConstraints(Modifier.fillMaxWidth().background(c.wall)) {
        val narrow = maxWidth < 520.dp
        val crumb: @Composable (Modifier) -> Unit = { m -> Crumb(s, project, onCrumb, m) }
        val views: @Composable (Modifier, Boolean) -> Unit = { m, fill ->
            Segmented(
                listOf("agents" to "Agents", "files" to "Files", "git" to "Git"),
                "agents",
                { v ->
                    val id = project?.num ?: return@Segmented
                    when (v) {
                        "files" -> nav.files(id)
                        "git" -> nav.git(id)
                    }
                },
                m,
                fill = fill,
            )
        }
        if (narrow) {
            Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp)) {
                crumb(Modifier.fillMaxWidth())
                views(Modifier.fillMaxWidth().padding(top = 6.dp), true)
            }
        } else {
            Row(Modifier.fillMaxWidth().heightIn(min = 52.dp).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                crumb(Modifier.weight(1f, fill = false))
                Box(Modifier.weight(1f))
                views(Modifier, false)
            }
        }
    }
}

/** The project crumb pill (editor.rs): branch glyph, "Workspace / Project · base", a chevron. */
@Composable
private fun Crumb(s: ShellState, project: Project?, onClick: () -> Unit, modifier: Modifier) {
    val c = Relay.colors
    Row(
        modifier
            .heightIn(min = 40.dp)
            .clip(Radii.key)
            .background(c.slab)
            .border(1.dp, c.edge, Radii.key)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Glyph("branch", 14.dp, c.ink2)
        val label = project?.let { "${crumb(it, s.workspaces)} · ${it.baseBranch}" } ?: "Pick a project"
        T(label, Modifier.weight(1f, fill = false), Relay.type.ui, c.ink, maxLines = 1)
        Glyph("chevron-down", 13.dp, c.ink3)
    }
}

// ---- What needs the person ----

/** Holds, agents that need the person, and work waiting for review; nothing when there is none. */
@Composable
private fun Attention(s: ShellState, plates: List<Session>, tasks: List<Task>, nav: Nav) {
    val c = Relay.colors
    val blocked = plates.filter { it.state == "blocked" }
    val review = tasks.count { it.column == "in_review" }
    if (s.holds.isEmpty() && blocked.isEmpty() && review == 0) return
    Row(
        Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (s.holds.isNotEmpty()) {
            Pill(if (s.holds.size == 1) "1 hold" else "${s.holds.size} holds", Modifier.heightIn(min = 36.dp), color = c.heldText, dot = c.held, onClick = nav::inbox)
        }
        if (blocked.isNotEmpty()) {
            val one = blocked.singleOrNull()
            Pill(
                one?.let { "${it.name} needs you" } ?: "${blocked.size} need you",
                Modifier.heightIn(min = 36.dp),
                color = c.heldText,
                dot = c.held,
                onClick = { if (one != null) nav.terminal(one.name) else nav.inbox() },
            )
        }
        if (review > 0) {
            Pill("$review in review", Modifier.heightIn(min = 36.dp), dot = c.waiting, onClick = nav::board)
        }
    }
}

// ---- A plate ----

/**
 * One agent as the PC draws it: the identity strip, then its screen in miniature when it is live
 * and streaming, or a slate saying where it stands over the last text the phone saved. Tap opens
 * its conversation, a long press the session's details.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun Plate(session: Session, nav: Nav, online: Boolean, streaming: Boolean, life: Lifecycle) {
    val c = Relay.colors
    val pending = session.pending
    Column(
        Modifier
            .fillMaxWidth()
            .background(c.screen)
            .combinedClickable(
                onClickLabel = "Open conversation",
                onLongClickLabel = "Session details",
                onLongClick = { if (!pending) nav.session(session.name) },
                onClick = { if (pending) nav.shell.toast(slateHint(session, online)) else nav.terminal(session.name) },
            ),
    ) {
        PlateStrip(session) {
            val act = if (pending) null else leadAct(session.state)
            if (act != null) StripKey(act, { life.run(act, session) }, dim = !online || life.busy != null)
        }
        if (streaming) {
            Miniature(nav.relay, session.name, PLATE_H)
        } else {
            Slate(session, nav, online, life)
        }
    }
}

/** A plate that is not streaming (terminal.rs slate): the state, a hint, the last saved text, the key that revives it. */
@Composable
private fun Slate(session: Session, nav: Nav, online: Boolean, life: Lifecycle) {
    val c = Relay.colors
    val lines = rememberSavedLines(nav.relay, session.name, 6)
    Column(
        Modifier.fillMaxWidth().heightIn(min = PLATE_H).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        T(session.stateLabel, style = Relay.type.plateState.copy(fontSize = 13.sp, letterSpacing = 0.1.em), color = c.ink2)
        T(slateHint(session, online), style = Relay.type.caption, color = c.ink3, maxLines = 2)
        if (!lines.isNullOrEmpty()) {
            Excerpt(lines, Modifier.weight(1f, fill = false).padding(top = 2.dp))
        } else if (lines != null && session.attachable && !online) {
            T("Nothing saved on this phone yet.", style = Relay.type.caption, color = c.ink3)
        }
        val revive = if (session.pending) null else reviveAct(session.state)
        if (revive != null) {
            Key(
                if (revive == Act.Start) "Start session" else revive.label,
                { life.run(revive, session) },
                Modifier.alpha(if (online) 1f else .5f),
                kind = KeyKind.Plain,
                glyph = revive.glyph,
                enabled = life.busy == null,
                compact = true,
            )
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
                    { life.resumeAll(list.map { it.name }) },
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
        Dot(if (life.busy == st.name) c.waiting else c.inkDim, 6.dp)
        Column(Modifier.weight(1f)) {
            T(st.name, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
            T(
                "${st.provider} · ${st.role}" + if (st.dirty) " · uncommitted changes" else "",
                style = Relay.type.caption,
                color = if (st.dirty) c.waiting else c.ink3,
                maxLines = 1,
                weight = if (st.dirty) FontWeight.Medium else null,
            )
        }
        IconKey("resume", { life.resume(st) { nav.terminal(st.name) } }, dim, size = 16.dp, tint = c.ink2, enabled = free, label = "Resume ${st.name}")
        IconKey("refresh", { life.startFresh(st) { nav.terminal(st.name) } }, dim, size = 16.dp, enabled = free, label = "Start ${st.name} fresh")
        IconKey("trash", { life.discard(st) }, dim, size = 16.dp, tint = c.heldText, enabled = free, label = "Discard ${st.name}")
    }
}
